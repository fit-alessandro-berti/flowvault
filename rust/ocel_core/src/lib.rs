//! OCEL 2.0 import/export and analysis core.
//!
//! This crate owns the format parsing, compact in-memory representation,
//! filtering, export, graph, state, and causal-analysis behavior used by the
//! WebAssembly adapter.

mod document;
mod error;

pub use document::OcelDocumentCore;
pub use error::{OcelError, OcelResult};
