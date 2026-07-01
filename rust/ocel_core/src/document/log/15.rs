impl CompactOcelLog {

    fn accumulate_window_directly_follows(
        &self,
        graph: &mut GraphAccumulator,
        window: &WindowEncoding,
        object_type: &str,
    ) {
        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            window.event_indices.len() as f64 + 1.0,
            1,
        );

        for (position, event_index) in window.event_indices.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            graph.add_node(event_type, "activity", position as f64 + 1.0, 1);
        }

        if let Some(first_index) = window.event_indices.first() {
            let first = self.pool.resolve(self.events[*first_index].type_name);
            graph.add_edge(&start, first, object_type, 1);
        }
        for pair in window.event_indices.windows(2) {
            let [source_index, target_index] = pair else {
                continue;
            };
            let source = self.pool.resolve(self.events[*source_index].type_name);
            let target = self.pool.resolve(self.events[*target_index].type_name);
            graph.add_edge(source, target, object_type, 1);
        }
        if let Some(last_index) = window.event_indices.last() {
            let last = self.pool.resolve(self.events[*last_index].type_name);
            graph.add_edge(last, &end, object_type, 1);
        }
    }

    fn state_detection_boundary_windows(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> (
        Vec<StateDetectionBoundaryWindow>,
        Vec<StateDetectionBoundaryWindow>,
        Vec<usize>,
        Vec<usize>,
    ) {
        let mut entering = Vec::new();
        let mut exiting = Vec::new();
        let mut entering_indices = Vec::new();
        let mut exiting_indices = Vec::new();

        for index in 1..run.windows.len() {
            let previous = &run.windows[index - 1];
            let current = &run.windows[index];
            if previous.object_index != current.object_index {
                continue;
            }
            let source = run.som.assignments[index - 1];
            let target = run.som.assignments[index];
            if source == target {
                continue;
            }
            if target == (cell_x, cell_y) {
                if entering.len() < 100 {
                    entering.push(self.boundary_window_summary(run, index, source, target));
                }
                entering_indices.push(index);
            }
            if source == (cell_x, cell_y) {
                if exiting.len() < 100 {
                    exiting.push(self.boundary_window_summary(run, index - 1, source, target));
                }
                exiting_indices.push(index - 1);
            }
        }

        (entering, exiting, entering_indices, exiting_indices)
    }

    fn boundary_window_summary(
        &self,
        run: &StateDetectionRun,
        window_index: usize,
        source: (usize, usize),
        target: (usize, usize),
    ) -> StateDetectionBoundaryWindow {
        let window = &run.windows[window_index];
        let projection = self.projected_window(window, run.pca.points[window_index], target);
        StateDetectionBoundaryWindow {
            object_id: projection.object_id,
            start_event: projection.start_event,
            end_event: projection.end_event,
            source_cell: cell_label(source.0, source.1),
            target_cell: cell_label(target.0, target.1),
            pc1: projection.pc1,
            pc2: projection.pc2,
            activities: self.window_activity_sequence(window),
        }
    }

    fn projected_window(
        &self,
        window: &WindowEncoding,
        point: (f64, f64),
        cell: (usize, usize),
    ) -> StateWindowProjection {
        let object = &self.objects[window.object_index];
        let first_event = window
            .event_indices
            .first()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .unwrap_or("");
        let last_event = window
            .event_indices
            .last()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .unwrap_or("");
        StateWindowProjection {
            object_id: self.pool.resolve(object.id).to_owned(),
            start_event: first_event.to_owned(),
            end_event: last_event.to_owned(),
            pc1: round_f64(point.0),
            pc2: round_f64(point.1),
            cell_x: cell.0,
            cell_y: cell.1,
        }
    }

    fn window_activity_sequence(&self, window: &WindowEncoding) -> Vec<String> {
        window
            .event_indices
            .iter()
            .map(|event_index| {
                self.pool
                    .resolve(self.events[*event_index].type_name)
                    .to_owned()
            })
            .collect()
    }
}
