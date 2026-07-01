impl CompactOcelLog {

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
}
