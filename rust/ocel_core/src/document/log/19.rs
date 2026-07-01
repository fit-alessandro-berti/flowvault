impl CompactOcelLog {

    fn window_attribute_value<'a>(
        &'a self,
        window: &WindowEncoding,
        attribute_name: &str,
    ) -> Option<&'a AttrValue> {
        let object = &self.objects[window.object_index];
        let event_time = window
            .event_indices
            .last()
            .map(|event_index| self.events[*event_index].time_ms)
            .unwrap_or(i64::MAX);
        self.latest_attribute_values_at(object, event_time)
            .remove(attribute_name)
    }

    fn dominant_window_activity(&self, window: &WindowEncoding) -> Option<String> {
        let mut counts = BTreeMap::<String, usize>::new();
        for event_index in &window.event_indices {
            *counts
                .entry(
                    self.pool
                        .resolve(self.events[*event_index].type_name)
                        .to_owned(),
                )
                .or_default() += 1;
        }
        counts
            .into_iter()
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
            .map(|(activity, _)| activity)
    }

    fn directly_follows_graph_json(&self, object_type: &str) -> OcelResult<String> {
        self.object_type_symbol(object_type).ok_or_else(|| {
            OcelError::new(format!("unknown leading object type '{object_type}'"))
        })?;
        let mut graph = GraphAccumulator::new(
            format!("Directly-Follows Graph: {object_type}"),
            format!("Flattened over {object_type} object lifecycles"),
        );
        for object in &self.objects {
            let current_type = self.pool.resolve(object.type_name);
            if current_type == object_type {
                self.accumulate_directly_follows_for_object(&mut graph, object, current_type);
            }
        }
        graph.into_layout()
    }

    fn object_centric_directly_follows_graph_json(&self) -> OcelResult<String> {
        self.object_centric_directly_follows_graph_json_with_filter(&GraphFilterRequest::default())
    }

    fn object_centric_directly_follows_graph_json_with_filter(
        &self,
        request: &GraphFilterRequest,
    ) -> OcelResult<String> {
        let mut graph = GraphAccumulator::new(
            "Object-Centric Directly-Follows Graph".to_owned(),
            "Flattened over selected object types with typed lifecycle edges".to_owned(),
        );
        let selected_object_types = request.object_types.as_ref().map(|object_types| {
            object_types
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        });
        for object in &self.objects {
            let object_type = self.pool.resolve(object.type_name);
            if selected_object_types
                .as_ref()
                .is_some_and(|selected| !selected.contains(object_type))
            {
                continue;
            }
            self.accumulate_directly_follows_for_object(&mut graph, object, object_type);
        }
        graph.into_filtered_layout(request.layout_filter())
    }

    fn state_aware_ocdfg_json(&self) -> OcelResult<String> {
        self.state_aware_ocdfg_json_with_filter(&GraphFilterRequest::default())
    }

    fn state_aware_ocdfg_json_with_filter(
        &self,
        request: &GraphFilterRequest,
    ) -> OcelResult<String> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;
        let mut graph = GraphAccumulator::new(
            "State-Aware Object-Centric Directly-Follows Graph".to_owned(),
            "State-enriched lifecycles collated across object types".to_owned(),
        );
        let selected_object_types = request.object_types.as_ref().map(|object_types| {
            object_types
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        });

        for object in &self.objects {
            let object_type = self.pool.resolve(object.type_name);
            if selected_object_types
                .as_ref()
                .is_some_and(|selected| !selected.contains(object_type))
            {
                continue;
            }
            self.accumulate_state_aware_directly_follows_for_object(
                &mut graph,
                object,
                object_type,
                state_attribute,
            );
        }

        graph.into_filtered_layout(request.layout_filter())
    }

    fn accumulate_directly_follows_for_object(
        &self,
        graph: &mut GraphAccumulator,
        object: &Object,
        object_type: &str,
    ) {
        if object.lifecycle.is_empty() {
            return;
        }

        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            object.lifecycle.len() as f64 + 1.0,
            1,
        );

        for (position, event_index) in object.lifecycle.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            graph.add_node(event_type, "activity", position as f64 + 1.0, 1);
        }

        if let Some(first_index) = object.lifecycle.first() {
            let first = self.pool.resolve(self.events[*first_index].type_name);
            graph.add_edge(&start, first, object_type, 1);
        }

        for pair in object.lifecycle.windows(2) {
            let [source_index, target_index] = pair else {
                continue;
            };
            let source = self.pool.resolve(self.events[*source_index].type_name);
            let target = self.pool.resolve(self.events[*target_index].type_name);
            graph.add_edge(source, target, object_type, 1);
        }

        if let Some(last_index) = object.lifecycle.last() {
            let last = self.pool.resolve(self.events[*last_index].type_name);
            graph.add_edge(last, &end, object_type, 1);
        }
    }
}
