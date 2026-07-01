// WebAssembly bindings for the Flowvault OCEL core.
//
// The public WebAssembly API accepts standard OCEL 2.0 JSON and XML files and
// delegates parsing, filtering, export, and analysis to `ocel_core`.

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
