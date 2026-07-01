impl CompactOcelLog {

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
}
