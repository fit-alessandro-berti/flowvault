impl CompactOcelLog {

    fn filter(&self, filter: &OcelFilterRequest) -> Self {
        let event_type_selection = if filter.event_types.is_empty() {
            self.event_types
                .iter()
                .map(|type_def| self.pool.resolve(type_def.name))
                .collect::<HashSet<_>>()
        } else {
            filter
                .event_types
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>()
        };
        let object_type_selection = if filter.object_types.is_empty() {
            self.object_types
                .iter()
                .map(|type_def| self.pool.resolve(type_def.name))
                .collect::<HashSet<_>>()
        } else {
            filter
                .object_types
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>()
        };
        let all_event_types_selected = event_type_selection.len() >= self.event_types.len();
        let all_object_types_selected = object_type_selection.len() >= self.object_types.len();

        if all_event_types_selected
            && all_object_types_selected
            && !filter.has_object_predicates()
            && !filter.has_time_predicate()
        {
            return self.clone();
        }

        let selected_object_ids = self
            .objects
            .iter()
            .enumerate()
            .filter(|(object_index, object)| {
                object_type_selection.contains(self.pool.resolve(object.type_name))
                    && self.object_satisfies_filter(*object_index, object, filter)
            })
            .map(|(_, object)| object.id)
            .collect::<HashSet<_>>();

        let mut retained_events = Vec::new();
        let mut retained_object_ids = HashSet::new();
        let require_relationship_match =
            !all_object_types_selected || filter.has_object_predicates();

        for event in &self.events {
            if !event_type_selection.contains(self.pool.resolve(event.type_name)) {
                continue;
            }
            if !filter.accepts_time(event.time_ms) {
                continue;
            }

            let relationships = event
                .relationships
                .iter()
                .filter(|relationship| selected_object_ids.contains(&relationship.object_id))
                .cloned()
                .collect::<Vec<_>>();

            if require_relationship_match && relationships.is_empty() {
                continue;
            }

            retained_object_ids.extend(
                relationships
                    .iter()
                    .map(|relationship| relationship.object_id),
            );
            retained_events.push(Event {
                id: event.id,
                type_name: event.type_name,
                time_ms: event.time_ms,
                attributes: event.attributes.clone(),
                relationships,
            });
        }

        let mut objects = self
            .objects
            .iter()
            .filter(|object| retained_object_ids.contains(&object.id))
            .map(|object| Object {
                id: object.id,
                type_name: object.type_name,
                attributes: object.attributes.clone(),
                relationships: object
                    .relationships
                    .iter()
                    .filter(|relationship| retained_object_ids.contains(&relationship.object_id))
                    .cloned()
                    .collect(),
                lifecycle: Vec::new(),
            })
            .collect::<Vec<_>>();
        let object_index = objects
            .iter()
            .enumerate()
            .map(|(index, object)| (object.id, index))
            .collect::<HashMap<_, _>>();

        for (event_index, event) in retained_events.iter().enumerate() {
            for relationship in &event.relationships {
                if let Some(object_pos) = object_index.get(&relationship.object_id) {
                    objects[*object_pos].lifecycle.push(event_index);
                }
            }
        }

        for object in &mut objects {
            object
                .lifecycle
                .sort_by_key(|event_index| (retained_events[*event_index].time_ms, *event_index));
        }

        let retained_event_types = retained_events
            .iter()
            .map(|event| event.type_name)
            .collect::<HashSet<_>>();
        let retained_object_types = objects
            .iter()
            .map(|object| object.type_name)
            .collect::<HashSet<_>>();

        Self {
            format: self.format,
            pool: self.pool.clone(),
            event_types: self
                .event_types
                .iter()
                .filter(|type_def| retained_event_types.contains(&type_def.name))
                .cloned()
                .collect(),
            object_types: self
                .object_types
                .iter()
                .filter(|type_def| retained_object_types.contains(&type_def.name))
                .cloned()
                .collect(),
            events: retained_events,
            objects,
            object_index,
            state_leading_object_type: self.state_leading_object_type,
        }
    }

    fn filter_options(&self) -> FilterOptions {
        let event_types = self
            .event_types
            .iter()
            .map(|type_def| self.pool.resolve(type_def.name).to_owned())
            .collect();
        let object_types = self
            .object_types
            .iter()
            .map(|type_def| self.pool.resolve(type_def.name).to_owned())
            .collect();

        FilterOptions {
            event_types,
            object_types,
            text_attributes: self.text_attribute_options(),
            time_min_ms: self.events.iter().map(|event| event.time_ms).min(),
            time_max_ms: self.events.iter().map(|event| event.time_ms).max(),
            time_buckets: self.time_filter_buckets(32),
        }
    }
}
