
impl OcelDocumentCore {

    /// Returns object-level feature table metadata and a preview for causal-model editing.
    pub fn causal_feature_table_json(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<CausalFeatureTableRequest>(request_json).map_err(|err| {
                OcelError::new(format!("could not parse causal feature request: {err}"))
            })?;
        self.log.causal_feature_table_json(&request)
    }

    /// Returns the object-level causal feature table as CSV.
    pub fn causal_feature_table_csv(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<CausalFeatureTableRequest>(request_json).map_err(|err| {
                OcelError::new(format!("could not parse causal feature request: {err}"))
            })?;
        self.log.causal_feature_table_csv(&request)
    }

    /// Fits a simple DAG-constrained causal model over the object-level feature table.
    pub fn fit_causal_model_json(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<CausalModelFitRequest>(request_json).map_err(|err| {
                OcelError::new(format!("could not parse causal model request: {err}"))
            })?;
        self.log.fit_causal_model_json(&request)
    }

    /// Computes a flattened directly-follows graph for one object type.
    pub fn directly_follows_graph_json(&self, object_type: &str) -> OcelResult<String> {
        self.log.directly_follows_graph_json(object_type)
    }

    /// Computes an object-centric directly-follows graph collated over all object types.
    pub fn object_centric_directly_follows_graph_json(&self) -> OcelResult<String> {
        self.log.object_centric_directly_follows_graph_json()
    }

    /// Computes an object-centric directly-follows graph for selected object types and frequencies.
    pub fn filtered_object_centric_directly_follows_graph_json(
        &self,
        request_json: &str,
    ) -> OcelResult<String> {
        let request = serde_json::from_str::<GraphFilterRequest>(request_json).map_err(|err| {
            OcelError::new(format!("could not parse graph filter request: {err}"))
        })?;
        self.log
            .object_centric_directly_follows_graph_json_with_filter(&request)
    }

    /// Computes a state-aware object-centric directly-follows graph.
    pub fn state_aware_ocdfg_json(&self) -> OcelResult<String> {
        self.log.state_aware_ocdfg_json()
    }

    /// Computes a state-aware OCDFG for selected object types and frequencies.
    pub fn filtered_state_aware_ocdfg_json(&self, request_json: &str) -> OcelResult<String> {
        let request = serde_json::from_str::<GraphFilterRequest>(request_json).map_err(|err| {
            OcelError::new(format!("could not parse graph filter request: {err}"))
        })?;
        self.log.state_aware_ocdfg_json_with_filter(&request)
    }
}
