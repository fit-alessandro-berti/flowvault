impl CompactOcelLog {

    fn fit_causal_model(
        &self,
        request: &CausalModelFitRequest,
    ) -> OcelResult<CausalModelFitResult> {
        let table = self.state_feature_table(&request.object_type)?;
        if request.nodes.is_empty() {
            return Err(OcelError::new(
                "causal model must contain at least one node",
            ));
        }
        let node_by_id = validate_causal_nodes(&request.nodes, &table.columns)?;
        validate_causal_edges(&request.edges, &node_by_id)?;
        let order = causal_topological_order(&request.nodes, &request.edges)?;
        let feature_index = table
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| (column.as_str(), index))
            .collect::<HashMap<_, _>>();
        let row_count = table.rows.len();
        let mut vectors = HashMap::<String, Vec<f64>>::new();

        for node_id in order {
            let node = node_by_id
                .get(node_id.as_str())
                .expect("topological order contains validated nodes");
            match causal_role(&node.role)? {
                CausalNodeRole::Observable | CausalNodeRole::Outcome => {
                    let feature = node.feature.as_deref().ok_or_else(|| {
                        OcelError::new(format!("node '{}' must reference a feature", node.label))
                    })?;
                    let column_index = *feature_index.get(feature).ok_or_else(|| {
                        OcelError::new(format!("unknown causal feature '{feature}'"))
                    })?;
                    let values = table
                        .rows
                        .iter()
                        .map(|row| {
                            transform_causal_value(row.values[column_index], &node.operation)
                        })
                        .collect::<Vec<_>>();
                    vectors.insert(node.id.clone(), values);
                }
                CausalNodeRole::Latent => {
                    let parents = request
                        .edges
                        .iter()
                        .filter(|edge| edge.target == node.id)
                        .filter_map(|edge| vectors.get(&edge.source))
                        .map(|values| standardized_vector(values))
                        .collect::<Vec<_>>();
                    vectors.insert(node.id.clone(), average_vectors(&parents, row_count));
                }
            }
        }

        let nodes = request
            .nodes
            .iter()
            .map(|node| {
                let values = vectors.get(&node.id).cloned().unwrap_or_default();
                let (mean, std_dev) = vector_mean_std(&values);
                CausalFitNode {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    role: node.role.clone(),
                    feature: node.feature.clone(),
                    operation: normalized_causal_operation(&node.operation),
                    mean: round_f64(mean),
                    std_dev: round_f64(std_dev),
                }
            })
            .collect::<Vec<_>>();
        let edges = request
            .edges
            .iter()
            .map(|edge| {
                let source_values = vectors
                    .get(&edge.source)
                    .expect("edge source validated before fitting");
                let target_values = vectors
                    .get(&edge.target)
                    .expect("edge target validated before fitting");
                let (correlation, sample_count) = pearson_correlation(source_values, target_values);
                let intensity = correlation.abs();
                CausalFitEdge {
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    correlation: round_f64(correlation),
                    intensity: round_f64(intensity),
                    p_value: round_f64(approximate_correlation_p_value(correlation, sample_count)),
                    sample_count,
                }
            })
            .collect::<Vec<_>>();

        Ok(CausalModelFitResult {
            object_type: request.object_type.clone(),
            sample_count: row_count,
            nodes,
            edges,
        })
    }

    fn state_feature_table(&self, object_type: &str) -> OcelResult<FeatureTableData> {
        let object_type_symbol = self.object_type_symbol(object_type).ok_or_else(|| {
            OcelError::new(format!(
                "unknown object type '{object_type}' for state detection"
            ))
        })?;
        let object_indices = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| (object.type_name == object_type_symbol).then_some(index))
            .collect::<Vec<_>>();
        let encoder = self.build_feature_encoder(&object_indices);
        let columns = encoder.columns.iter().map(FeatureColumn::label).collect();
        let rows = object_indices
            .iter()
            .map(|object_index| {
                let object = &self.objects[*object_index];
                FeatureRow {
                    object_id: self.pool.resolve(object.id).to_owned(),
                    values: self.encode_feature_vector(
                        *object_index,
                        &object.lifecycle,
                        i64::MAX,
                        &encoder,
                    ),
                }
            })
            .collect();

        Ok(FeatureTableData { columns, rows })
    }
}
