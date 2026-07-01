impl CompactOcelLog {

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
}
