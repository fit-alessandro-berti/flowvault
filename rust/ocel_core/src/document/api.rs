/// Parsed OCEL document with a stable Rust API.
///
/// Constructing this type imports and validates the OCEL text once. Subsequent
/// summary/export calls reuse the compact in-memory representation.
pub struct OcelDocumentCore {
    original_log: CompactOcelLog,
    log: CompactOcelLog,
    current_filter: OcelFilterRequest,
}

impl OcelDocumentCore {
    /// Imports an OCEL 2.0 JSON or XML string.
    ///
    /// `format_hint` may be `"json"`, `"xml"`, a filename, or `None`.
    /// When omitted, the parser detects JSON/XML from the first non-whitespace
    /// character.
    pub fn new(input: &str, format_hint: Option<&str>) -> OcelResult<Self> {
        let log = CompactOcelLog::from_input(input, format_hint)?;
        let current_filter = OcelFilterRequest::all_for(&log);
        Ok(Self {
            original_log: log.clone(),
            log,
            current_filter,
        })
    }

    /// Imports an OCEL 2.0 JSON/XML file from raw bytes.
    ///
    /// The byte input may be plain UTF-8 JSON/XML or a gzip-compressed `.gz`
    /// file containing UTF-8 JSON/XML.
    pub fn from_bytes(input: &[u8], format_hint: Option<&str>) -> OcelResult<Self> {
        let log = CompactOcelLog::from_bytes(input, format_hint)?;
        let current_filter = OcelFilterRequest::all_for(&log);
        Ok(Self {
            original_log: log.clone(),
            log,
            current_filter,
        })
    }

    /// Returns summary counts as a JSON string.
    pub fn summary_json(&self) -> String {
        self.log.summary_json()
    }

    /// Returns summary counts for the unfiltered imported document as a JSON string.
    pub fn original_summary_json(&self) -> String {
        self.original_log.summary_json()
    }

    /// Returns available event and object types for filtering as a JSON string.
    pub fn filter_options_json(&self) -> String {
        serde_json::to_string(&self.original_log.filter_options())
            .expect("filter option serialization cannot fail")
    }

    /// Rebuilds the active log from the original, possibly state-enriched, log.
    pub fn apply_filter(&mut self, filter_json: &str) -> OcelResult<String> {
        let filter = serde_json::from_str::<OcelFilterRequest>(filter_json)
            .map_err(|err| OcelError::new(format!("could not parse filter request: {err}")))?;
        self.current_filter = filter;
        self.log = self.original_log.filter(&self.current_filter);
        Ok(self.log.summary_json())
    }

    /// Exports the document as OCEL 2.0 JSON.
    pub fn export_json(&self) -> OcelResult<String> {
        self.log.export_json()
    }

    /// Exports the document as OCEL 2.0 XML.
    pub fn export_xml(&self) -> OcelResult<String> {
        self.log.export_xml()
    }

    /// Returns the ordered event IDs related to an object ID as a JSON array.
    pub fn object_lifecycle_json(&self, object_id: &str) -> OcelResult<String> {
        self.log.lifecycle_json(object_id)
    }

    /// Applies a SQL-like CASE query and writes a string state attribute to events.
    pub fn apply_state_query(&mut self, query: &str) -> OcelResult<String> {
        let parsed_query = StateQuery::parse(query)?;
        let attribute = parsed_query.attribute_name;
        let leading_object_type = parsed_query.leading_object_type;
        self.original_log.apply_state_query(query)?;
        self.log = self.original_log.filter(&self.current_filter);
        let assigned_events = self.log.count_events_with_attribute(&attribute);
        let total_events = self.log.events.len();

        let result = StateQueryResult {
            attribute,
            leading_object_type,
            assigned_events,
            total_events,
        };
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize state result: {err}")))
    }

    /// Applies the current State Detection SOM labels as the string `state` event attribute.
    pub fn apply_state_detection(&mut self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<StateDetectionRequest>(request_json).map_err(|err| {
                OcelError::new(format!("could not parse state detection request: {err}"))
            })?;
        let assignments = self.log.state_detection_state_assignments(&request)?;
        self.original_log.apply_state_labels_by_event_id(
            &assignments.leading_object_type,
            &assignments.states,
        )?;
        self.log = self.original_log.filter(&self.current_filter);
        let assigned_events = self.log.count_events_with_attribute("state");
        let total_events = self.log.events.len();

        let result = StateQueryResult {
            attribute: "state".to_owned(),
            leading_object_type: assignments.leading_object_type,
            assigned_events,
            total_events,
        };
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize state result: {err}")))
    }

    /// Detects ranked intra-state and inter-state behavioral patterns.
    pub fn state_patterns_json(&self) -> OcelResult<String> {
        self.log.state_patterns_json()
    }

    /// Extracts object-level features and detects execution-state cells with PCA and SOM.
    pub fn state_detection_json(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<StateDetectionRequest>(request_json).map_err(|err| {
                OcelError::new(format!("could not parse state detection request: {err}"))
            })?;
        self.log.state_detection_json(&request)
    }

    /// Returns details for one SOM cell, including a DFG and entering/exiting windows.
    pub fn state_detection_cell_json(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<StateDetectionCellRequest>(request_json).map_err(|err| {
                OcelError::new(format!(
                    "could not parse state detection cell request: {err}"
                ))
            })?;
        self.log.state_detection_cell_json(&request)
    }

    /// Returns the object-level numerical feature table as CSV.
    pub fn state_feature_table_csv(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<StateDetectionRequest>(request_json).map_err(|err| {
                OcelError::new(format!("could not parse state detection request: {err}"))
            })?;
        self.log.state_feature_table_csv(&request)
    }

    /// Correlates object-level features with the currently applied event state.
    pub fn state_correlations_json(&self) -> OcelResult<String> {
        self.log.state_correlations_json()
    }

    /// Returns smoothed time-perspective inputs for state frequencies and transition durations.
    pub fn time_perspective_json(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<TimePerspectiveRequest>(request_json).map_err(|err| {
                OcelError::new(format!("could not parse time perspective request: {err}"))
            })?;
        self.log.time_perspective_json(&request)
    }

    /// Returns transition matrix inputs, dwell times, recovery rows, and stuck-state rankings.
    pub fn state_transition_kpis_json(&self, request_json: &str) -> OcelResult<String> {
        let request =
            serde_json::from_str::<StateTransitionKpiRequest>(request_json).map_err(|err| {
                OcelError::new(format!(
                    "could not parse state transition KPI request: {err}"
                ))
            })?;
        self.log.state_transition_kpis_json(&request)
    }

    /// Searches active-log object IDs for lifecycle inspection.
    pub fn object_search_json(&self, request_json: &str) -> OcelResult<String> {
        let request = serde_json::from_str::<ObjectSearchRequest>(request_json).map_err(|err| {
            OcelError::new(format!("could not parse object search request: {err}"))
        })?;
        self.log.object_search_json(&request)
    }

    /// Returns rich event, state, stock, and related-object timeline data for one object.
    pub fn object_lifecycle_detail_json(&self, object_id: &str) -> OcelResult<String> {
        self.log.object_lifecycle_detail_json(object_id)
    }

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

