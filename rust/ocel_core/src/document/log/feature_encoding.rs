impl CompactOcelLog {

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
}
