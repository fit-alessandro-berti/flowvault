impl CompactOcelLog {

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
}
