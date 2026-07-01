use ocel_core::OcelDocumentCore;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const JSON_EXAMPLE: &str = include_str!("../../../files/ocel2/ocel20_example.json");
const XML_EXAMPLE: &str = include_str!("../../../files/ocel2/ocel20_example.xml");

#[test]
fn imports_json_and_xml_examples() {
    let json_doc = OcelDocumentCore::new(JSON_EXAMPLE, Some("json")).unwrap();
    let json_summary = json_value(&json_doc.summary_json());

    assert_eq!(json_summary["source_format"], "json");
    assert_eq!(json_summary["event_types"], 8);
    assert_eq!(json_summary["object_types"], 4);
    assert_eq!(json_summary["events"], 13);
    assert_eq!(json_summary["objects"], 9);
    assert_eq!(json_summary["e2o_relationships"], 20);
    assert_eq!(json_summary["o2o_relationships"], 7);
    assert_eq!(json_summary["objects_with_lifecycle"], 9);

    let xml_doc = OcelDocumentCore::new(XML_EXAMPLE, Some("xml")).unwrap();
    let xml_summary = json_value(&xml_doc.summary_json());

    assert_eq!(xml_summary["source_format"], "xml");
    assert_eq!(xml_summary["event_types"], json_summary["event_types"]);
    assert_eq!(xml_summary["object_types"], json_summary["object_types"]);
    assert_eq!(xml_summary["events"], json_summary["events"]);
    assert_eq!(xml_summary["objects"], json_summary["objects"]);
}

#[test]
fn exports_round_trip_without_changing_structural_counts() {
    let doc = OcelDocumentCore::new(JSON_EXAMPLE, Some("json")).unwrap();
    let original = json_value(&doc.summary_json());

    let exported_json = doc.export_json().unwrap();
    let reparsed_json = OcelDocumentCore::new(&exported_json, Some("json")).unwrap();
    assert_same_structural_summary(&json_value(&reparsed_json.summary_json()), &original);

    let exported_xml = doc.export_xml().unwrap();
    let reparsed_xml = OcelDocumentCore::new(&exported_xml, Some("xml")).unwrap();
    assert_same_structural_summary(&json_value(&reparsed_xml.summary_json()), &original);
}

#[test]
fn exposes_lifecycle_filter_and_state_query_api() {
    let mut doc = OcelDocumentCore::new(JSON_EXAMPLE, Some("json")).unwrap();

    assert_eq!(
        doc.object_lifecycle_json("PO1").unwrap(),
        r#"["e3","e4","e5","e6"]"#
    );

    let state_result = json_value(
        &doc.apply_state_query(
            r#"
            STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
              WHEN object.is_blocked = 'Yes' THEN 'Blocked'
              WHEN event.type LIKE '%Payment%' THEN 'Payment'
              ELSE 'Normal'
            END
            "#,
        )
        .unwrap(),
    );
    assert_eq!(state_result["leading_object_type"], "Invoice");
    assert!(state_result["assigned_events"]
        .as_u64()
        .is_some_and(|count| count > 0));

    let filtered_summary = json_value(
        &doc.apply_filter(
            r#"{
                "event_types":["Create Purchase Order","Insert Invoice"],
                "object_types":["Purchase Order","Invoice"]
            }"#,
        )
        .unwrap(),
    );
    let original_summary = json_value(&doc.original_summary_json());

    assert!(
        filtered_summary["events"].as_u64().unwrap() < original_summary["events"].as_u64().unwrap()
    );
    assert!(
        filtered_summary["objects"].as_u64().unwrap()
            < original_summary["objects"].as_u64().unwrap()
    );
    assert!(filtered_summary["stateful_events"].as_u64().unwrap() > 0);

    let options = json_value(&doc.filter_options_json());
    assert!(options["event_types"]
        .as_array()
        .is_some_and(|types| types.iter().any(|value| value == "Create Purchase Order")));
    assert!(options["text_attributes"]
        .as_array()
        .is_some_and(|attributes| attributes
            .iter()
            .any(|attribute| { attribute["scope"] == "event" && attribute["name"] == "state" })));
}

#[test]
fn exposes_graph_state_detection_and_causal_json_endpoints() {
    let mut doc = OcelDocumentCore::new(JSON_EXAMPLE, Some("json")).unwrap();
    doc.apply_state_query(
        r#"
        STATE state FOR LEADING OBJECT TYPE 'Purchase Order' AS CASE
          WHEN event.type = 'Create Purchase Order' THEN 'Opening'
          WHEN event.type = 'Pay Invoice' THEN 'Closing'
          ELSE 'Processing'
        END
        "#,
    )
    .unwrap();

    let dfg = json_value(&doc.directly_follows_graph_json("Purchase Order").unwrap());
    assert_eq!(dfg["title"], "Directly-Follows Graph: Purchase Order");
    assert!(dfg["nodes"]
        .as_array()
        .is_some_and(|nodes| !nodes.is_empty()));
    assert!(dfg["edges"]
        .as_array()
        .is_some_and(|edges| !edges.is_empty()));

    let filtered_ocdfg = json_value(
        &doc.filtered_object_centric_directly_follows_graph_json(
            r#"{
                "object_types":["Invoice"],
                "min_activity_frequency":2,
                "min_path_frequency":2
            }"#,
        )
        .unwrap(),
    );
    assert!(filtered_ocdfg["nodes"]
        .as_array()
        .is_some_and(|nodes| !nodes.is_empty()));

    let patterns = json_value(&doc.state_patterns_json().unwrap());
    assert!(patterns["intra"]
        .as_array()
        .is_some_and(|patterns| !patterns.is_empty()));
    assert!(patterns["inter"].as_array().is_some());

    let detection_request = r#"{
        "object_type":"Purchase Order",
        "window_size":2,
        "som_width":3,
        "som_height":2,
        "epochs":25
    }"#;
    let detection = json_value(&doc.state_detection_json(detection_request).unwrap());
    assert_eq!(detection["object_type"], "Purchase Order");
    assert!(detection["windows"]
        .as_array()
        .is_some_and(|windows| !windows.is_empty()));

    let table = json_value(
        &doc.causal_feature_table_json(r#"{"object_type":"Purchase Order"}"#)
            .unwrap(),
    );
    assert!(table["feature_columns"]
        .as_array()
        .is_some_and(|columns| !columns.is_empty()));
    assert!(doc
        .causal_feature_table_csv(r#"{"object_type":"Purchase Order"}"#)
        .unwrap()
        .contains("object_id"));
}

#[test]
fn imports_all_fixture_files_and_compressed_variants() {
    for fixture_path in ocel_fixture_paths() {
        let fixture_name = fixture_path.display().to_string();
        let input = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {fixture_name}: {err}"));
        let format_hint = fixture_path
            .extension()
            .and_then(|extension| extension.to_str())
            .expect("fixture should have an extension");
        let doc = OcelDocumentCore::new(&input, Some(format_hint))
            .unwrap_or_else(|err| panic!("failed to import {fixture_name}: {err}"));
        let summary = json_value(&doc.summary_json());

        assert!(
            summary["event_types"].as_u64().unwrap() > 0,
            "{fixture_name}"
        );
        assert!(
            summary["object_types"].as_u64().unwrap() > 0,
            "{fixture_name}"
        );
        assert!(summary["events"].as_u64().unwrap() > 0, "{fixture_name}");
        assert!(summary["objects"].as_u64().unwrap() > 0, "{fixture_name}");
    }

    for compressed_path in compressed_ocel_fixture_paths() {
        let compressed_name = compressed_path.display().to_string();
        let compressed_bytes = fs::read(&compressed_path)
            .unwrap_or_else(|err| panic!("failed to read {compressed_name}: {err}"));
        let file_name = compressed_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("compressed fixture should have a file name");
        let compressed_doc = OcelDocumentCore::from_bytes(&compressed_bytes, Some(file_name))
            .unwrap_or_else(|err| panic!("failed to import {compressed_name}: {err}"));

        let uncompressed_file_name = file_name
            .strip_suffix(".gz")
            .expect("compressed fixture should end with .gz");
        let uncompressed_path = fixture_dir("ocel2").join(uncompressed_file_name);
        let uncompressed_input = fs::read_to_string(&uncompressed_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", uncompressed_path.display()));
        let uncompressed_doc =
            OcelDocumentCore::new(&uncompressed_input, Some(uncompressed_file_name)).unwrap();

        assert_same_structural_summary(
            &json_value(&compressed_doc.summary_json()),
            &json_value(&uncompressed_doc.summary_json()),
        );
    }
}

#[test]
fn returns_clear_import_errors() {
    let input = r#"{
      "eventTypes": [{"name": "a", "attributes": []}],
      "objectTypes": [{"name": "o", "attributes": []}],
      "events": [{
        "id": "e1",
        "type": "a",
        "time": "1970-01-01T00:00:00Z",
        "relationships": [{"objectId": "missing", "qualifier": "x"}]
      }],
      "objects": [{"id": "o1", "type": "o"}]
    }"#;

    let error = match OcelDocumentCore::new(input, Some("json")) {
        Ok(_) => panic!("import should reject an unknown relationship target"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("unknown object 'missing'"));
}

fn json_value(input: &str) -> Value {
    serde_json::from_str(input).unwrap()
}

fn assert_same_structural_summary(actual: &Value, expected: &Value) {
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

fn ocel_fixture_paths() -> Vec<PathBuf> {
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

fn compressed_ocel_fixture_paths() -> Vec<PathBuf> {
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

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../files")
        .join(name)
}
