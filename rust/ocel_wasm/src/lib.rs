//! WebAssembly bindings for the Flowvault OCEL core.
//!
//! The public WebAssembly API accepts standard OCEL 2.0 JSON and XML files and
//! delegates parsing, filtering, export, and analysis to `ocel_core`.

use ocel_core::{OcelDocumentCore, OcelResult};
use wasm_bindgen::prelude::*;

fn into_js_result<T>(result: OcelResult<T>) -> Result<T, JsValue> {
    result.map_err(|err| JsValue::from_str(&err.to_string()))
}

/// Parsed OCEL document exposed to JavaScript.
///
/// Constructing this type imports and validates the OCEL text once. Subsequent
/// summary/export calls reuse the compact in-memory representation.
#[wasm_bindgen]
pub struct OcelDocument {
    inner: OcelDocumentCore,
}

#[wasm_bindgen]
impl OcelDocument {
    /// Imports an OCEL 2.0 JSON or XML string.
    ///
    /// `format_hint` may be `"json"`, `"xml"`, a filename, or `undefined`.
    /// When omitted, the parser detects JSON/XML from the first non-whitespace
    /// character.
    #[wasm_bindgen(constructor)]
    pub fn new(input: &str, format_hint: Option<String>) -> Result<OcelDocument, JsValue> {
        let inner = into_js_result(OcelDocumentCore::new(input, format_hint.as_deref()))?;
        Ok(Self { inner })
    }

    /// Imports an OCEL 2.0 JSON/XML file from raw bytes.
    ///
    /// The byte input may be plain UTF-8 JSON/XML or a gzip-compressed `.gz`
    /// file containing UTF-8 JSON/XML.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(input: &[u8], format_hint: Option<String>) -> Result<OcelDocument, JsValue> {
        let inner = into_js_result(OcelDocumentCore::from_bytes(input, format_hint.as_deref()))?;
        Ok(Self { inner })
    }

    /// Returns summary counts as a JSON string.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> String {
        self.inner.summary_json()
    }

    /// Returns summary counts for the unfiltered imported document as a JSON string.
    #[wasm_bindgen(js_name = originalSummaryJson)]
    pub fn original_summary_json(&self) -> String {
        self.inner.original_summary_json()
    }

    /// Returns available event and object types for filtering as a JSON string.
    #[wasm_bindgen(js_name = filterOptionsJson)]
    pub fn filter_options_json(&self) -> String {
        self.inner.filter_options_json()
    }

    /// Rebuilds the active log from the original, possibly state-enriched, log.
    #[wasm_bindgen(js_name = applyFilter)]
    pub fn apply_filter(&mut self, filter_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.apply_filter(filter_json))
    }

    /// Exports the document as OCEL 2.0 JSON.
    #[wasm_bindgen(js_name = exportJson)]
    pub fn export_json(&self) -> Result<String, JsValue> {
        into_js_result(self.inner.export_json())
    }

    /// Exports the document as OCEL 2.0 XML.
    #[wasm_bindgen(js_name = exportXml)]
    pub fn export_xml(&self) -> Result<String, JsValue> {
        into_js_result(self.inner.export_xml())
    }

    /// Returns the ordered event IDs related to an object ID as a JSON array.
    #[wasm_bindgen(js_name = objectLifecycleJson)]
    pub fn object_lifecycle_json(&self, object_id: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.object_lifecycle_json(object_id))
    }

    /// Applies a SQL-like CASE query and writes a string state attribute to events.
    #[wasm_bindgen(js_name = applyStateQuery)]
    pub fn apply_state_query(&mut self, query: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.apply_state_query(query))
    }

    /// Applies the current State Detection SOM labels as the string `state` event attribute.
    #[wasm_bindgen(js_name = applyStateDetection)]
    pub fn apply_state_detection(&mut self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.apply_state_detection(request_json))
    }

    /// Detects ranked intra-state and inter-state behavioral patterns.
    #[wasm_bindgen(js_name = statePatternsJson)]
    pub fn state_patterns_json(&self) -> Result<String, JsValue> {
        into_js_result(self.inner.state_patterns_json())
    }

    /// Extracts object-level features and detects execution-state cells with PCA and SOM.
    #[wasm_bindgen(js_name = stateDetectionJson)]
    pub fn state_detection_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.state_detection_json(request_json))
    }

    /// Returns details for one SOM cell, including a DFG and entering/exiting windows.
    #[wasm_bindgen(js_name = stateDetectionCellJson)]
    pub fn state_detection_cell_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.state_detection_cell_json(request_json))
    }

    /// Returns the object-level numerical feature table as CSV.
    #[wasm_bindgen(js_name = stateFeatureTableCsv)]
    pub fn state_feature_table_csv(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.state_feature_table_csv(request_json))
    }

    /// Correlates object-level features with the currently applied event state.
    #[wasm_bindgen(js_name = stateCorrelationsJson)]
    pub fn state_correlations_json(&self) -> Result<String, JsValue> {
        into_js_result(self.inner.state_correlations_json())
    }

    /// Returns smoothed time-perspective inputs for state frequencies and transition durations.
    #[wasm_bindgen(js_name = timePerspectiveJson)]
    pub fn time_perspective_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.time_perspective_json(request_json))
    }

    /// Returns transition matrix inputs, dwell times, recovery rows, and stuck-state rankings.
    #[wasm_bindgen(js_name = stateTransitionKpisJson)]
    pub fn state_transition_kpis_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.state_transition_kpis_json(request_json))
    }

    /// Searches active-log object IDs for lifecycle inspection.
    #[wasm_bindgen(js_name = objectSearchJson)]
    pub fn object_search_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.object_search_json(request_json))
    }

    /// Returns rich event, state, stock, and related-object timeline data for one object.
    #[wasm_bindgen(js_name = objectLifecycleDetailJson)]
    pub fn object_lifecycle_detail_json(&self, object_id: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.object_lifecycle_detail_json(object_id))
    }

    /// Returns object-level feature table metadata and a preview for causal-model editing.
    #[wasm_bindgen(js_name = causalFeatureTableJson)]
    pub fn causal_feature_table_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.causal_feature_table_json(request_json))
    }

    /// Returns the object-level causal feature table as CSV.
    #[wasm_bindgen(js_name = causalFeatureTableCsv)]
    pub fn causal_feature_table_csv(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.causal_feature_table_csv(request_json))
    }

    /// Fits a simple DAG-constrained causal model over the object-level feature table.
    #[wasm_bindgen(js_name = fitCausalModelJson)]
    pub fn fit_causal_model_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.fit_causal_model_json(request_json))
    }

    /// Computes a flattened directly-follows graph for one object type.
    #[wasm_bindgen(js_name = directlyFollowsGraphJson)]
    pub fn directly_follows_graph_json(&self, object_type: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.directly_follows_graph_json(object_type))
    }

    /// Computes an object-centric directly-follows graph collated over all object types.
    #[wasm_bindgen(js_name = objectCentricDirectlyFollowsGraphJson)]
    pub fn object_centric_directly_follows_graph_json(&self) -> Result<String, JsValue> {
        into_js_result(self.inner.object_centric_directly_follows_graph_json())
    }

    /// Computes an object-centric directly-follows graph for selected object types and frequencies.
    #[wasm_bindgen(js_name = filteredObjectCentricDirectlyFollowsGraphJson)]
    pub fn filtered_object_centric_directly_follows_graph_json(
        &self,
        request_json: &str,
    ) -> Result<String, JsValue> {
        into_js_result(
            self.inner
                .filtered_object_centric_directly_follows_graph_json(request_json),
        )
    }

    /// Computes a state-aware object-centric directly-follows graph.
    #[wasm_bindgen(js_name = stateAwareObjectCentricDirectlyFollowsGraphJson)]
    pub fn state_aware_ocdfg_json(&self) -> Result<String, JsValue> {
        into_js_result(self.inner.state_aware_ocdfg_json())
    }

    /// Computes a state-aware OCDFG for selected object types and frequencies.
    #[wasm_bindgen(js_name = filteredStateAwareObjectCentricDirectlyFollowsGraphJson)]
    pub fn filtered_state_aware_ocdfg_json(&self, request_json: &str) -> Result<String, JsValue> {
        into_js_result(self.inner.filtered_state_aware_ocdfg_json(request_json))
    }
}
