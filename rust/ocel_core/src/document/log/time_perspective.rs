impl CompactOcelLog {

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
}
