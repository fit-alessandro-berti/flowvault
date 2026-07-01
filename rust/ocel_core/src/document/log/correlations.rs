impl CompactOcelLog {

    fn state_correlations(&self) -> OcelResult<StateCorrelationResult> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;
        let leading_type_symbol = self.state_leading_object_type.ok_or_else(|| {
            OcelError::new(
                "state leading object type is unknown; apply a state query or state detection first",
            )
        })?;
        let object_type = self.pool.resolve(leading_type_symbol).to_owned();
        let feature_table = self.state_feature_table(&object_type)?;
        let state_by_object = self
            .objects
            .iter()
            .filter(|object| object.type_name == leading_type_symbol)
            .filter_map(|object| {
                self.dominant_object_state(object, state_attribute)
                    .map(|state| (self.pool.resolve(object.id).to_owned(), state))
            })
            .collect::<HashMap<_, _>>();

        if state_by_object.is_empty() {
            return Err(OcelError::new(format!(
                "no active '{object_type}' objects have stateful events"
            )));
        }

        let mut state_distribution = BTreeMap::<String, usize>::new();
        let row_states = feature_table
            .rows
            .iter()
            .map(|row| {
                let state = state_by_object.get(&row.object_id).cloned();
                if let Some(state) = &state {
                    *state_distribution.entry(state.clone()).or_default() += 1;
                }
                state
            })
            .collect::<Vec<_>>();

        let stateful_object_count = row_states.iter().filter(|state| state.is_some()).count();
        if stateful_object_count == 0 {
            return Err(OcelError::new(format!(
                "no active '{object_type}' feature rows can be matched to stateful objects"
            )));
        }

        let mut rows = Vec::with_capacity(feature_table.columns.len());
        for (column_index, feature) in feature_table.columns.iter().enumerate() {
            rows.push(state_feature_correlation_row(
                feature,
                column_index,
                &feature_table.rows,
                &row_states,
            ));
        }
        rows.sort_by(|left, right| {
            right
                .strength
                .partial_cmp(&left.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.feature.cmp(&right.feature))
        });

        Ok(StateCorrelationResult {
            object_type,
            object_count: feature_table.rows.len(),
            stateful_object_count,
            state_count: state_distribution.len(),
            feature_count: feature_table.columns.len(),
            state_distribution: state_distribution
                .into_iter()
                .map(|(state, count)| StateCorrelationStateCount { state, count })
                .collect(),
            rows,
        })
    }

    fn fit_causal_model_json(&self, request: &CausalModelFitRequest) -> OcelResult<String> {
        let fit = self.fit_causal_model(request)?;
        serde_json::to_string(&fit)
            .map_err(|err| OcelError::new(format!("could not serialize causal model fit: {err}")))
    }
}
