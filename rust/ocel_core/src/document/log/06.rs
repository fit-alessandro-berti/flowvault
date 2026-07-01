impl CompactOcelLog {

    fn apply_state_labels_by_event_id(
        &mut self,
        leading_object_type: &str,
        states: &[StateDetectionEventState],
    ) -> OcelResult<usize> {
        let leading_type_symbol =
            self.object_type_symbol(leading_object_type)
                .ok_or_else(|| {
                    OcelError::new(format!(
                        "unknown leading object type '{leading_object_type}'"
                    ))
                })?;
        let attribute_symbol = self.pool.intern("state");
        self.ensure_event_attribute(attribute_symbol, AttrType::String);
        self.state_leading_object_type = Some(leading_type_symbol);

        for event in &mut self.events {
            event
                .attributes
                .retain(|attribute| attribute.name != attribute_symbol);
        }

        let event_index_by_id = self
            .events
            .iter()
            .enumerate()
            .map(|(event_index, event)| (event.id, event_index))
            .collect::<HashMap<_, _>>();

        let mut assigned = 0usize;
        for assignment in states {
            let Some(event_index) = event_index_by_id.get(&assignment.event_id) else {
                continue;
            };
            let state_symbol = self.pool.intern(&assignment.state);
            self.events[*event_index].attributes.push(Attribute {
                name: attribute_symbol,
                value: AttrValue::String(state_symbol),
            });
            assigned += 1;
        }

        Ok(assigned)
    }

    fn state_patterns_json(&self) -> OcelResult<String> {
        let analysis = self.detect_state_patterns()?;
        serde_json::to_string(&analysis).map_err(|err| {
            OcelError::new(format!("could not serialize state pattern analysis: {err}"))
        })
    }

    fn state_detection_json(&self, request: &StateDetectionRequest) -> OcelResult<String> {
        let analysis = self.detect_execution_states(request)?;
        serde_json::to_string(&analysis)
            .map_err(|err| OcelError::new(format!("could not serialize state detection: {err}")))
    }

    fn state_detection_cell_json(&self, request: &StateDetectionCellRequest) -> OcelResult<String> {
        let state_request = StateDetectionRequest {
            object_type: request.object_type.clone(),
            window_size: request.window_size,
            som_width: request.som_width,
            som_height: request.som_height,
            epochs: request.epochs,
            color_attribute: request.color_attribute.clone(),
        };
        let run = self.compute_state_detection_run(&state_request)?;
        let detail = self.state_detection_cell_detail(&run, request.cell_x, request.cell_y)?;
        serde_json::to_string(&detail).map_err(|err| {
            OcelError::new(format!("could not serialize state detection cell: {err}"))
        })
    }

    fn state_feature_table_csv(&self, request: &StateDetectionRequest) -> OcelResult<String> {
        let table = self.state_feature_table(&request.object_type)?;
        Ok(feature_table_to_csv(&table))
    }

    fn state_correlations_json(&self) -> OcelResult<String> {
        let analysis = self.state_correlations()?;
        serde_json::to_string(&analysis)
            .map_err(|err| OcelError::new(format!("could not serialize state correlations: {err}")))
    }

    fn time_perspective_json(&self, request: &TimePerspectiveRequest) -> OcelResult<String> {
        let analysis = self.time_perspective(request)?;
        serde_json::to_string(&analysis)
            .map_err(|err| OcelError::new(format!("could not serialize time perspective: {err}")))
    }

    fn state_transition_kpis_json(
        &self,
        request: &StateTransitionKpiRequest,
    ) -> OcelResult<String> {
        let analysis = self.state_transition_kpis(request)?;
        serde_json::to_string(&analysis).map_err(|err| {
            OcelError::new(format!("could not serialize state transition KPIs: {err}"))
        })
    }

    fn object_search_json(&self, request: &ObjectSearchRequest) -> OcelResult<String> {
        let result = self.object_search(request);
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize object search: {err}")))
    }

    fn object_lifecycle_detail_json(&self, object_id: &str) -> OcelResult<String> {
        let detail = self.object_lifecycle_detail(object_id)?;
        serde_json::to_string(&detail)
            .map_err(|err| OcelError::new(format!("could not serialize object lifecycle: {err}")))
    }

    fn causal_feature_table_json(&self, request: &CausalFeatureTableRequest) -> OcelResult<String> {
        let table = self.state_feature_table(&request.object_type)?;
        let result = CausalFeatureTableResult {
            object_type: request.object_type.clone(),
            object_count: table.rows.len(),
            feature_count: table.columns.len(),
            feature_columns: table.columns.clone(),
            table_preview: table
                .rows
                .iter()
                .take(15)
                .map(|row| FeaturePreviewRow {
                    object_id: row.object_id.clone(),
                    values: row.values.clone(),
                })
                .collect(),
        };
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize causal features: {err}")))
    }

    fn causal_feature_table_csv(&self, request: &CausalFeatureTableRequest) -> OcelResult<String> {
        let table = self.state_feature_table(&request.object_type)?;
        Ok(feature_table_to_csv(&table))
    }
}
