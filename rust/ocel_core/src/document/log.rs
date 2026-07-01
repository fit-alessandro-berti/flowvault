impl CompactOcelLog {
    fn from_input(input: &str, format_hint: Option<&str>) -> OcelResult<Self> {
        let format = detect_format(input, format_hint)?;
        let source = match format {
            OcelFormat::Json => parse_json(input)?,
            OcelFormat::Xml => parse_xml(input)?,
        };
        Self::from_source(source, format)
    }

    fn from_bytes(input: &[u8], format_hint: Option<&str>) -> OcelResult<Self> {
        let text = decode_ocel_bytes(input)?;
        Self::from_input(&text, format_hint)
    }

    fn from_source(source: SourceLog, format: OcelFormat) -> OcelResult<Self> {
        let mut pool = StringPool::default();
        let mut event_type_names = HashSet::new();
        let mut object_type_names = HashSet::new();
        let mut event_attr_types = HashMap::new();
        let mut object_attr_types = HashMap::new();
        let mut event_types = Vec::with_capacity(source.event_types.len());
        let mut object_types = Vec::with_capacity(source.object_types.len());

        for source_type in &source.event_types {
            if !event_type_names.insert(source_type.name.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate event type '{}'",
                    source_type.name
                )));
            }
            event_types.push(compact_type_def(
                source_type,
                &mut pool,
                &mut event_attr_types,
                "event type",
            )?);
        }

        for source_type in &source.object_types {
            if !object_type_names.insert(source_type.name.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate object type '{}'",
                    source_type.name
                )));
            }
            object_types.push(compact_type_def(
                source_type,
                &mut pool,
                &mut object_attr_types,
                "object type",
            )?);
        }

        let mut object_ids = HashSet::new();
        for object in &source.objects {
            if !object_ids.insert(object.id.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate object id '{}'",
                    object.id
                )));
            }
            if !object_type_names.contains(&object.type_name) {
                return Err(OcelError::new(format!(
                    "object '{}' references unknown object type '{}'",
                    object.id, object.type_name
                )));
            }
        }

        let mut event_ids = HashSet::new();
        for event in &source.events {
            if !event_ids.insert(event.id.clone()) {
                return Err(OcelError::new(format!("duplicate event id '{}'", event.id)));
            }
            if !event_type_names.contains(&event.type_name) {
                return Err(OcelError::new(format!(
                    "event '{}' references unknown event type '{}'",
                    event.id, event.type_name
                )));
            }
            for rel in &event.relationships {
                if !object_ids.contains(&rel.object_id) {
                    return Err(OcelError::new(format!(
                        "event '{}' references unknown object '{}'",
                        event.id, rel.object_id
                    )));
                }
            }
        }

        let mut objects = Vec::with_capacity(source.objects.len());
        let mut object_index = HashMap::with_capacity(source.objects.len());

        for source_object in &source.objects {
            for rel in &source_object.relationships {
                if !object_ids.contains(&rel.object_id) {
                    return Err(OcelError::new(format!(
                        "object '{}' references unknown object '{}'",
                        source_object.id, rel.object_id
                    )));
                }
            }

            let id = pool.intern(&source_object.id);
            let object = Object {
                id,
                type_name: pool.intern(&source_object.type_name),
                attributes: compact_timed_attributes(
                    &source_object.attributes,
                    &source_object.type_name,
                    &object_attr_types,
                    &mut pool,
                )?,
                relationships: compact_relationships(&source_object.relationships, &mut pool),
                lifecycle: Vec::new(),
            };
            object_index.insert(id, objects.len());
            objects.push(object);
        }

        let mut events = Vec::with_capacity(source.events.len());
        for source_event in &source.events {
            let time_ms = parse_timestamp_ms(&source_event.time)?;
            let event = Event {
                id: pool.intern(&source_event.id),
                type_name: pool.intern(&source_event.type_name),
                time_ms,
                attributes: compact_attributes(
                    &source_event.attributes,
                    &source_event.type_name,
                    &event_attr_types,
                    &mut pool,
                )?,
                relationships: compact_relationships(&source_event.relationships, &mut pool),
            };
            events.push(event);
        }

        for (event_index, event) in events.iter().enumerate() {
            for rel in &event.relationships {
                if let Some(object_pos) = object_index.get(&rel.object_id) {
                    objects[*object_pos].lifecycle.push(event_index);
                }
            }
        }

        for object in &mut objects {
            object
                .lifecycle
                .sort_by_key(|event_index| (events[*event_index].time_ms, *event_index));
        }

        Ok(Self {
            format,
            pool: pool.finish(),
            event_types,
            object_types,
            events,
            objects,
            object_index,
            state_leading_object_type: None,
        })
    }

    fn summary(&self) -> OcelSummary {
        OcelSummary {
            source_format: self.format.as_str(),
            event_types: self.event_types.len(),
            object_types: self.object_types.len(),
            events: self.events.len(),
            objects: self.objects.len(),
            e2o_relationships: self
                .events
                .iter()
                .map(|event| event.relationships.len())
                .sum(),
            o2o_relationships: self
                .objects
                .iter()
                .map(|object| object.relationships.len())
                .sum(),
            interned_strings: self.pool.values.len(),
            objects_with_lifecycle: self
                .objects
                .iter()
                .filter(|object| !object.lifecycle.is_empty())
                .count(),
            stateful_events: self.count_events_with_attribute("state"),
        }
    }

    fn count_events_with_attribute(&self, attribute_name: &str) -> usize {
        self.events
            .iter()
            .filter(|event| {
                event
                    .attributes
                    .iter()
                    .any(|attribute| self.pool.resolve(attribute.name) == attribute_name)
            })
            .count()
    }

    fn summary_json(&self) -> String {
        serde_json::to_string(&self.summary()).expect("summary serialization cannot fail")
    }

    fn filter(&self, filter: &OcelFilterRequest) -> Self {
        let event_type_selection = if filter.event_types.is_empty() {
            self.event_types
                .iter()
                .map(|type_def| self.pool.resolve(type_def.name))
                .collect::<HashSet<_>>()
        } else {
            filter
                .event_types
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>()
        };
        let object_type_selection = if filter.object_types.is_empty() {
            self.object_types
                .iter()
                .map(|type_def| self.pool.resolve(type_def.name))
                .collect::<HashSet<_>>()
        } else {
            filter
                .object_types
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>()
        };
        let all_event_types_selected = event_type_selection.len() >= self.event_types.len();
        let all_object_types_selected = object_type_selection.len() >= self.object_types.len();

        if all_event_types_selected
            && all_object_types_selected
            && !filter.has_object_predicates()
            && !filter.has_time_predicate()
        {
            return self.clone();
        }

        let selected_object_ids = self
            .objects
            .iter()
            .enumerate()
            .filter(|(object_index, object)| {
                object_type_selection.contains(self.pool.resolve(object.type_name))
                    && self.object_satisfies_filter(*object_index, object, filter)
            })
            .map(|(_, object)| object.id)
            .collect::<HashSet<_>>();

        let mut retained_events = Vec::new();
        let mut retained_object_ids = HashSet::new();
        let require_relationship_match =
            !all_object_types_selected || filter.has_object_predicates();

        for event in &self.events {
            if !event_type_selection.contains(self.pool.resolve(event.type_name)) {
                continue;
            }
            if !filter.accepts_time(event.time_ms) {
                continue;
            }

            let relationships = event
                .relationships
                .iter()
                .filter(|relationship| selected_object_ids.contains(&relationship.object_id))
                .cloned()
                .collect::<Vec<_>>();

            if require_relationship_match && relationships.is_empty() {
                continue;
            }

            retained_object_ids.extend(
                relationships
                    .iter()
                    .map(|relationship| relationship.object_id),
            );
            retained_events.push(Event {
                id: event.id,
                type_name: event.type_name,
                time_ms: event.time_ms,
                attributes: event.attributes.clone(),
                relationships,
            });
        }

        let mut objects = self
            .objects
            .iter()
            .filter(|object| retained_object_ids.contains(&object.id))
            .map(|object| Object {
                id: object.id,
                type_name: object.type_name,
                attributes: object.attributes.clone(),
                relationships: object
                    .relationships
                    .iter()
                    .filter(|relationship| retained_object_ids.contains(&relationship.object_id))
                    .cloned()
                    .collect(),
                lifecycle: Vec::new(),
            })
            .collect::<Vec<_>>();
        let object_index = objects
            .iter()
            .enumerate()
            .map(|(index, object)| (object.id, index))
            .collect::<HashMap<_, _>>();

        for (event_index, event) in retained_events.iter().enumerate() {
            for relationship in &event.relationships {
                if let Some(object_pos) = object_index.get(&relationship.object_id) {
                    objects[*object_pos].lifecycle.push(event_index);
                }
            }
        }

        for object in &mut objects {
            object
                .lifecycle
                .sort_by_key(|event_index| (retained_events[*event_index].time_ms, *event_index));
        }

        let retained_event_types = retained_events
            .iter()
            .map(|event| event.type_name)
            .collect::<HashSet<_>>();
        let retained_object_types = objects
            .iter()
            .map(|object| object.type_name)
            .collect::<HashSet<_>>();

        Self {
            format: self.format,
            pool: self.pool.clone(),
            event_types: self
                .event_types
                .iter()
                .filter(|type_def| retained_event_types.contains(&type_def.name))
                .cloned()
                .collect(),
            object_types: self
                .object_types
                .iter()
                .filter(|type_def| retained_object_types.contains(&type_def.name))
                .cloned()
                .collect(),
            events: retained_events,
            objects,
            object_index,
            state_leading_object_type: self.state_leading_object_type,
        }
    }

    fn filter_options(&self) -> FilterOptions {
        let event_types = self
            .event_types
            .iter()
            .map(|type_def| self.pool.resolve(type_def.name).to_owned())
            .collect();
        let object_types = self
            .object_types
            .iter()
            .map(|type_def| self.pool.resolve(type_def.name).to_owned())
            .collect();

        FilterOptions {
            event_types,
            object_types,
            text_attributes: self.text_attribute_options(),
            time_min_ms: self.events.iter().map(|event| event.time_ms).min(),
            time_max_ms: self.events.iter().map(|event| event.time_ms).max(),
            time_buckets: self.time_filter_buckets(32),
        }
    }

    fn time_filter_buckets(&self, buckets: usize) -> Vec<FilterTimeBucket> {
        let Some(min_ms) = self.events.iter().map(|event| event.time_ms).min() else {
            return Vec::new();
        };
        let max_ms = self
            .events
            .iter()
            .map(|event| event.time_ms)
            .max()
            .unwrap_or(min_ms);
        let bucket_count = buckets.max(1);
        let span = (max_ms - min_ms).max(1) as f64;
        let mut counts = vec![0usize; bucket_count];
        for event in &self.events {
            let ratio = ((event.time_ms - min_ms) as f64 / span).clamp(0.0, 1.0);
            let index = ((ratio * bucket_count as f64).floor() as usize).min(bucket_count - 1);
            counts[index] += 1;
        }
        let bucket_width = span / bucket_count as f64;
        counts
            .into_iter()
            .enumerate()
            .map(|(index, count)| FilterTimeBucket {
                start_ms: min_ms + (bucket_width * index as f64).round() as i64,
                end_ms: if index + 1 == bucket_count {
                    max_ms
                } else {
                    min_ms + (bucket_width * (index + 1) as f64).round() as i64
                },
                count,
            })
            .collect()
    }

    fn text_attribute_options(&self) -> Vec<TextAttributeOption> {
        let mut options = BTreeMap::<(String, String), BTreeSet<String>>::new();

        for event in &self.events {
            for attribute in &event.attributes {
                if !matches!(attribute.value, AttrValue::String(_)) {
                    continue;
                }
                options
                    .entry((
                        "event".to_owned(),
                        self.pool.resolve(attribute.name).to_owned(),
                    ))
                    .or_default()
                    .insert(self.attr_value_label(&attribute.value));
            }
        }

        for object in &self.objects {
            for attribute in &object.attributes {
                if !matches!(attribute.value, AttrValue::String(_)) {
                    continue;
                }
                options
                    .entry((
                        "object".to_owned(),
                        self.pool.resolve(attribute.name).to_owned(),
                    ))
                    .or_default()
                    .insert(self.attr_value_label(&attribute.value));
            }
        }

        options
            .into_iter()
            .map(|((scope, name), values)| TextAttributeOption {
                scope,
                name,
                values: values.into_iter().take(200).collect(),
            })
            .collect()
    }

    fn object_satisfies_filter(
        &self,
        object_index: usize,
        object: &Object,
        filter: &OcelFilterRequest,
    ) -> bool {
        filter.df_nodes.iter().all(|activity| {
            object.lifecycle.iter().any(|event_index| {
                self.pool.resolve(self.events[*event_index].type_name) == activity
            })
        }) && filter.df_edges.iter().all(|edge| {
            object.lifecycle.windows(2).any(|pair| {
                let [source_index, target_index] = pair else {
                    return false;
                };
                self.pool.resolve(self.events[*source_index].type_name) == edge.source
                    && self.pool.resolve(self.events[*target_index].type_name) == edge.target
            })
        }) && filter
            .text_attributes
            .iter()
            .all(|attribute_filter| self.object_matches_text_attribute(object, attribute_filter))
            && filter.patterns.iter().all(|pattern_filter| {
                self.object_matches_pattern_filter(object_index, object, pattern_filter)
            })
    }

    fn object_matches_text_attribute(
        &self,
        object: &Object,
        attribute_filter: &TextAttributeFilter,
    ) -> bool {
        if attribute_filter.values.is_empty() {
            return true;
        }
        let values = attribute_filter
            .values
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();

        if attribute_filter.scope == "object" {
            return object.attributes.iter().any(|attribute| {
                self.pool.resolve(attribute.name) == attribute_filter.name
                    && values.contains(self.attr_value_label(&attribute.value).as_str())
            });
        }

        object.lifecycle.iter().any(|event_index| {
            self.events[*event_index]
                .attributes
                .iter()
                .any(|attribute| {
                    self.pool.resolve(attribute.name) == attribute_filter.name
                        && values.contains(self.attr_value_label(&attribute.value).as_str())
                })
        })
    }

    fn object_matches_pattern_filter(
        &self,
        object_index: usize,
        object: &Object,
        pattern_filter: &PatternFilter,
    ) -> bool {
        if pattern_filter.leading_object_type.is_empty()
            || self.pool.resolve(object.type_name) != pattern_filter.leading_object_type
        {
            return false;
        }

        let Some(state_attribute) = self.symbol_for_value("state") else {
            return false;
        };
        let state_lifecycle = object
            .lifecycle
            .iter()
            .filter_map(|event_index| {
                self.event_state(&self.events[*event_index], state_attribute)
                    .map(|state| (*event_index, state.to_owned()))
            })
            .collect::<Vec<_>>();
        if state_lifecycle.is_empty() {
            return false;
        }

        let episodes = state_episodes(&state_lifecycle);
        match pattern_filter.family.as_str() {
            "intra" => episodes.iter().any(|episode| {
                let instance = self.pattern_instance(
                    PatternFamily::Intra,
                    object_index,
                    episode.state.clone(),
                    None,
                    self.intra_sequence(&state_lifecycle, episode),
                    &state_lifecycle[episode.start..=episode.end],
                );
                pattern_filter_matches_instance(pattern_filter, &instance)
            }),
            "inter" => episodes.windows(2).any(|episode_pair| {
                let [left, right] = episode_pair else {
                    return false;
                };
                if left.state == right.state {
                    return false;
                }

                let mut segment_events = Vec::with_capacity(right.end - left.start + 1);
                segment_events.extend_from_slice(&state_lifecycle[left.start..=left.end]);
                segment_events.extend_from_slice(&state_lifecycle[right.start..=right.end]);
                let instance = self.pattern_instance(
                    PatternFamily::Inter,
                    object_index,
                    left.state.clone(),
                    Some(right.state.clone()),
                    self.inter_sequence(&state_lifecycle, left, right),
                    &segment_events,
                );
                pattern_filter_matches_instance(pattern_filter, &instance)
            }),
            _ => false,
        }
    }

    fn object_type_symbol(&self, object_type: &str) -> Option<Symbol> {
        self.object_types
            .iter()
            .find(|type_def| self.pool.resolve(type_def.name) == object_type)
            .map(|type_def| type_def.name)
    }

    fn lifecycle_json(&self, object_id: &str) -> OcelResult<String> {
        let lookup = self
            .pool
            .values
            .iter()
            .position(|value| value == object_id)
            .map(|index| Symbol(index as u32))
            .and_then(|symbol| self.object_index.get(&symbol).copied());

        let object_index = lookup.ok_or_else(|| {
            OcelError::new(format!("object id '{object_id}' was not found in the log"))
        })?;

        let event_ids: Vec<&str> = self.objects[object_index]
            .lifecycle
            .iter()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .collect();
        serde_json::to_string(&event_ids)
            .map_err(|err| OcelError::new(format!("could not serialize lifecycle: {err}")))
    }

    fn apply_state_query(&mut self, query: &str) -> OcelResult<String> {
        let state_query = StateQuery::parse(query)?;
        let leading_type_symbol = self
            .object_type_symbol(&state_query.leading_object_type)
            .ok_or_else(|| {
                OcelError::new(format!(
                    "unknown leading object type '{}'",
                    state_query.leading_object_type
                ))
            })?;
        let eval_index = StateEvalIndex::build(self, &state_query);
        let attribute_symbol = self.pool.intern(&state_query.attribute_name);
        self.ensure_event_attribute(attribute_symbol, AttrType::String);
        self.state_leading_object_type = Some(leading_type_symbol);

        for event in &mut self.events {
            event
                .attributes
                .retain(|attribute| attribute.name != attribute_symbol);
        }

        let mut assigned = 0usize;
        for event_index in 0..self.events.len() {
            if let Some(state) = self.evaluate_state_query(&state_query, &eval_index, event_index) {
                let state_symbol = self.pool.intern(&state);
                let event = &mut self.events[event_index];
                event.attributes.push(Attribute {
                    name: attribute_symbol,
                    value: AttrValue::String(state_symbol),
                });
                assigned += 1;
            }
        }

        let result = StateQueryResult {
            attribute: state_query.attribute_name,
            leading_object_type: state_query.leading_object_type,
            assigned_events: assigned,
            total_events: self.events.len(),
        };
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize state query result: {err}")))
    }

    fn state_detection_state_assignments(
        &self,
        request: &StateDetectionRequest,
    ) -> OcelResult<StateDetectionStateAssignments> {
        let run = self.compute_state_detection_run(request)?;
        let mut event_votes = vec![StateDetectionEventVote::default(); self.events.len()];

        for (window_index, (window, cell)) in run
            .windows
            .iter()
            .zip(run.som.assignments.iter())
            .enumerate()
        {
            for event_index in &window.event_indices {
                event_votes[*event_index].add(*cell, window_index);
            }
        }

        let states = event_votes
            .into_iter()
            .enumerate()
            .filter_map(|(event_index, vote)| {
                vote.winning_cell().map(|cell| StateDetectionEventState {
                    event_id: self.events[event_index].id,
                    state: cell_label(cell.0, cell.1),
                })
            })
            .collect();

        Ok(StateDetectionStateAssignments {
            leading_object_type: request.object_type.clone(),
            states,
        })
    }

    fn apply_state_labels_by_event_id(
        &mut self,
        leading_object_type: &str,
        states: &[StateDetectionEventState],
    ) -> OcelResult<usize> {
        let leading_type_symbol =
            self.object_type_symbol(leading_object_type)
                .ok_or_else(|| {
                    OcelError::new(format!(
                        "unknown leading object type '{leading_object_type}'"
                    ))
                })?;
        let attribute_symbol = self.pool.intern("state");
        self.ensure_event_attribute(attribute_symbol, AttrType::String);
        self.state_leading_object_type = Some(leading_type_symbol);

        for event in &mut self.events {
            event
                .attributes
                .retain(|attribute| attribute.name != attribute_symbol);
        }

        let event_index_by_id = self
            .events
            .iter()
            .enumerate()
            .map(|(event_index, event)| (event.id, event_index))
            .collect::<HashMap<_, _>>();

        let mut assigned = 0usize;
        for assignment in states {
            let Some(event_index) = event_index_by_id.get(&assignment.event_id) else {
                continue;
            };
            let state_symbol = self.pool.intern(&assignment.state);
            self.events[*event_index].attributes.push(Attribute {
                name: attribute_symbol,
                value: AttrValue::String(state_symbol),
            });
            assigned += 1;
        }

        Ok(assigned)
    }

    fn state_patterns_json(&self) -> OcelResult<String> {
        let analysis = self.detect_state_patterns()?;
        serde_json::to_string(&analysis).map_err(|err| {
            OcelError::new(format!("could not serialize state pattern analysis: {err}"))
        })
    }

    fn state_detection_json(&self, request: &StateDetectionRequest) -> OcelResult<String> {
        let analysis = self.detect_execution_states(request)?;
        serde_json::to_string(&analysis)
            .map_err(|err| OcelError::new(format!("could not serialize state detection: {err}")))
    }

    fn state_detection_cell_json(&self, request: &StateDetectionCellRequest) -> OcelResult<String> {
        let state_request = StateDetectionRequest {
            object_type: request.object_type.clone(),
            window_size: request.window_size,
            som_width: request.som_width,
            som_height: request.som_height,
            epochs: request.epochs,
            color_attribute: request.color_attribute.clone(),
        };
        let run = self.compute_state_detection_run(&state_request)?;
        let detail = self.state_detection_cell_detail(&run, request.cell_x, request.cell_y)?;
        serde_json::to_string(&detail).map_err(|err| {
            OcelError::new(format!("could not serialize state detection cell: {err}"))
        })
    }

    fn state_feature_table_csv(&self, request: &StateDetectionRequest) -> OcelResult<String> {
        let table = self.state_feature_table(&request.object_type)?;
        Ok(feature_table_to_csv(&table))
    }

    fn state_correlations_json(&self) -> OcelResult<String> {
        let analysis = self.state_correlations()?;
        serde_json::to_string(&analysis)
            .map_err(|err| OcelError::new(format!("could not serialize state correlations: {err}")))
    }

    fn time_perspective_json(&self, request: &TimePerspectiveRequest) -> OcelResult<String> {
        let analysis = self.time_perspective(request)?;
        serde_json::to_string(&analysis)
            .map_err(|err| OcelError::new(format!("could not serialize time perspective: {err}")))
    }

    fn state_transition_kpis_json(
        &self,
        request: &StateTransitionKpiRequest,
    ) -> OcelResult<String> {
        let analysis = self.state_transition_kpis(request)?;
        serde_json::to_string(&analysis).map_err(|err| {
            OcelError::new(format!("could not serialize state transition KPIs: {err}"))
        })
    }

    fn object_search_json(&self, request: &ObjectSearchRequest) -> OcelResult<String> {
        let result = self.object_search(request);
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize object search: {err}")))
    }

    fn object_lifecycle_detail_json(&self, object_id: &str) -> OcelResult<String> {
        let detail = self.object_lifecycle_detail(object_id)?;
        serde_json::to_string(&detail)
            .map_err(|err| OcelError::new(format!("could not serialize object lifecycle: {err}")))
    }

    fn causal_feature_table_json(&self, request: &CausalFeatureTableRequest) -> OcelResult<String> {
        let table = self.state_feature_table(&request.object_type)?;
        let result = CausalFeatureTableResult {
            object_type: request.object_type.clone(),
            object_count: table.rows.len(),
            feature_count: table.columns.len(),
            feature_columns: table.columns.clone(),
            table_preview: table
                .rows
                .iter()
                .take(15)
                .map(|row| FeaturePreviewRow {
                    object_id: row.object_id.clone(),
                    values: row.values.clone(),
                })
                .collect(),
        };
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize causal features: {err}")))
    }

    fn causal_feature_table_csv(&self, request: &CausalFeatureTableRequest) -> OcelResult<String> {
        let table = self.state_feature_table(&request.object_type)?;
        Ok(feature_table_to_csv(&table))
    }

    fn state_transition_kpis(
        &self,
        request: &StateTransitionKpiRequest,
    ) -> OcelResult<StateTransitionKpiResult> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;
        let object_type_symbol = request
            .object_type
            .as_deref()
            .and_then(|object_type| self.symbol_for_value(object_type))
            .or(self.state_leading_object_type)
            .or_else(|| self.objects.first().map(|object| object.type_name))
            .ok_or_else(|| OcelError::new("no object type is available for transition KPIs"))?;
        let object_type = self.pool.resolve(object_type_symbol).to_owned();
        let stuck_limit = request.stuck_limit.unwrap_or(15).clamp(1, 100);

        let mut states = BTreeSet::<String>::new();
        let mut transition_counts = BTreeMap::<(String, String), TransitionAccumulator>::new();
        let mut transition_objects = BTreeMap::<(String, String), BTreeSet<String>>::new();
        let mut dwell_durations = BTreeMap::<String, Vec<i64>>::new();
        let mut dwell_objects = BTreeMap::<String, BTreeSet<String>>::new();
        let mut stateful_object_count = 0usize;
        let mut stuck = Vec::<StuckStateRow>::new();

        for object in self
            .objects
            .iter()
            .filter(|object| object.type_name == object_type_symbol)
        {
            let object_id = self.pool.resolve(object.id).to_owned();
            let stateful_lifecycle = self.stateful_lifecycle(object, state_attribute);
            if stateful_lifecycle.is_empty() {
                continue;
            }
            stateful_object_count += 1;
            for (_, state) in &stateful_lifecycle {
                states.insert(state.clone());
            }

            let episodes = state_episodes(&stateful_lifecycle);
            for (episode_index, episode) in episodes.iter().enumerate() {
                let start_event_index = stateful_lifecycle[episode.start].0;
                let end_event_index = stateful_lifecycle[episode.end].0;
                let next_event_index = episodes
                    .get(episode_index + 1)
                    .map(|next| stateful_lifecycle[next.start].0);
                let start_time_ms = self.events[start_event_index].time_ms;
                let end_time_ms = next_event_index
                    .map(|index| self.events[index].time_ms)
                    .unwrap_or(self.events[end_event_index].time_ms);
                let duration_ms = (end_time_ms - start_time_ms).max(0);
                dwell_durations
                    .entry(episode.state.clone())
                    .or_default()
                    .push(duration_ms);
                dwell_objects
                    .entry(episode.state.clone())
                    .or_default()
                    .insert(object_id.clone());

                if episode_index + 1 == episodes.len() {
                    stuck.push(StuckStateRow {
                        object_id: object_id.clone(),
                        state: episode.state.clone(),
                        entered_time_ms: start_time_ms,
                        last_time_ms: self.events[end_event_index].time_ms,
                        duration_ms,
                        event_count: episode.end - episode.start + 1,
                    });
                }
            }

            for pair in episodes.windows(2) {
                let from = &pair[0];
                let to = &pair[1];
                let from_start_index = stateful_lifecycle[from.start].0;
                let to_start_index = stateful_lifecycle[to.start].0;
                let duration_ms = (self.events[to_start_index].time_ms
                    - self.events[from_start_index].time_ms)
                    .max(0);
                let key = (from.state.clone(), to.state.clone());
                transition_counts
                    .entry(key.clone())
                    .or_default()
                    .durations
                    .push(duration_ms);
                transition_objects
                    .entry(key)
                    .or_default()
                    .insert(object_id.clone());
            }
        }

        let mut transitions = transition_counts
            .into_iter()
            .map(|((from_state, to_state), accumulator)| {
                let stats = duration_stats(accumulator.durations);
                let object_count = transition_objects
                    .get(&(from_state.clone(), to_state.clone()))
                    .map_or(0, BTreeSet::len);
                StateTransitionKpiRow {
                    from_state,
                    to_state,
                    count: stats.sample_count,
                    object_count,
                    min_duration_ms: stats.min_duration_ms,
                    median_duration_ms: stats.median_duration_ms,
                    avg_duration_ms: stats.avg_duration_ms,
                    max_duration_ms: stats.max_duration_ms,
                }
            })
            .collect::<Vec<_>>();
        transitions.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.from_state.cmp(&right.from_state))
                .then_with(|| left.to_state.cmp(&right.to_state))
        });

        let mut dwell = dwell_durations
            .into_iter()
            .map(|(state, durations)| {
                let total_duration_ms = durations.iter().sum::<i64>();
                let stats = duration_stats(durations);
                StateDwellKpiRow {
                    state: state.clone(),
                    episode_count: stats.sample_count,
                    object_count: dwell_objects.get(&state).map_or(0, BTreeSet::len),
                    total_duration_ms,
                    min_duration_ms: stats.min_duration_ms,
                    median_duration_ms: stats.median_duration_ms,
                    avg_duration_ms: stats.avg_duration_ms,
                    max_duration_ms: stats.max_duration_ms,
                }
            })
            .collect::<Vec<_>>();
        dwell.sort_by(|left, right| {
            right
                .total_duration_ms
                .cmp(&left.total_duration_ms)
                .then_with(|| left.state.cmp(&right.state))
        });

        let mut recovery = transitions
            .iter()
            .filter(|transition| {
                is_recovery_transition(&transition.from_state, &transition.to_state)
            })
            .cloned()
            .collect::<Vec<_>>();
        recovery.sort_by(|left, right| {
            right
                .median_duration_ms
                .unwrap_or(0)
                .cmp(&left.median_duration_ms.unwrap_or(0))
                .then_with(|| right.count.cmp(&left.count))
        });

        stuck.sort_by(|left, right| {
            right
                .duration_ms
                .cmp(&left.duration_ms)
                .then_with(|| left.object_id.cmp(&right.object_id))
        });
        stuck.truncate(stuck_limit);

        let object_count = self
            .objects
            .iter()
            .filter(|object| object.type_name == object_type_symbol)
            .count();

        Ok(StateTransitionKpiResult {
            object_type,
            object_count,
            stateful_object_count,
            state_count: states.len(),
            states: states.into_iter().collect(),
            transitions,
            dwell,
            recovery,
            stuck,
        })
    }

    fn object_search(&self, request: &ObjectSearchRequest) -> ObjectSearchResult {
        let query = request
            .query
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let object_type_symbol = request
            .object_type
            .as_deref()
            .and_then(|object_type| self.symbol_for_value(object_type));
        let limit = request.limit.unwrap_or(30).clamp(1, 200);
        let mut matches = Vec::new();

        for object in &self.objects {
            if object_type_symbol.is_some_and(|symbol| object.type_name != symbol) {
                continue;
            }
            let object_id = self.pool.resolve(object.id);
            if !query.is_empty() && !object_id.to_ascii_lowercase().contains(&query) {
                continue;
            }
            matches.push(ObjectSearchHit {
                object_id: object_id.to_owned(),
                object_type: self.pool.resolve(object.type_name).to_owned(),
                event_count: object.lifecycle.len(),
            });
            if matches.len() >= limit {
                break;
            }
        }

        ObjectSearchResult { objects: matches }
    }

    fn object_lifecycle_detail(&self, object_id: &str) -> OcelResult<ObjectLifecycleDetail> {
        let lookup = self
            .symbol_for_value(object_id)
            .and_then(|symbol| self.object_index.get(&symbol).copied());
        let object_index = lookup.ok_or_else(|| {
            OcelError::new(format!(
                "object id '{object_id}' was not found in the active log"
            ))
        })?;
        let object = &self.objects[object_index];
        let state_attribute = self.symbol_for_value("state");
        let mut events = Vec::new();
        let mut stock_points = Vec::new();
        let mut related = BTreeMap::<(Symbol, String), LifecycleRelatedObjectAccumulator>::new();

        for event_index in &object.lifecycle {
            let event = &self.events[*event_index];
            let state = state_attribute.and_then(|symbol| self.event_state(event, symbol));
            let mut related_objects = Vec::new();
            for relationship in &event.relationships {
                if relationship.object_id == object.id {
                    continue;
                }
                let Some(related_index) = self.object_index.get(&relationship.object_id).copied()
                else {
                    continue;
                };
                let related_object = &self.objects[related_index];
                let qualifier = self.pool.resolve(relationship.qualifier).to_owned();
                related_objects.push(LifecycleRelatedObject {
                    object_id: self.pool.resolve(relationship.object_id).to_owned(),
                    object_type: self.pool.resolve(related_object.type_name).to_owned(),
                    qualifier: qualifier.clone(),
                });
                let entry = related
                    .entry((relationship.object_id, qualifier.clone()))
                    .or_insert_with(|| LifecycleRelatedObjectAccumulator {
                        object_type: self.pool.resolve(related_object.type_name).to_owned(),
                        qualifier,
                        event_count: 0,
                    });
                entry.event_count += 1;
            }

            for attribute in &event.attributes {
                let name = self.pool.resolve(attribute.name);
                if !name.to_ascii_lowercase().contains("stock") {
                    continue;
                }
                if let Some(value) = numeric_attr_value(&attribute.value) {
                    stock_points.push(LifecycleStockPoint {
                        name: name.to_owned(),
                        time_ms: event.time_ms,
                        value,
                        event_id: self.pool.resolve(event.id).to_owned(),
                    });
                }
            }

            events.push(LifecycleEventDetail {
                event_id: self.pool.resolve(event.id).to_owned(),
                event_type: self.pool.resolve(event.type_name).to_owned(),
                time_ms: event.time_ms,
                state: state.map(str::to_owned),
                attributes: event
                    .attributes
                    .iter()
                    .map(|attribute| {
                        Ok(LifecycleAttribute {
                            name: self.pool.resolve(attribute.name).to_owned(),
                            value: self.attr_value_to_json(&attribute.value)?,
                        })
                    })
                    .collect::<OcelResult<Vec<_>>>()?,
                related_objects,
            });
        }

        let state_bands = lifecycle_state_bands(&events);
        let related_objects = related
            .into_iter()
            .map(
                |((object_symbol, _), accumulator)| LifecycleRelatedObjectSummary {
                    object_id: self.pool.resolve(object_symbol).to_owned(),
                    object_type: accumulator.object_type,
                    qualifier: accumulator.qualifier,
                    event_count: accumulator.event_count,
                },
            )
            .collect::<Vec<_>>();

        Ok(ObjectLifecycleDetail {
            object_id: self.pool.resolve(object.id).to_owned(),
            object_type: self.pool.resolve(object.type_name).to_owned(),
            event_count: events.len(),
            event_min_ms: events.first().map(|event| event.time_ms),
            event_max_ms: events.last().map(|event| event.time_ms),
            events,
            state_bands,
            stock_points,
            related_objects,
        })
    }

    fn stateful_lifecycle(&self, object: &Object, state_attribute: Symbol) -> Vec<(usize, String)> {
        object
            .lifecycle
            .iter()
            .filter_map(|event_index| {
                let event = &self.events[*event_index];
                self.event_state(event, state_attribute)
                    .map(|state| (*event_index, state.to_owned()))
            })
            .collect()
    }

    fn time_perspective(
        &self,
        request: &TimePerspectiveRequest,
    ) -> OcelResult<TimePerspectiveResult> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;
        let leading_type_symbol = request
            .object_type
            .as_deref()
            .and_then(|object_type| self.symbol_for_value(object_type))
            .or(self.state_leading_object_type)
            .or_else(|| self.objects.first().map(|object| object.type_name))
            .ok_or_else(|| OcelError::new("no object type is available for time perspective"))?;
        let object_type = self.pool.resolve(leading_type_symbol).to_owned();

        let mut event_min_ms = None::<i64>;
        let mut event_max_ms = None::<i64>;
        let mut states = BTreeSet::<String>::new();
        let mut stateful_events = Vec::<(i64, String)>::new();
        for event in &self.events {
            event_min_ms = Some(event_min_ms.map_or(event.time_ms, |min| min.min(event.time_ms)));
            event_max_ms = Some(event_max_ms.map_or(event.time_ms, |max| max.max(event.time_ms)));
            if let Some(state) = self.event_state(event, state_attribute) {
                states.insert(state.to_owned());
                stateful_events.push((event.time_ms, state.to_owned()));
            }
        }

        let event_min_ms = event_min_ms.unwrap_or(0);
        let event_max_ms = event_max_ms.unwrap_or(event_min_ms);
        let buckets = request.buckets.unwrap_or(24).clamp(4, 96);
        let mut bucket_counts = vec![BTreeMap::<String, usize>::new(); buckets];
        let mut bucket_totals = vec![0usize; buckets];
        let span = (event_max_ms - event_min_ms).max(1) as f64;
        for (time_ms, state) in stateful_events {
            let ratio = ((time_ms - event_min_ms) as f64 / span).clamp(0.0, 1.0);
            let bucket_index = ((ratio * buckets as f64).floor() as usize).min(buckets - 1);
            *bucket_counts[bucket_index].entry(state).or_default() += 1;
            bucket_totals[bucket_index] += 1;
        }
        let bucket_width = span / buckets as f64;
        let frequency_buckets = bucket_counts
            .into_iter()
            .enumerate()
            .map(|(index, counts)| {
                let total = bucket_totals[index];
                let start_ms = event_min_ms + (bucket_width * index as f64).round() as i64;
                let end_ms = if index + 1 == buckets {
                    event_max_ms
                } else {
                    event_min_ms + (bucket_width * (index + 1) as f64).round() as i64
                };
                let percentages = states
                    .iter()
                    .map(|state| TimeStatePercentage {
                        state: state.clone(),
                        percentage: if total == 0 {
                            0.0
                        } else {
                            (*counts.get(state).unwrap_or(&0) as f64 / total as f64) * 100.0
                        },
                        count: *counts.get(state).unwrap_or(&0),
                    })
                    .collect();
                TimeFrequencyBucket {
                    start_ms,
                    end_ms,
                    total,
                    percentages,
                }
            })
            .collect();

        let most_frequent_transition =
            self.most_frequent_state_transition(leading_type_symbol, state_attribute);
        let from_state = request
            .from_state
            .clone()
            .or_else(|| {
                most_frequent_transition
                    .as_ref()
                    .map(|(from, _)| from.clone())
            })
            .or_else(|| states.iter().next().cloned());
        let to_state = request
            .to_state
            .clone()
            .filter(|state| Some(state) != from_state.as_ref())
            .or_else(|| {
                most_frequent_transition
                    .as_ref()
                    .and_then(|(_, to)| (Some(to) != from_state.as_ref()).then(|| to.clone()))
            })
            .or_else(|| {
                states
                    .iter()
                    .find(|state| Some(*state) != from_state.as_ref())
                    .cloned()
            });
        let performance = match (from_state, to_state) {
            (Some(from_state), Some(to_state)) => self.state_transition_performance(
                leading_type_symbol,
                state_attribute,
                &from_state,
                &to_state,
                request.roundtrip,
            ),
            _ => TimePerformanceSpectrum {
                object_type: object_type.clone(),
                from_state: String::new(),
                to_state: String::new(),
                roundtrip: request.roundtrip,
                sample_count: 0,
                min_duration_ms: None,
                median_duration_ms: None,
                avg_duration_ms: None,
                max_duration_ms: None,
                samples: Vec::new(),
            },
        };

        Ok(TimePerspectiveResult {
            object_type,
            event_min_ms,
            event_max_ms,
            states: states.into_iter().collect(),
            buckets: frequency_buckets,
            performance,
        })
    }

    fn state_transition_performance(
        &self,
        object_type: Symbol,
        state_attribute: Symbol,
        from_state: &str,
        to_state: &str,
        roundtrip: bool,
    ) -> TimePerformanceSpectrum {
        let mut samples = Vec::new();
        for object in self
            .objects
            .iter()
            .filter(|object| object.type_name == object_type)
        {
            let stateful_lifecycle = object
                .lifecycle
                .iter()
                .filter_map(|event_index| {
                    let event = &self.events[*event_index];
                    self.event_state(event, state_attribute)
                        .map(|state| (*event_index, state.to_owned()))
                })
                .collect::<Vec<_>>();

            for (start_pos, (start_index, state)) in stateful_lifecycle.iter().enumerate() {
                if state != from_state {
                    continue;
                }
                let Some((middle_pos, (middle_index, _))) = stateful_lifecycle
                    .iter()
                    .enumerate()
                    .skip(start_pos + 1)
                    .find(|(_, (_, candidate))| candidate == to_state)
                else {
                    continue;
                };
                let end_index = if roundtrip {
                    let Some((end_index, _)) = stateful_lifecycle
                        .iter()
                        .skip(middle_pos + 1)
                        .find(|(_, candidate)| candidate == from_state)
                    else {
                        continue;
                    };
                    Some(*end_index)
                } else {
                    None
                };
                let start_time_ms = self.events[*start_index].time_ms;
                let middle_time_ms = self.events[*middle_index].time_ms;
                let end_time_ms = end_index.map(|index| self.events[index].time_ms);
                let duration_ms = end_time_ms.unwrap_or(middle_time_ms) - start_time_ms;
                if duration_ms < 0 {
                    continue;
                }
                samples.push(TimePerformanceSample {
                    object_id: self.pool.resolve(object.id).to_owned(),
                    start_time_ms,
                    middle_time_ms,
                    end_time_ms,
                    duration_ms,
                });
            }
        }

        samples.sort_by_key(|sample| sample.duration_ms);
        let durations = samples
            .iter()
            .map(|sample| sample.duration_ms)
            .collect::<Vec<_>>();
        let sample_count = samples.len();
        let avg_duration_ms = if durations.is_empty() {
            None
        } else {
            Some(
                durations
                    .iter()
                    .map(|duration| *duration as f64)
                    .sum::<f64>()
                    / durations.len() as f64,
            )
        };
        let median_duration_ms = if durations.is_empty() {
            None
        } else {
            Some(durations[durations.len() / 2])
        };

        TimePerformanceSpectrum {
            object_type: self.pool.resolve(object_type).to_owned(),
            from_state: from_state.to_owned(),
            to_state: to_state.to_owned(),
            roundtrip,
            sample_count,
            min_duration_ms: durations.first().copied(),
            median_duration_ms,
            avg_duration_ms,
            max_duration_ms: durations.last().copied(),
            samples,
        }
    }

    fn most_frequent_state_transition(
        &self,
        object_type: Symbol,
        state_attribute: Symbol,
    ) -> Option<(String, String)> {
        let mut counts = BTreeMap::<(String, String), usize>::new();
        for object in self
            .objects
            .iter()
            .filter(|object| object.type_name == object_type)
        {
            let states = object
                .lifecycle
                .iter()
                .filter_map(|event_index| {
                    self.event_state(&self.events[*event_index], state_attribute)
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>();
            for pair in states.windows(2) {
                if pair[0] == pair[1] {
                    continue;
                }
                *counts
                    .entry((pair[0].clone(), pair[1].clone()))
                    .or_default() += 1;
            }
        }

        counts
            .into_iter()
            .max_by(|(left_pair, left_count), (right_pair, right_count)| {
                left_count
                    .cmp(right_count)
                    .then_with(|| right_pair.cmp(left_pair))
            })
            .map(|(pair, _)| pair)
    }

    fn state_correlations(&self) -> OcelResult<StateCorrelationResult> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;
        let leading_type_symbol = self.state_leading_object_type.ok_or_else(|| {
            OcelError::new(
                "state leading object type is unknown; apply a state query or state detection first",
            )
        })?;
        let object_type = self.pool.resolve(leading_type_symbol).to_owned();
        let feature_table = self.state_feature_table(&object_type)?;
        let state_by_object = self
            .objects
            .iter()
            .filter(|object| object.type_name == leading_type_symbol)
            .filter_map(|object| {
                self.dominant_object_state(object, state_attribute)
                    .map(|state| (self.pool.resolve(object.id).to_owned(), state))
            })
            .collect::<HashMap<_, _>>();

        if state_by_object.is_empty() {
            return Err(OcelError::new(format!(
                "no active '{object_type}' objects have stateful events"
            )));
        }

        let mut state_distribution = BTreeMap::<String, usize>::new();
        let row_states = feature_table
            .rows
            .iter()
            .map(|row| {
                let state = state_by_object.get(&row.object_id).cloned();
                if let Some(state) = &state {
                    *state_distribution.entry(state.clone()).or_default() += 1;
                }
                state
            })
            .collect::<Vec<_>>();

        let stateful_object_count = row_states.iter().filter(|state| state.is_some()).count();
        if stateful_object_count == 0 {
            return Err(OcelError::new(format!(
                "no active '{object_type}' feature rows can be matched to stateful objects"
            )));
        }

        let mut rows = Vec::with_capacity(feature_table.columns.len());
        for (column_index, feature) in feature_table.columns.iter().enumerate() {
            rows.push(state_feature_correlation_row(
                feature,
                column_index,
                &feature_table.rows,
                &row_states,
            ));
        }
        rows.sort_by(|left, right| {
            right
                .strength
                .partial_cmp(&left.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.feature.cmp(&right.feature))
        });

        Ok(StateCorrelationResult {
            object_type,
            object_count: feature_table.rows.len(),
            stateful_object_count,
            state_count: state_distribution.len(),
            feature_count: feature_table.columns.len(),
            state_distribution: state_distribution
                .into_iter()
                .map(|(state, count)| StateCorrelationStateCount { state, count })
                .collect(),
            rows,
        })
    }

    fn fit_causal_model_json(&self, request: &CausalModelFitRequest) -> OcelResult<String> {
        let fit = self.fit_causal_model(request)?;
        serde_json::to_string(&fit)
            .map_err(|err| OcelError::new(format!("could not serialize causal model fit: {err}")))
    }

    fn fit_causal_model(
        &self,
        request: &CausalModelFitRequest,
    ) -> OcelResult<CausalModelFitResult> {
        let table = self.state_feature_table(&request.object_type)?;
        if request.nodes.is_empty() {
            return Err(OcelError::new(
                "causal model must contain at least one node",
            ));
        }
        let node_by_id = validate_causal_nodes(&request.nodes, &table.columns)?;
        validate_causal_edges(&request.edges, &node_by_id)?;
        let order = causal_topological_order(&request.nodes, &request.edges)?;
        let feature_index = table
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| (column.as_str(), index))
            .collect::<HashMap<_, _>>();
        let row_count = table.rows.len();
        let mut vectors = HashMap::<String, Vec<f64>>::new();

        for node_id in order {
            let node = node_by_id
                .get(node_id.as_str())
                .expect("topological order contains validated nodes");
            match causal_role(&node.role)? {
                CausalNodeRole::Observable | CausalNodeRole::Outcome => {
                    let feature = node.feature.as_deref().ok_or_else(|| {
                        OcelError::new(format!("node '{}' must reference a feature", node.label))
                    })?;
                    let column_index = *feature_index.get(feature).ok_or_else(|| {
                        OcelError::new(format!("unknown causal feature '{feature}'"))
                    })?;
                    let values = table
                        .rows
                        .iter()
                        .map(|row| {
                            transform_causal_value(row.values[column_index], &node.operation)
                        })
                        .collect::<Vec<_>>();
                    vectors.insert(node.id.clone(), values);
                }
                CausalNodeRole::Latent => {
                    let parents = request
                        .edges
                        .iter()
                        .filter(|edge| edge.target == node.id)
                        .filter_map(|edge| vectors.get(&edge.source))
                        .map(|values| standardized_vector(values))
                        .collect::<Vec<_>>();
                    vectors.insert(node.id.clone(), average_vectors(&parents, row_count));
                }
            }
        }

        let nodes = request
            .nodes
            .iter()
            .map(|node| {
                let values = vectors.get(&node.id).cloned().unwrap_or_default();
                let (mean, std_dev) = vector_mean_std(&values);
                CausalFitNode {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    role: node.role.clone(),
                    feature: node.feature.clone(),
                    operation: normalized_causal_operation(&node.operation),
                    mean: round_f64(mean),
                    std_dev: round_f64(std_dev),
                }
            })
            .collect::<Vec<_>>();
        let edges = request
            .edges
            .iter()
            .map(|edge| {
                let source_values = vectors
                    .get(&edge.source)
                    .expect("edge source validated before fitting");
                let target_values = vectors
                    .get(&edge.target)
                    .expect("edge target validated before fitting");
                let (correlation, sample_count) = pearson_correlation(source_values, target_values);
                let intensity = correlation.abs();
                CausalFitEdge {
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    correlation: round_f64(correlation),
                    intensity: round_f64(intensity),
                    p_value: round_f64(approximate_correlation_p_value(correlation, sample_count)),
                    sample_count,
                }
            })
            .collect::<Vec<_>>();

        Ok(CausalModelFitResult {
            object_type: request.object_type.clone(),
            sample_count: row_count,
            nodes,
            edges,
        })
    }

    fn state_feature_table(&self, object_type: &str) -> OcelResult<FeatureTableData> {
        let object_type_symbol = self.object_type_symbol(object_type).ok_or_else(|| {
            OcelError::new(format!(
                "unknown object type '{object_type}' for state detection"
            ))
        })?;
        let object_indices = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| (object.type_name == object_type_symbol).then_some(index))
            .collect::<Vec<_>>();
        let encoder = self.build_feature_encoder(&object_indices);
        let columns = encoder.columns.iter().map(FeatureColumn::label).collect();
        let rows = object_indices
            .iter()
            .map(|object_index| {
                let object = &self.objects[*object_index];
                FeatureRow {
                    object_id: self.pool.resolve(object.id).to_owned(),
                    values: self.encode_feature_vector(
                        *object_index,
                        &object.lifecycle,
                        i64::MAX,
                        &encoder,
                    ),
                }
            })
            .collect();

        Ok(FeatureTableData { columns, rows })
    }

    fn detect_execution_states(
        &self,
        request: &StateDetectionRequest,
    ) -> OcelResult<StateDetectionResult> {
        let run = self.compute_state_detection_run(request)?;
        let window_size = request.window_size.unwrap_or(4).clamp(1, 30);
        let som_width = run.som.width;
        let som_height = run.som.weights.len() / som_width;
        let som_summary =
            self.summarize_som(&run.windows, &run.pca.points, &run.som, &run.color_metric);
        let feature_columns = run
            .encoder
            .columns
            .iter()
            .map(FeatureColumn::label)
            .collect::<Vec<_>>();
        let table_preview = run
            .feature_table
            .rows
            .iter()
            .take(15)
            .map(|row| FeaturePreviewRow {
                object_id: row.object_id.clone(),
                values: row.values.clone(),
            })
            .collect();
        let projected_windows = self.projected_windows(&run, 500);

        Ok(StateDetectionResult {
            object_type: request.object_type.clone(),
            window_size,
            som_width,
            som_height,
            color_attribute: run.color_metric.id(),
            color_attributes: run.color_options,
            object_count: run.object_indices.len(),
            feature_count: feature_columns.len(),
            window_count: run.windows.len(),
            feature_columns,
            table_preview,
            pca: PcaSummary {
                pc1_variance: round_f64(run.pca.pc1_variance),
                pc2_variance: round_f64(run.pca.pc2_variance),
                pc1_explained_ratio: round_f64(run.pca.pc1_explained_ratio),
                pc2_explained_ratio: round_f64(run.pca.pc2_explained_ratio),
            },
            som: som_summary,
            windows: projected_windows,
        })
    }

    fn compute_state_detection_run(
        &self,
        request: &StateDetectionRequest,
    ) -> OcelResult<StateDetectionRun> {
        let object_type_symbol =
            self.object_type_symbol(&request.object_type)
                .ok_or_else(|| {
                    OcelError::new(format!(
                        "unknown object type '{}' for state detection",
                        request.object_type
                    ))
                })?;
        let object_indices = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| (object.type_name == object_type_symbol).then_some(index))
            .collect::<Vec<_>>();
        if object_indices.is_empty() {
            return Err(OcelError::new(format!(
                "no objects of type '{}' are available in the active log",
                request.object_type
            )));
        }

        let encoder = self.build_feature_encoder(&object_indices);
        if encoder.columns.is_empty() {
            return Err(OcelError::new(format!(
                "no numerical features could be extracted for '{}'",
                request.object_type
            )));
        }

        let window_size = request.window_size.unwrap_or(4).clamp(1, 30);
        let windows = self.encode_lifecycle_windows(&object_indices, window_size, &encoder);
        if windows.is_empty() {
            return Err(OcelError::new(format!(
                "no lifecycle windows could be extracted for '{}'",
                request.object_type
            )));
        }

        let values = windows
            .iter()
            .map(|window| window.values.clone())
            .collect::<Vec<_>>();
        let pca = pca_project(&values);
        let (som_width, som_height) =
            default_som_dimensions(pca.points.len(), request.som_width, request.som_height);
        let som = train_som(
            &pca.points,
            som_width,
            som_height,
            request.epochs.unwrap_or(120).clamp(10, 500),
        );
        let feature_table = self.state_feature_table(&request.object_type)?;
        let color_options = self.state_detection_color_options(&object_indices);
        let color_metric =
            self.resolve_color_metric(request.color_attribute.as_deref(), &color_options);

        Ok(StateDetectionRun {
            object_indices,
            encoder,
            feature_table,
            windows,
            pca,
            som,
            color_metric,
            color_options,
        })
    }

    fn state_detection_color_options(
        &self,
        object_indices: &[usize],
    ) -> Vec<StateDetectionColorOption> {
        let mut attributes = BTreeMap::<String, AttributeFeatureCollector>::new();
        for object_index in object_indices {
            let object = &self.objects[*object_index];
            for attribute in &object.attributes {
                let entry = attributes
                    .entry(self.pool.resolve(attribute.name).to_owned())
                    .or_default();
                if attr_value_to_f64(&attribute.value).is_some() {
                    entry.has_numeric = true;
                } else {
                    entry
                        .categories
                        .insert(self.attr_value_label(&attribute.value));
                }
            }
        }

        let mut options = vec![StateDetectionColorOption {
            id: "__window_count".to_owned(),
            label: "Assigned windows".to_owned(),
            kind: "count",
        }];
        for (name, collector) in attributes {
            if collector.has_numeric && collector.categories.is_empty() {
                options.push(StateDetectionColorOption {
                    id: format!("attribute::{name}"),
                    label: name,
                    kind: "numeric",
                });
            } else if !collector.categories.is_empty() && collector.categories.len() < 50 {
                options.push(StateDetectionColorOption {
                    id: format!("attribute::{name}"),
                    label: name,
                    kind: "categorical",
                });
            }
        }
        options
    }

    fn resolve_color_metric(
        &self,
        requested: Option<&str>,
        options: &[StateDetectionColorOption],
    ) -> ColorMetric {
        let Some(requested) = requested else {
            return ColorMetric::WindowCount;
        };
        if requested == "__window_count" {
            return ColorMetric::WindowCount;
        }
        let Some(option) = options.iter().find(|option| option.id == requested) else {
            return ColorMetric::WindowCount;
        };
        let attribute_name = option
            .id
            .strip_prefix("attribute::")
            .unwrap_or(&option.label)
            .to_owned();
        match option.kind {
            "numeric" => ColorMetric::NumericAttribute(attribute_name),
            "categorical" => ColorMetric::CategoricalAttribute(attribute_name),
            _ => ColorMetric::WindowCount,
        }
    }

    fn projected_windows(
        &self,
        run: &StateDetectionRun,
        limit: usize,
    ) -> Vec<StateWindowProjection> {
        run.windows
            .iter()
            .zip(run.pca.points.iter())
            .zip(run.som.assignments.iter())
            .take(limit)
            .map(|((window, (pc1, pc2)), (cell_x, cell_y))| {
                let object = &self.objects[window.object_index];
                let first_event = window
                    .event_indices
                    .first()
                    .map(|event_index| self.pool.resolve(self.events[*event_index].id))
                    .unwrap_or("");
                let last_event = window
                    .event_indices
                    .last()
                    .map(|event_index| self.pool.resolve(self.events[*event_index].id))
                    .unwrap_or("");
                StateWindowProjection {
                    object_id: self.pool.resolve(object.id).to_owned(),
                    start_event: first_event.to_owned(),
                    end_event: last_event.to_owned(),
                    pc1: round_f64(*pc1),
                    pc2: round_f64(*pc2),
                    cell_x: *cell_x,
                    cell_y: *cell_y,
                }
            })
            .collect()
    }

    fn state_detection_cell_detail(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> OcelResult<StateDetectionCellDetail> {
        let height = run.som.weights.len() / run.som.width;
        if cell_x >= run.som.width || cell_y >= height {
            return Err(OcelError::new(format!(
                "SOM cell {},{} is outside the {}x{} grid",
                cell_x, cell_y, run.som.width, height
            )));
        }

        let som_summary =
            self.summarize_som(&run.windows, &run.pca.points, &run.som, &run.color_metric);
        let cell = som_summary
            .cells
            .into_iter()
            .find(|cell| cell.x == cell_x && cell.y == cell_y)
            .expect("validated SOM cell must exist");
        let dfg = self.state_detection_cell_dfg(run, cell_x, cell_y);
        let (entering_windows, exiting_windows, entering_indices, exiting_indices) =
            self.state_detection_boundary_windows(run, cell_x, cell_y);
        let entering_dfg = self.state_detection_windows_dfg(
            run,
            &entering_indices,
            format!("Entering Windows: {}", cell.label),
            "Directly-follows graph over windows entering the selected SOM cell".to_owned(),
        );
        let exiting_dfg = self.state_detection_windows_dfg(
            run,
            &exiting_indices,
            format!("Exiting Windows: {}", cell.label),
            "Directly-follows graph over windows exiting the selected SOM cell".to_owned(),
        );

        Ok(StateDetectionCellDetail {
            cell,
            dfg,
            entering_dfg,
            exiting_dfg,
            entering_window_count: entering_indices.len(),
            exiting_window_count: exiting_indices.len(),
            entering_windows,
            exiting_windows,
        })
    }

    fn state_detection_cell_dfg(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> LayoutGraph {
        let mut graph = GraphAccumulator::new(
            format!("State Detection Cell S{}-{}", cell_x + 1, cell_y + 1),
            "Directly-follows graph over windows assigned to the selected SOM cell".to_owned(),
        );
        let object_type = self
            .pool
            .resolve(self.objects[run.windows[0].object_index].type_name);

        for (window, (assigned_x, assigned_y)) in run.windows.iter().zip(run.som.assignments.iter())
        {
            if *assigned_x != cell_x || *assigned_y != cell_y || window.event_indices.is_empty() {
                continue;
            }
            self.accumulate_window_directly_follows(&mut graph, window, object_type);
        }

        layout_accumulated_graph(graph)
    }

    fn state_detection_windows_dfg(
        &self,
        run: &StateDetectionRun,
        window_indices: &[usize],
        title: String,
        subtitle: String,
    ) -> LayoutGraph {
        let mut graph = GraphAccumulator::new(title, subtitle);
        let object_type = self
            .pool
            .resolve(self.objects[run.windows[0].object_index].type_name);

        for window_index in window_indices {
            if let Some(window) = run.windows.get(*window_index) {
                self.accumulate_window_directly_follows(&mut graph, window, object_type);
            }
        }

        layout_accumulated_graph(graph)
    }

    fn accumulate_window_directly_follows(
        &self,
        graph: &mut GraphAccumulator,
        window: &WindowEncoding,
        object_type: &str,
    ) {
        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            window.event_indices.len() as f64 + 1.0,
            1,
        );

        for (position, event_index) in window.event_indices.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            graph.add_node(event_type, "activity", position as f64 + 1.0, 1);
        }

        if let Some(first_index) = window.event_indices.first() {
            let first = self.pool.resolve(self.events[*first_index].type_name);
            graph.add_edge(&start, first, object_type, 1);
        }
        for pair in window.event_indices.windows(2) {
            let [source_index, target_index] = pair else {
                continue;
            };
            let source = self.pool.resolve(self.events[*source_index].type_name);
            let target = self.pool.resolve(self.events[*target_index].type_name);
            graph.add_edge(source, target, object_type, 1);
        }
        if let Some(last_index) = window.event_indices.last() {
            let last = self.pool.resolve(self.events[*last_index].type_name);
            graph.add_edge(last, &end, object_type, 1);
        }
    }

    fn state_detection_boundary_windows(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> (
        Vec<StateDetectionBoundaryWindow>,
        Vec<StateDetectionBoundaryWindow>,
        Vec<usize>,
        Vec<usize>,
    ) {
        let mut entering = Vec::new();
        let mut exiting = Vec::new();
        let mut entering_indices = Vec::new();
        let mut exiting_indices = Vec::new();

        for index in 1..run.windows.len() {
            let previous = &run.windows[index - 1];
            let current = &run.windows[index];
            if previous.object_index != current.object_index {
                continue;
            }
            let source = run.som.assignments[index - 1];
            let target = run.som.assignments[index];
            if source == target {
                continue;
            }
            if target == (cell_x, cell_y) {
                if entering.len() < 100 {
                    entering.push(self.boundary_window_summary(run, index, source, target));
                }
                entering_indices.push(index);
            }
            if source == (cell_x, cell_y) {
                if exiting.len() < 100 {
                    exiting.push(self.boundary_window_summary(run, index - 1, source, target));
                }
                exiting_indices.push(index - 1);
            }
        }

        (entering, exiting, entering_indices, exiting_indices)
    }

    fn boundary_window_summary(
        &self,
        run: &StateDetectionRun,
        window_index: usize,
        source: (usize, usize),
        target: (usize, usize),
    ) -> StateDetectionBoundaryWindow {
        let window = &run.windows[window_index];
        let projection = self.projected_window(window, run.pca.points[window_index], target);
        StateDetectionBoundaryWindow {
            object_id: projection.object_id,
            start_event: projection.start_event,
            end_event: projection.end_event,
            source_cell: cell_label(source.0, source.1),
            target_cell: cell_label(target.0, target.1),
            pc1: projection.pc1,
            pc2: projection.pc2,
            activities: self.window_activity_sequence(window),
        }
    }

    fn projected_window(
        &self,
        window: &WindowEncoding,
        point: (f64, f64),
        cell: (usize, usize),
    ) -> StateWindowProjection {
        let object = &self.objects[window.object_index];
        let first_event = window
            .event_indices
            .first()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .unwrap_or("");
        let last_event = window
            .event_indices
            .last()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .unwrap_or("");
        StateWindowProjection {
            object_id: self.pool.resolve(object.id).to_owned(),
            start_event: first_event.to_owned(),
            end_event: last_event.to_owned(),
            pc1: round_f64(point.0),
            pc2: round_f64(point.1),
            cell_x: cell.0,
            cell_y: cell.1,
        }
    }

    fn window_activity_sequence(&self, window: &WindowEncoding) -> Vec<String> {
        window
            .event_indices
            .iter()
            .map(|event_index| {
                self.pool
                    .resolve(self.events[*event_index].type_name)
                    .to_owned()
            })
            .collect()
    }

    fn build_feature_encoder(&self, object_indices: &[usize]) -> FeatureEncoder {
        let mut activity_types = BTreeSet::<String>::new();
        let mut related_object_types = BTreeSet::<String>::new();
        let mut attributes = BTreeMap::<String, AttributeFeatureCollector>::new();

        for object_index in object_indices {
            let object = &self.objects[*object_index];
            for event_index in &object.lifecycle {
                let event = &self.events[*event_index];
                activity_types.insert(self.pool.resolve(event.type_name).to_owned());
                for relationship in &event.relationships {
                    if relationship.object_id == object.id {
                        continue;
                    }
                    if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                        related_object_types.insert(
                            self.pool
                                .resolve(self.objects[*related_index].type_name)
                                .to_owned(),
                        );
                    }
                }
            }

            for relationship in &object.relationships {
                if relationship.object_id == object.id {
                    continue;
                }
                if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                    related_object_types.insert(
                        self.pool
                            .resolve(self.objects[*related_index].type_name)
                            .to_owned(),
                    );
                }
            }

            for (name, value) in self.latest_attribute_values_at(object, i64::MAX) {
                let entry = attributes.entry(name).or_default();
                if attr_value_to_f64(value).is_some() {
                    entry.has_numeric = true;
                } else {
                    entry.categories.insert(self.attr_value_label(value));
                }
            }
        }

        let mut columns = Vec::new();
        columns.extend(
            activity_types
                .into_iter()
                .map(|event_type| FeatureColumn::Activity { event_type }),
        );
        columns.extend(
            related_object_types
                .into_iter()
                .map(|object_type| FeatureColumn::RelatedObjectType { object_type }),
        );
        for (name, collector) in attributes {
            if !collector.categories.is_empty() {
                if collector.categories.len() < 50 {
                    columns.extend(collector.categories.into_iter().map(|value| {
                        FeatureColumn::CategoricalAttribute {
                            name: name.clone(),
                            value,
                        }
                    }));
                }
            } else if collector.has_numeric {
                columns.push(FeatureColumn::NumericAttribute { name });
            }
        }

        FeatureEncoder { columns }
    }

    fn encode_lifecycle_windows(
        &self,
        object_indices: &[usize],
        window_size: usize,
        encoder: &FeatureEncoder,
    ) -> Vec<WindowEncoding> {
        let mut windows = Vec::new();
        for object_index in object_indices {
            let lifecycle = &self.objects[*object_index].lifecycle;
            if lifecycle.is_empty() {
                continue;
            }

            if lifecycle.len() <= window_size {
                let event_indices = lifecycle.clone();
                let end_time = event_indices
                    .last()
                    .map(|event_index| self.events[*event_index].time_ms)
                    .unwrap_or(i64::MAX);
                windows.push(WindowEncoding {
                    object_index: *object_index,
                    values: self.encode_feature_vector(
                        *object_index,
                        &event_indices,
                        end_time,
                        encoder,
                    ),
                    event_indices,
                });
                continue;
            }

            for start in 0..=lifecycle.len() - window_size {
                let event_indices = lifecycle[start..start + window_size].to_vec();
                let end_time = event_indices
                    .last()
                    .map(|event_index| self.events[*event_index].time_ms)
                    .unwrap_or(i64::MAX);
                windows.push(WindowEncoding {
                    object_index: *object_index,
                    values: self.encode_feature_vector(
                        *object_index,
                        &event_indices,
                        end_time,
                        encoder,
                    ),
                    event_indices,
                });
            }
        }
        windows
    }

    fn encode_feature_vector(
        &self,
        object_index: usize,
        event_indices: &[usize],
        attribute_time_ms: i64,
        encoder: &FeatureEncoder,
    ) -> Vec<f64> {
        let object = &self.objects[object_index];
        let mut activity_counts = BTreeMap::<String, f64>::new();
        let mut related_objects = BTreeMap::<String, BTreeSet<Symbol>>::new();

        for event_index in event_indices {
            let event = &self.events[*event_index];
            *activity_counts
                .entry(self.pool.resolve(event.type_name).to_owned())
                .or_default() += 1.0;
            for relationship in &event.relationships {
                if relationship.object_id == object.id {
                    continue;
                }
                if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                    related_objects
                        .entry(
                            self.pool
                                .resolve(self.objects[*related_index].type_name)
                                .to_owned(),
                        )
                        .or_default()
                        .insert(relationship.object_id);
                }
            }
        }

        for relationship in &object.relationships {
            if relationship.object_id == object.id {
                continue;
            }
            if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                related_objects
                    .entry(
                        self.pool
                            .resolve(self.objects[*related_index].type_name)
                            .to_owned(),
                    )
                    .or_default()
                    .insert(relationship.object_id);
            }
        }

        let attribute_values = self.latest_attribute_values_at(object, attribute_time_ms);
        encoder
            .columns
            .iter()
            .map(|column| match column {
                FeatureColumn::Activity { event_type } => {
                    *activity_counts.get(event_type).unwrap_or(&0.0)
                }
                FeatureColumn::RelatedObjectType { object_type } => related_objects
                    .get(object_type)
                    .map(|objects| objects.len() as f64)
                    .unwrap_or(0.0),
                FeatureColumn::NumericAttribute { name } => attribute_values
                    .get(name)
                    .and_then(|value| attr_value_to_f64(value))
                    .unwrap_or(0.0),
                FeatureColumn::CategoricalAttribute { name, value } => attribute_values
                    .get(name)
                    .is_some_and(|candidate| self.attr_value_label(candidate) == *value)
                    .then_some(1.0)
                    .unwrap_or(0.0),
            })
            .collect()
    }

    fn latest_attribute_values_at<'a>(
        &'a self,
        object: &'a Object,
        time_ms: i64,
    ) -> BTreeMap<String, &'a AttrValue> {
        let mut latest = BTreeMap::<String, (i64, &'a AttrValue)>::new();
        for attribute in &object.attributes {
            if attribute.time_ms > time_ms {
                continue;
            }
            let name = self.pool.resolve(attribute.name).to_owned();
            if latest
                .get(&name)
                .is_none_or(|(existing_time, _)| attribute.time_ms >= *existing_time)
            {
                latest.insert(name, (attribute.time_ms, &attribute.value));
            }
        }
        latest
            .into_iter()
            .map(|(name, (_time, value))| (name, value))
            .collect()
    }

    fn attr_value_label(&self, value: &AttrValue) -> String {
        match value {
            AttrValue::String(symbol) => self.pool.resolve(*symbol).to_owned(),
            AttrValue::Time(ms) => ms.to_string(),
            AttrValue::Integer(value) => value.to_string(),
            AttrValue::Float(value) => value.to_string(),
            AttrValue::Boolean(value) => value.to_string(),
        }
    }

    fn summarize_som(
        &self,
        windows: &[WindowEncoding],
        points: &[(f64, f64)],
        som: &SomModel,
        color_metric: &ColorMetric,
    ) -> SomSummary {
        let mut cell_counts = vec![0usize; som.width * som.weights.len() / som.width];
        let mut pc_sums = vec![(0.0, 0.0); cell_counts.len()];
        let mut activity_counts = vec![BTreeMap::<String, usize>::new(); cell_counts.len()];
        let mut numeric_color_sums = vec![(0.0, 0usize); cell_counts.len()];
        let mut categorical_color_counts =
            vec![BTreeMap::<String, usize>::new(); cell_counts.len()];
        let mut transitions = BTreeMap::<(usize, usize, usize, usize), usize>::new();

        for ((window, (pc1, pc2)), (cell_x, cell_y)) in windows
            .iter()
            .zip(points.iter())
            .zip(som.assignments.iter())
        {
            let cell_index = cell_y * som.width + cell_x;
            cell_counts[cell_index] += 1;
            pc_sums[cell_index].0 += *pc1;
            pc_sums[cell_index].1 += *pc2;
            if let Some(activity) = self.dominant_window_activity(window) {
                *activity_counts[cell_index].entry(activity).or_default() += 1;
            }
            match color_metric {
                ColorMetric::WindowCount => {}
                ColorMetric::NumericAttribute(name) => {
                    if let Some(value) = self.window_attribute_value(window, name) {
                        if let Some(number) = attr_value_to_f64(value) {
                            numeric_color_sums[cell_index].0 += number;
                            numeric_color_sums[cell_index].1 += 1;
                        }
                    }
                }
                ColorMetric::CategoricalAttribute(name) => {
                    if let Some(value) = self.window_attribute_value(window, name) {
                        *categorical_color_counts[cell_index]
                            .entry(self.attr_value_label(value))
                            .or_default() += 1;
                    }
                }
            }
        }

        for pair in windows.windows(2).zip(som.assignments.windows(2)) {
            let (window_pair, cell_pair) = pair;
            let [left_window, right_window] = window_pair else {
                continue;
            };
            if left_window.object_index != right_window.object_index {
                continue;
            }
            let [(source_x, source_y), (target_x, target_y)] = cell_pair else {
                continue;
            };
            if source_x == target_x && source_y == target_y {
                continue;
            }
            *transitions
                .entry((*source_x, *source_y, *target_x, *target_y))
                .or_default() += 1;
        }

        let max_count = cell_counts.iter().copied().max().unwrap_or(0).max(1);
        let numeric_averages = numeric_color_sums
            .iter()
            .map(|(sum, count)| (*count > 0).then_some(sum / *count as f64))
            .collect::<Vec<_>>();
        let numeric_min = numeric_averages
            .iter()
            .filter_map(|value| *value)
            .fold(f64::INFINITY, f64::min);
        let numeric_max = numeric_averages
            .iter()
            .filter_map(|value| *value)
            .fold(f64::NEG_INFINITY, f64::max);
        let categorical_max = categorical_color_counts
            .iter()
            .filter_map(|counts| counts.values().max().copied())
            .max()
            .unwrap_or(1)
            .max(1);
        let height = som.weights.len() / som.width;
        let mut cells = Vec::with_capacity(som.weights.len());
        for y in 0..height {
            for x in 0..som.width {
                let index = y * som.width + x;
                let count = cell_counts[index];
                let dominant_activity = activity_counts[index]
                    .iter()
                    .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
                    .map(|(activity, _)| activity.clone());
                let (color_value, color_label, color_kind) = match color_metric {
                    ColorMetric::WindowCount => (
                        count as f64 / max_count as f64,
                        format!("{} windows", count),
                        "count".to_owned(),
                    ),
                    ColorMetric::NumericAttribute(name) => {
                        if let Some(average) = numeric_averages[index] {
                            let normalized = if (numeric_max - numeric_min).abs() <= f64::EPSILON {
                                1.0
                            } else {
                                (average - numeric_min) / (numeric_max - numeric_min)
                            };
                            (
                                normalized,
                                format!("avg {name}: {}", format_numeric_feature(average)),
                                "numeric".to_owned(),
                            )
                        } else {
                            (0.0, format!("avg {name}: n/a"), "numeric".to_owned())
                        }
                    }
                    ColorMetric::CategoricalAttribute(name) => {
                        let dominant_category =
                            categorical_color_counts[index]
                                .iter()
                                .max_by(|left, right| {
                                    left.1.cmp(right.1).then_with(|| right.0.cmp(left.0))
                                });
                        if let Some((category, category_count)) = dominant_category {
                            (
                                *category_count as f64 / categorical_max as f64,
                                format!("{name}: {category} ({category_count})"),
                                "categorical".to_owned(),
                            )
                        } else {
                            (0.0, format!("{name}: n/a"), "categorical".to_owned())
                        }
                    }
                };
                cells.push(SomCellSummary {
                    x,
                    y,
                    label: format!("S{}-{}", x + 1, y + 1),
                    count,
                    color_value: round_f64(color_value),
                    color_label,
                    color_kind,
                    avg_pc1: round_f64(if count == 0 {
                        som.weights[index].0
                    } else {
                        pc_sums[index].0 / count as f64
                    }),
                    avg_pc2: round_f64(if count == 0 {
                        som.weights[index].1
                    } else {
                        pc_sums[index].1 / count as f64
                    }),
                    dominant_activity,
                });
            }
        }

        let mut transitions = transitions
            .into_iter()
            .map(|((source_x, source_y, target_x, target_y), count)| {
                let distance = source_x.abs_diff(target_x) + source_y.abs_diff(target_y);
                SomTransitionSummary {
                    source_x,
                    source_y,
                    target_x,
                    target_y,
                    count,
                    distance,
                    nearby: distance <= 1,
                }
            })
            .collect::<Vec<_>>();
        transitions.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.distance.cmp(&right.distance))
                .then_with(|| left.source_y.cmp(&right.source_y))
                .then_with(|| left.source_x.cmp(&right.source_x))
        });

        SomSummary { cells, transitions }
    }

    fn window_attribute_value<'a>(
        &'a self,
        window: &WindowEncoding,
        attribute_name: &str,
    ) -> Option<&'a AttrValue> {
        let object = &self.objects[window.object_index];
        let event_time = window
            .event_indices
            .last()
            .map(|event_index| self.events[*event_index].time_ms)
            .unwrap_or(i64::MAX);
        self.latest_attribute_values_at(object, event_time)
            .remove(attribute_name)
    }

    fn dominant_window_activity(&self, window: &WindowEncoding) -> Option<String> {
        let mut counts = BTreeMap::<String, usize>::new();
        for event_index in &window.event_indices {
            *counts
                .entry(
                    self.pool
                        .resolve(self.events[*event_index].type_name)
                        .to_owned(),
                )
                .or_default() += 1;
        }
        counts
            .into_iter()
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
            .map(|(activity, _)| activity)
    }

    fn directly_follows_graph_json(&self, object_type: &str) -> OcelResult<String> {
        self.object_type_symbol(object_type).ok_or_else(|| {
            OcelError::new(format!("unknown leading object type '{object_type}'"))
        })?;
        let mut graph = GraphAccumulator::new(
            format!("Directly-Follows Graph: {object_type}"),
            format!("Flattened over {object_type} object lifecycles"),
        );
        for object in &self.objects {
            let current_type = self.pool.resolve(object.type_name);
            if current_type == object_type {
                self.accumulate_directly_follows_for_object(&mut graph, object, current_type);
            }
        }
        graph.into_layout()
    }

    fn object_centric_directly_follows_graph_json(&self) -> OcelResult<String> {
        self.object_centric_directly_follows_graph_json_with_filter(&GraphFilterRequest::default())
    }

    fn object_centric_directly_follows_graph_json_with_filter(
        &self,
        request: &GraphFilterRequest,
    ) -> OcelResult<String> {
        let mut graph = GraphAccumulator::new(
            "Object-Centric Directly-Follows Graph".to_owned(),
            "Flattened over selected object types with typed lifecycle edges".to_owned(),
        );
        let selected_object_types = request.object_types.as_ref().map(|object_types| {
            object_types
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        });
        for object in &self.objects {
            let object_type = self.pool.resolve(object.type_name);
            if selected_object_types
                .as_ref()
                .is_some_and(|selected| !selected.contains(object_type))
            {
                continue;
            }
            self.accumulate_directly_follows_for_object(&mut graph, object, object_type);
        }
        graph.into_filtered_layout(request.layout_filter())
    }

    fn state_aware_ocdfg_json(&self) -> OcelResult<String> {
        self.state_aware_ocdfg_json_with_filter(&GraphFilterRequest::default())
    }

    fn state_aware_ocdfg_json_with_filter(
        &self,
        request: &GraphFilterRequest,
    ) -> OcelResult<String> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;
        let mut graph = GraphAccumulator::new(
            "State-Aware Object-Centric Directly-Follows Graph".to_owned(),
            "State-enriched lifecycles collated across object types".to_owned(),
        );
        let selected_object_types = request.object_types.as_ref().map(|object_types| {
            object_types
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        });

        for object in &self.objects {
            let object_type = self.pool.resolve(object.type_name);
            if selected_object_types
                .as_ref()
                .is_some_and(|selected| !selected.contains(object_type))
            {
                continue;
            }
            self.accumulate_state_aware_directly_follows_for_object(
                &mut graph,
                object,
                object_type,
                state_attribute,
            );
        }

        graph.into_filtered_layout(request.layout_filter())
    }

    fn accumulate_directly_follows_for_object(
        &self,
        graph: &mut GraphAccumulator,
        object: &Object,
        object_type: &str,
    ) {
        if object.lifecycle.is_empty() {
            return;
        }

        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            object.lifecycle.len() as f64 + 1.0,
            1,
        );

        for (position, event_index) in object.lifecycle.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            graph.add_node(event_type, "activity", position as f64 + 1.0, 1);
        }

        if let Some(first_index) = object.lifecycle.first() {
            let first = self.pool.resolve(self.events[*first_index].type_name);
            graph.add_edge(&start, first, object_type, 1);
        }

        for pair in object.lifecycle.windows(2) {
            let [source_index, target_index] = pair else {
                continue;
            };
            let source = self.pool.resolve(self.events[*source_index].type_name);
            let target = self.pool.resolve(self.events[*target_index].type_name);
            graph.add_edge(source, target, object_type, 1);
        }

        if let Some(last_index) = object.lifecycle.last() {
            let last = self.pool.resolve(self.events[*last_index].type_name);
            graph.add_edge(last, &end, object_type, 1);
        }
    }

    fn accumulate_state_aware_directly_follows_for_object(
        &self,
        graph: &mut GraphAccumulator,
        object: &Object,
        object_type: &str,
        state_attribute: Symbol,
    ) {
        let stateful_lifecycle = object
            .lifecycle
            .iter()
            .filter_map(|event_index| {
                self.event_state(&self.events[*event_index], state_attribute)
                    .map(|state| (*event_index, state.to_owned()))
            })
            .collect::<Vec<_>>();

        if stateful_lifecycle.is_empty() {
            return;
        }

        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            stateful_lifecycle.len() as f64 * 2.0,
            1,
        );

        for (position, (event_index, state)) in stateful_lifecycle.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            let label = format!("{event_type} [{state}]");
            graph.add_node(&label, "state-activity", position as f64 * 2.0 + 1.0, 1);
        }

        if let Some((first_index, first_state)) = stateful_lifecycle.first() {
            let first_event_type = self.pool.resolve(self.events[*first_index].type_name);
            let first = format!("{first_event_type} [{first_state}]");
            graph.add_edge(&start, &first, object_type, 1);
        }

        for (position, pair) in stateful_lifecycle.windows(2).enumerate() {
            let [(source_index, source_state), (target_index, target_state)] = pair else {
                continue;
            };
            let source_event_type = self.pool.resolve(self.events[*source_index].type_name);
            let target_event_type = self.pool.resolve(self.events[*target_index].type_name);
            let source = format!("{source_event_type} [{source_state}]");
            let target = format!("{target_event_type} [{target_state}]");

            if source_state == target_state {
                graph.add_edge(&source, &target, object_type, 1);
                continue;
            }

            let change = format!("CHANGE {source_state} -> {target_state}");
            graph.add_node(&change, "state-change", position as f64 * 2.0 + 2.0, 1);
            graph.add_edge(&source, &change, object_type, 1);
            graph.add_edge(&change, &target, object_type, 1);
        }

        if let Some((last_index, last_state)) = stateful_lifecycle.last() {
            let last_event_type = self.pool.resolve(self.events[*last_index].type_name);
            let last = format!("{last_event_type} [{last_state}]");
            graph.add_edge(&last, &end, object_type, 1);
        }
    }

    fn detect_state_patterns(&self) -> OcelResult<PatternAnalysis> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;

        if !self
            .events
            .iter()
            .any(|event| self.event_state(event, state_attribute).is_some())
        {
            return Err(OcelError::new(
                "event state attribute is empty; apply a state query first",
            ));
        }

        let mut intra = HashMap::<PatternKey, PatternAccumulator>::new();
        let mut inter = HashMap::<PatternKey, PatternAccumulator>::new();

        for (object_index, object) in self.objects.iter().enumerate() {
            if self
                .state_leading_object_type
                .is_some_and(|leading_type| object.type_name != leading_type)
            {
                continue;
            }

            let state_lifecycle = object
                .lifecycle
                .iter()
                .filter_map(|event_index| {
                    self.event_state(&self.events[*event_index], state_attribute)
                        .map(|state| (*event_index, state.to_owned()))
                })
                .collect::<Vec<_>>();

            if state_lifecycle.is_empty() {
                continue;
            }

            let episodes = state_episodes(&state_lifecycle);
            for episode in &episodes {
                let instance = self.pattern_instance(
                    PatternFamily::Intra,
                    object_index,
                    episode.state.clone(),
                    None,
                    self.intra_sequence(&state_lifecycle, episode),
                    &state_lifecycle[episode.start..=episode.end],
                );
                insert_pattern_instance(&mut intra, instance);
            }

            for episode_pair in episodes.windows(2) {
                let [left, right] = episode_pair else {
                    continue;
                };
                if left.state == right.state {
                    continue;
                }

                let mut segment_events = Vec::with_capacity(right.end - left.start + 1);
                segment_events.extend_from_slice(&state_lifecycle[left.start..=left.end]);
                segment_events.extend_from_slice(&state_lifecycle[right.start..=right.end]);

                let instance = self.pattern_instance(
                    PatternFamily::Inter,
                    object_index,
                    left.state.clone(),
                    Some(right.state.clone()),
                    self.inter_sequence(&state_lifecycle, left, right),
                    &segment_events,
                );
                insert_pattern_instance(&mut inter, instance);
            }
        }

        Ok(PatternAnalysis {
            intra: summarize_patterns(intra.into_values().collect()),
            inter: summarize_patterns(inter.into_values().collect()),
        })
    }

    fn pattern_instance(
        &self,
        family: PatternFamily,
        leading_object_index: usize,
        state: String,
        to_state: Option<String>,
        sequence: Vec<String>,
        segment_events: &[(usize, String)],
    ) -> PatternInstance {
        let leading_object = &self.objects[leading_object_index];
        let leading_type = self.pool.resolve(leading_object.type_name).to_owned();
        let mut object_types = BTreeSet::from([leading_type.clone()]);
        let mut eo_edges = BTreeMap::<(String, String), usize>::new();
        let mut oo_edges = BTreeMap::<(String, String), usize>::new();

        for (event_index, state) in segment_events {
            let event = &self.events[*event_index];
            let event_label = self.state_aware_event_label(*event_index, state);

            for relationship in &event.relationships {
                if relationship.object_id == leading_object.id {
                    continue;
                }

                let Some(related_object_index) =
                    self.object_index.get(&relationship.object_id).copied()
                else {
                    continue;
                };
                let related_type = self
                    .pool
                    .resolve(self.objects[related_object_index].type_name)
                    .to_owned();

                object_types.insert(related_type.clone());
                *eo_edges
                    .entry((event_label.clone(), related_type.clone()))
                    .or_default() += 1;
                let oo_pair = unordered_pair(&leading_type, &related_type);
                *oo_edges.entry(oo_pair).or_default() += 1;
            }
        }

        let mut df_edges = BTreeMap::<(String, String), usize>::new();
        for pair in sequence.windows(2) {
            let [source, target] = pair else {
                continue;
            };
            *df_edges
                .entry((source.clone(), target.clone()))
                .or_default() += 1;
        }

        PatternInstance {
            family,
            leading_object_type: leading_type,
            state,
            to_state,
            sequence,
            object_types,
            df_edges,
            eo_edges,
            oo_edges,
        }
    }

    fn state_aware_event_label(&self, event_index: usize, state: &str) -> String {
        format!(
            "{} [{}]",
            self.pool.resolve(self.events[event_index].type_name),
            state
        )
    }

    fn dominant_object_state(&self, object: &Object, state_attribute: Symbol) -> Option<String> {
        let mut counts = BTreeMap::<String, (usize, usize)>::new();
        for (position, event_index) in object.lifecycle.iter().enumerate() {
            let Some(state) = self.event_state(&self.events[*event_index], state_attribute) else {
                continue;
            };
            let entry = counts.entry(state.to_owned()).or_default();
            entry.0 += 1;
            entry.1 = position;
        }

        counts
            .into_iter()
            .max_by(
                |(left_state, (left_count, left_position)),
                 (right_state, (right_count, right_position))| {
                    left_count
                        .cmp(right_count)
                        .then_with(|| left_position.cmp(right_position))
                        .then_with(|| right_state.cmp(left_state))
                },
            )
            .map(|(state, _)| state)
    }

    fn intra_sequence(
        &self,
        state_lifecycle: &[(usize, String)],
        episode: &StateEpisode,
    ) -> Vec<String> {
        let mut sequence = Vec::with_capacity(episode.end - episode.start + 3);
        sequence.push(format!("START {}", episode.state));
        sequence.extend(
            state_lifecycle[episode.start..=episode.end]
                .iter()
                .map(|(event_index, state)| self.state_aware_event_label(*event_index, state)),
        );
        sequence.push(format!("END {}", episode.state));
        sequence
    }

    fn inter_sequence(
        &self,
        state_lifecycle: &[(usize, String)],
        left: &StateEpisode,
        right: &StateEpisode,
    ) -> Vec<String> {
        let mut sequence = Vec::with_capacity(right.end - left.start + 4);
        sequence.push(format!("START {}", left.state));
        sequence.extend(
            state_lifecycle[left.start..=left.end]
                .iter()
                .map(|(event_index, state)| self.state_aware_event_label(*event_index, state)),
        );
        sequence.push(format!("CHANGE {} -> {}", left.state, right.state));
        sequence.extend(
            state_lifecycle[right.start..=right.end]
                .iter()
                .map(|(event_index, state)| self.state_aware_event_label(*event_index, state)),
        );
        sequence.push(format!("END {}", right.state));
        sequence
    }

    fn event_state<'a>(&'a self, event: &'a Event, state_attribute: Symbol) -> Option<&'a str> {
        event.attributes.iter().find_map(|attribute| {
            if attribute.name == state_attribute {
                match attribute.value {
                    AttrValue::String(symbol) => Some(self.pool.resolve(symbol)),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    fn symbol_for_value(&self, value: &str) -> Option<Symbol> {
        self.pool
            .values
            .iter()
            .position(|candidate| candidate == value)
            .map(|index| Symbol(index as u32))
    }

    fn ensure_event_attribute(&mut self, attribute_symbol: Symbol, attr_type: AttrType) {
        for event_type in &mut self.event_types {
            if !event_type
                .attributes
                .iter()
                .any(|attribute| attribute.name == attribute_symbol)
            {
                event_type.attributes.push(AttributeDef {
                    name: attribute_symbol,
                    attr_type,
                });
            }
        }
    }

    fn evaluate_state_query(
        &self,
        query: &StateQuery,
        eval_index: &StateEvalIndex,
        event_index: usize,
    ) -> Option<String> {
        let event = &self.events[event_index];
        let leading_type_symbol = self.object_type_symbol(&query.leading_object_type)?;
        let related_objects = event
            .relationships
            .iter()
            .filter_map(|relationship| self.object_index.get(&relationship.object_id).copied())
            .filter(|object_index| self.objects[*object_index].type_name == leading_type_symbol)
            .collect::<Vec<_>>();
        if related_objects.is_empty() {
            return None;
        }

        for branch in &query.branches {
            if branch.condition.references_object() {
                for object_index in &related_objects {
                    let context = EvalContext {
                        log: self,
                        eval_index,
                        event_index,
                        object_index: Some(*object_index),
                    };
                    if context.eval_condition(&branch.condition) {
                        return context.eval_state_value(&branch.value);
                    }
                }
            } else {
                let context = EvalContext {
                    log: self,
                    eval_index,
                    event_index,
                    object_index: None,
                };
                if context.eval_condition(&branch.condition) {
                    return context.eval_state_value(&branch.value);
                }
            }
        }

        query.else_value.as_ref().and_then(|value| {
            if value.references_object() {
                related_objects.first().and_then(|object_index| {
                    EvalContext {
                        log: self,
                        eval_index,
                        event_index,
                        object_index: Some(*object_index),
                    }
                    .eval_state_value(value)
                })
            } else {
                EvalContext {
                    log: self,
                    eval_index,
                    event_index,
                    object_index: None,
                }
                .eval_state_value(value)
            }
        })
    }

    fn export_json(&self) -> OcelResult<String> {
        let mut top = Map::new();
        top.insert(
            "eventTypes".to_owned(),
            Value::Array(
                self.event_types
                    .iter()
                    .map(|type_def| self.type_def_to_json(type_def))
                    .collect(),
            ),
        );
        top.insert(
            "objectTypes".to_owned(),
            Value::Array(
                self.object_types
                    .iter()
                    .map(|type_def| self.type_def_to_json(type_def))
                    .collect(),
            ),
        );
        top.insert(
            "events".to_owned(),
            Value::Array(
                self.events
                    .iter()
                    .map(|event| self.event_to_json(event))
                    .collect::<OcelResult<Vec<_>>>()?,
            ),
        );
        top.insert(
            "objects".to_owned(),
            Value::Array(
                self.objects
                    .iter()
                    .map(|object| self.object_to_json(object))
                    .collect::<OcelResult<Vec<_>>>()?,
            ),
        );

        serde_json::to_string_pretty(&Value::Object(top))
            .map_err(|err| OcelError::new(format!("could not export JSON: {err}")))
    }

    fn type_def_to_json(&self, type_def: &TypeDef) -> Value {
        json!({
            "name": self.pool.resolve(type_def.name),
            "attributes": type_def.attributes.iter().map(|attribute| {
                json!({
                    "name": self.pool.resolve(attribute.name),
                    "type": attribute.attr_type.as_str(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    fn event_to_json(&self, event: &Event) -> OcelResult<Value> {
        Ok(json!({
            "id": self.pool.resolve(event.id),
            "type": self.pool.resolve(event.type_name),
            "time": format_timestamp_ms(event.time_ms)?,
            "attributes": event.attributes.iter().map(|attribute| {
                self.attribute_to_json(attribute)
            }).collect::<OcelResult<Vec<_>>>()?,
            "relationships": self.relationships_to_json(&event.relationships),
        }))
    }

    fn object_to_json(&self, object: &Object) -> OcelResult<Value> {
        Ok(json!({
            "id": self.pool.resolve(object.id),
            "type": self.pool.resolve(object.type_name),
            "attributes": object.attributes.iter().map(|attribute| {
                self.timed_attribute_to_json(attribute)
            }).collect::<OcelResult<Vec<_>>>()?,
            "relationships": self.relationships_to_json(&object.relationships),
        }))
    }

    fn attribute_to_json(&self, attribute: &Attribute) -> OcelResult<Value> {
        Ok(json!({
            "name": self.pool.resolve(attribute.name),
            "value": self.attr_value_to_json(&attribute.value)?,
        }))
    }

    fn timed_attribute_to_json(&self, attribute: &TimedAttribute) -> OcelResult<Value> {
        Ok(json!({
            "name": self.pool.resolve(attribute.name),
            "time": format_timestamp_ms(attribute.time_ms)?,
            "value": self.attr_value_to_json(&attribute.value)?,
        }))
    }

    fn relationships_to_json(&self, relationships: &[Relationship]) -> Value {
        Value::Array(
            relationships
                .iter()
                .map(|relationship| {
                    json!({
                        "objectId": self.pool.resolve(relationship.object_id),
                        "qualifier": self.pool.resolve(relationship.qualifier),
                    })
                })
                .collect(),
        )
    }

    fn attr_value_to_json(&self, value: &AttrValue) -> OcelResult<Value> {
        match value {
            AttrValue::String(symbol) => Ok(Value::String(self.pool.resolve(*symbol).to_owned())),
            AttrValue::Time(ms) => Ok(Value::String(format_timestamp_ms(*ms)?)),
            AttrValue::Integer(number) => Ok(Value::Number(Number::from(*number))),
            AttrValue::Float(number) => Number::from_f64(*number)
                .map(Value::Number)
                .ok_or_else(|| OcelError::new("cannot export non-finite float value")),
            AttrValue::Boolean(value) => Ok(Value::Bool(*value)),
        }
    }

    fn export_xml(&self) -> OcelResult<String> {
        let mut output = String::new();
        output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<log>\n");

        output.push_str("  <event-types>\n");
        for event_type in &self.event_types {
            self.write_type_xml(&mut output, "event-type", event_type, 2)?;
        }
        output.push_str("  </event-types>\n");

        output.push_str("  <object-types>\n");
        for object_type in &self.object_types {
            self.write_type_xml(&mut output, "object-type", object_type, 2)?;
        }
        output.push_str("  </object-types>\n");

        output.push_str("  <events>\n");
        for event in &self.events {
            writeln!(
                output,
                "    <event id=\"{}\" type=\"{}\" time=\"{}\">",
                escape_xml_attr(self.pool.resolve(event.id)),
                escape_xml_attr(self.pool.resolve(event.type_name)),
                format_timestamp_ms(event.time_ms)?
            )
            .expect("writing to String cannot fail");
            self.write_attributes_xml(&mut output, &event.attributes, 6)?;
            self.write_relationships_xml(&mut output, &event.relationships, 6);
            output.push_str("    </event>\n");
        }
        output.push_str("  </events>\n");

        output.push_str("  <objects>\n");
        for object in &self.objects {
            writeln!(
                output,
                "    <object id=\"{}\" type=\"{}\">",
                escape_xml_attr(self.pool.resolve(object.id)),
                escape_xml_attr(self.pool.resolve(object.type_name))
            )
            .expect("writing to String cannot fail");
            self.write_timed_attributes_xml(&mut output, &object.attributes, 6)?;
            self.write_relationships_xml(&mut output, &object.relationships, 6);
            output.push_str("    </object>\n");
        }
        output.push_str("  </objects>\n</log>\n");
        Ok(output)
    }

    fn write_type_xml(
        &self,
        output: &mut String,
        tag: &str,
        type_def: &TypeDef,
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        writeln!(
            output,
            "{pad}<{tag} name=\"{}\">",
            escape_xml_attr(self.pool.resolve(type_def.name))
        )
        .expect("writing to String cannot fail");
        if type_def.attributes.is_empty() {
            writeln!(output, "{pad}  <attributes/>").expect("writing to String cannot fail");
        } else {
            writeln!(output, "{pad}  <attributes>").expect("writing to String cannot fail");
            for attribute in &type_def.attributes {
                writeln!(
                    output,
                    "{pad}    <attribute name=\"{}\" type=\"{}\"/>",
                    escape_xml_attr(self.pool.resolve(attribute.name)),
                    attribute.attr_type.as_str()
                )
                .expect("writing to String cannot fail");
            }
            writeln!(output, "{pad}  </attributes>").expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</{tag}>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_attributes_xml(
        &self,
        output: &mut String,
        attributes: &[Attribute],
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        if attributes.is_empty() {
            writeln!(output, "{pad}<attributes/>").expect("writing to String cannot fail");
            return Ok(());
        }

        writeln!(output, "{pad}<attributes>").expect("writing to String cannot fail");
        for attribute in attributes {
            writeln!(
                output,
                "{pad}  <attribute name=\"{}\">{}</attribute>",
                escape_xml_attr(self.pool.resolve(attribute.name)),
                escape_xml_text(&self.attr_value_to_xml_text(&attribute.value)?)
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</attributes>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_timed_attributes_xml(
        &self,
        output: &mut String,
        attributes: &[TimedAttribute],
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        if attributes.is_empty() {
            writeln!(output, "{pad}<attributes/>").expect("writing to String cannot fail");
            return Ok(());
        }

        writeln!(output, "{pad}<attributes>").expect("writing to String cannot fail");
        for attribute in attributes {
            writeln!(
                output,
                "{pad}  <attribute name=\"{}\" time=\"{}\">{}</attribute>",
                escape_xml_attr(self.pool.resolve(attribute.name)),
                format_timestamp_ms(attribute.time_ms)?,
                escape_xml_text(&self.attr_value_to_xml_text(&attribute.value)?)
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</attributes>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_relationships_xml(
        &self,
        output: &mut String,
        relationships: &[Relationship],
        indent: usize,
    ) {
        if relationships.is_empty() {
            return;
        }

        let pad = " ".repeat(indent);
        writeln!(output, "{pad}<objects>").expect("writing to String cannot fail");
        for relationship in relationships {
            writeln!(
                output,
                "{pad}  <relationship object-id=\"{}\" qualifier=\"{}\"/>",
                escape_xml_attr(self.pool.resolve(relationship.object_id)),
                escape_xml_attr(self.pool.resolve(relationship.qualifier))
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</objects>").expect("writing to String cannot fail");
    }

    fn attr_value_to_xml_text(&self, value: &AttrValue) -> OcelResult<String> {
        match value {
            AttrValue::String(symbol) => Ok(self.pool.resolve(*symbol).to_owned()),
            AttrValue::Time(ms) => format_timestamp_ms(*ms),
            AttrValue::Integer(number) => Ok(number.to_string()),
            AttrValue::Float(number) => {
                if !number.is_finite() {
                    return Err(OcelError::new("cannot export non-finite float value"));
                }
                Ok(number.to_string())
            }
            AttrValue::Boolean(value) => Ok(if *value { "1" } else { "0" }.to_owned()),
        }
    }
}
