mod common;

use common::{
    assert_same_structural_summary, compressed_ocel_fixture_paths, fixture_dir, json_value,
    ocel_fixture_paths, JSON_EXAMPLE, XML_EXAMPLE,
};
use ocel_core::OcelDocumentCore;
use std::fs;

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

        assert!(summary["event_types"].as_u64().unwrap() > 0);
        assert!(summary["object_types"].as_u64().unwrap() > 0);
        assert!(summary["events"].as_u64().unwrap() > 0);
        assert!(summary["objects"].as_u64().unwrap() > 0);
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
