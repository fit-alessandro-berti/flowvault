#![allow(dead_code)]

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const JSON_EXAMPLE: &str = include_str!("../../../../files/ocel2/ocel20_example.json");
pub const XML_EXAMPLE: &str = include_str!("../../../../files/ocel2/ocel20_example.xml");

pub fn json_value(input: &str) -> Value {
    serde_json::from_str(input).unwrap()
}

pub fn assert_same_structural_summary(actual: &Value, expected: &Value) {
    for key in [
        "event_types",
        "object_types",
        "events",
        "objects",
        "e2o_relationships",
        "o2o_relationships",
        "objects_with_lifecycle",
    ] {
        assert_eq!(actual[key], expected[key], "{key}");
    }
}

pub fn ocel_fixture_paths() -> Vec<PathBuf> {
    let mut paths = fs::read_dir(fixture_dir("ocel2"))
        .expect("failed to read OCEL fixture directory")
        .map(|entry| {
            entry
                .expect("failed to read fixture directory entry")
                .path()
        })
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| matches!(extension, "json" | "xml" | "jsonocel" | "xmlocel"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn compressed_ocel_fixture_paths() -> Vec<PathBuf> {
    let mut paths = fs::read_dir(fixture_dir("ocel2_compressed"))
        .expect("failed to read compressed OCEL fixture directory")
        .map(|entry| {
            entry
                .expect("failed to read compressed fixture directory entry")
                .path()
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| {
                    name.ends_with(".json.gz")
                        || name.ends_with(".xml.gz")
                        || name.ends_with(".jsonocel.gz")
                        || name.ends_with(".xmlocel.gz")
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../files")
        .join(name)
}
