impl CompactOcelLog {

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
}
