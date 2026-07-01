impl CompactOcelLog {

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
}
