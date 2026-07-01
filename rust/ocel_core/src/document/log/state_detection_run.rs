impl CompactOcelLog {

    fn detect_execution_states(
        &self,
        request: &StateDetectionRequest,
    ) -> OcelResult<StateDetectionResult> {
        let run = self.compute_state_detection_run(request)?;
        let window_size = request.window_size.unwrap_or(4).clamp(1, 30);
        let som_width = run.som.width;
        let som_height = run.som.weights.len() / som_width;
        let som_summary =
            self.summarize_som(&run.windows, &run.pca.points, &run.som, &run.color_metric);
        let feature_columns = run
            .encoder
            .columns
            .iter()
            .map(FeatureColumn::label)
            .collect::<Vec<_>>();
        let table_preview = run
            .feature_table
            .rows
            .iter()
            .take(15)
            .map(|row| FeaturePreviewRow {
                object_id: row.object_id.clone(),
                values: row.values.clone(),
            })
            .collect();
        let projected_windows = self.projected_windows(&run, 500);

        Ok(StateDetectionResult {
            object_type: request.object_type.clone(),
            window_size,
            som_width,
            som_height,
            color_attribute: run.color_metric.id(),
            color_attributes: run.color_options,
            object_count: run.object_indices.len(),
            feature_count: feature_columns.len(),
            window_count: run.windows.len(),
            feature_columns,
            table_preview,
            pca: PcaSummary {
                pc1_variance: round_f64(run.pca.pc1_variance),
                pc2_variance: round_f64(run.pca.pc2_variance),
                pc1_explained_ratio: round_f64(run.pca.pc1_explained_ratio),
                pc2_explained_ratio: round_f64(run.pca.pc2_explained_ratio),
            },
            som: som_summary,
            windows: projected_windows,
        })
    }

    fn compute_state_detection_run(
        &self,
        request: &StateDetectionRequest,
    ) -> OcelResult<StateDetectionRun> {
        let object_type_symbol =
            self.object_type_symbol(&request.object_type)
                .ok_or_else(|| {
                    OcelError::new(format!(
                        "unknown object type '{}' for state detection",
                        request.object_type
                    ))
                })?;
        let object_indices = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| (object.type_name == object_type_symbol).then_some(index))
            .collect::<Vec<_>>();
        if object_indices.is_empty() {
            return Err(OcelError::new(format!(
                "no objects of type '{}' are available in the active log",
                request.object_type
            )));
        }

        let encoder = self.build_feature_encoder(&object_indices);
        if encoder.columns.is_empty() {
            return Err(OcelError::new(format!(
                "no numerical features could be extracted for '{}'",
                request.object_type
            )));
        }

        let window_size = request.window_size.unwrap_or(4).clamp(1, 30);
        let windows = self.encode_lifecycle_windows(&object_indices, window_size, &encoder);
        if windows.is_empty() {
            return Err(OcelError::new(format!(
                "no lifecycle windows could be extracted for '{}'",
                request.object_type
            )));
        }

        let values = windows
            .iter()
            .map(|window| window.values.clone())
            .collect::<Vec<_>>();
        let pca = pca_project(&values);
        let (som_width, som_height) =
            default_som_dimensions(pca.points.len(), request.som_width, request.som_height);
        let som = train_som(
            &pca.points,
            som_width,
            som_height,
            request.epochs.unwrap_or(120).clamp(10, 500),
        );
        let feature_table = self.state_feature_table(&request.object_type)?;
        let color_options = self.state_detection_color_options(&object_indices);
        let color_metric =
            self.resolve_color_metric(request.color_attribute.as_deref(), &color_options);

        Ok(StateDetectionRun {
            object_indices,
            encoder,
            feature_table,
            windows,
            pca,
            som,
            color_metric,
            color_options,
        })
    }

    fn state_detection_color_options(
        &self,
        object_indices: &[usize],
    ) -> Vec<StateDetectionColorOption> {
        let mut attributes = BTreeMap::<String, AttributeFeatureCollector>::new();
        for object_index in object_indices {
            let object = &self.objects[*object_index];
            for attribute in &object.attributes {
                let entry = attributes
                    .entry(self.pool.resolve(attribute.name).to_owned())
                    .or_default();
                if attr_value_to_f64(&attribute.value).is_some() {
                    entry.has_numeric = true;
                } else {
                    entry
                        .categories
                        .insert(self.attr_value_label(&attribute.value));
                }
            }
        }

        let mut options = vec![StateDetectionColorOption {
            id: "__window_count".to_owned(),
            label: "Assigned windows".to_owned(),
            kind: "count",
        }];
        for (name, collector) in attributes {
            if collector.has_numeric && collector.categories.is_empty() {
                options.push(StateDetectionColorOption {
                    id: format!("attribute::{name}"),
                    label: name,
                    kind: "numeric",
                });
            } else if !collector.categories.is_empty() && collector.categories.len() < 50 {
                options.push(StateDetectionColorOption {
                    id: format!("attribute::{name}"),
                    label: name,
                    kind: "categorical",
                });
            }
        }
        options
    }
}
