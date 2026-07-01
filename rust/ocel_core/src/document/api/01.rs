/// Parsed OCEL document with a stable Rust API.
///
/// Constructing this type imports and validates the OCEL text once. Subsequent
/// summary/export calls reuse the compact in-memory representation.
pub struct OcelDocumentCore {
    original_log: CompactOcelLog,
    log: CompactOcelLog,
    current_filter: OcelFilterRequest,
}
