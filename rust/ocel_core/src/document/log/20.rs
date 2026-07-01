impl CompactOcelLog {

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
}
