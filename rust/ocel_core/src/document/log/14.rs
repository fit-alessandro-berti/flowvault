impl CompactOcelLog {

    fn resolve_color_metric(
        &self,
        requested: Option<&str>,
        options: &[StateDetectionColorOption],
    ) -> ColorMetric {
        let Some(requested) = requested else {
            return ColorMetric::WindowCount;
        };
        if requested == "__window_count" {
            return ColorMetric::WindowCount;
        }
        let Some(option) = options.iter().find(|option| option.id == requested) else {
            return ColorMetric::WindowCount;
        };
        let attribute_name = option
            .id
            .strip_prefix("attribute::")
            .unwrap_or(&option.label)
            .to_owned();
        match option.kind {
            "numeric" => ColorMetric::NumericAttribute(attribute_name),
            "categorical" => ColorMetric::CategoricalAttribute(attribute_name),
            _ => ColorMetric::WindowCount,
        }
    }

    fn projected_windows(
        &self,
        run: &StateDetectionRun,
        limit: usize,
    ) -> Vec<StateWindowProjection> {
        run.windows
            .iter()
            .zip(run.pca.points.iter())
            .zip(run.som.assignments.iter())
            .take(limit)
            .map(|((window, (pc1, pc2)), (cell_x, cell_y))| {
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
                    pc1: round_f64(*pc1),
                    pc2: round_f64(*pc2),
                    cell_x: *cell_x,
                    cell_y: *cell_y,
                }
            })
            .collect()
    }

    fn state_detection_cell_detail(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> OcelResult<StateDetectionCellDetail> {
        let height = run.som.weights.len() / run.som.width;
        if cell_x >= run.som.width || cell_y >= height {
            return Err(OcelError::new(format!(
                "SOM cell {},{} is outside the {}x{} grid",
                cell_x, cell_y, run.som.width, height
            )));
        }

        let som_summary =
            self.summarize_som(&run.windows, &run.pca.points, &run.som, &run.color_metric);
        let cell = som_summary
            .cells
            .into_iter()
            .find(|cell| cell.x == cell_x && cell.y == cell_y)
            .expect("validated SOM cell must exist");
        let dfg = self.state_detection_cell_dfg(run, cell_x, cell_y);
        let (entering_windows, exiting_windows, entering_indices, exiting_indices) =
            self.state_detection_boundary_windows(run, cell_x, cell_y);
        let entering_dfg = self.state_detection_windows_dfg(
            run,
            &entering_indices,
            format!("Entering Windows: {}", cell.label),
            "Directly-follows graph over windows entering the selected SOM cell".to_owned(),
        );
        let exiting_dfg = self.state_detection_windows_dfg(
            run,
            &exiting_indices,
            format!("Exiting Windows: {}", cell.label),
            "Directly-follows graph over windows exiting the selected SOM cell".to_owned(),
        );

        Ok(StateDetectionCellDetail {
            cell,
            dfg,
            entering_dfg,
            exiting_dfg,
            entering_window_count: entering_indices.len(),
            exiting_window_count: exiting_indices.len(),
            entering_windows,
            exiting_windows,
        })
    }

    fn state_detection_cell_dfg(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> LayoutGraph {
        let mut graph = GraphAccumulator::new(
            format!("State Detection Cell S{}-{}", cell_x + 1, cell_y + 1),
            "Directly-follows graph over windows assigned to the selected SOM cell".to_owned(),
        );
        let object_type = self
            .pool
            .resolve(self.objects[run.windows[0].object_index].type_name);

        for (window, (assigned_x, assigned_y)) in run.windows.iter().zip(run.som.assignments.iter())
        {
            if *assigned_x != cell_x || *assigned_y != cell_y || window.event_indices.is_empty() {
                continue;
            }
            self.accumulate_window_directly_follows(&mut graph, window, object_type);
        }

        layout_accumulated_graph(graph)
    }

    fn state_detection_windows_dfg(
        &self,
        run: &StateDetectionRun,
        window_indices: &[usize],
        title: String,
        subtitle: String,
    ) -> LayoutGraph {
        let mut graph = GraphAccumulator::new(title, subtitle);
        let object_type = self
            .pool
            .resolve(self.objects[run.windows[0].object_index].type_name);

        for window_index in window_indices {
            if let Some(window) = run.windows.get(*window_index) {
                self.accumulate_window_directly_follows(&mut graph, window, object_type);
            }
        }

        layout_accumulated_graph(graph)
    }
}
