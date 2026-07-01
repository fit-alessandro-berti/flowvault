impl CompactOcelLog {

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
}
