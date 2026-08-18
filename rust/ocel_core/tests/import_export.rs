mod common;

use common::{
    assert_same_structural_summary, compressed_ocel_fixture_paths, fixture_dir, json_value,
    ocel_fixture_paths, structured_ocel_fixture_paths, JSON_EXAMPLE, XML_EXAMPLE,
};
use ocel_core::OcelDocumentCore;
use serde_json::{Number, Value};
use std::fs;
use std::io::Write;

const PRECISION_AND_ESCAPING_EXAMPLE: &str = r#"{
  "eventTypes": [{
    "name": "measure",
    "attributes": [
      {"name":"text","type":"string"},
      {"name":"count","type":"integer"},
      {"name":"ratio","type":"float"},
      {"name":"active","type":"boolean"},
      {"name":"observed","type":"time"}
    ]
  }],
  "objectTypes": [{
    "name": "Type / #",
    "attributes": [
      {"name":"label","type":"string"},
      {"name":"amount","type":"integer"},
      {"name":"ratio","type":"float"},
      {"name":"active","type":"boolean"},
      {"name":"observed","type":"time"}
    ]
  }],
  "events": [{
    "id": "event-1",
    "type": "measure",
    "time": "2026-08-18T06:00:00.123456Z",
    "attributes": [
      {"name":"text","value":"plain"},
      {"name":"count","value":42},
      {"name":"ratio","value":1.0},
      {"name":"active","value":true},
      {"name":"observed","value":"2026-08-18T06:00:00.654321Z"}
    ],
    "relationships": [{
      "objectId": "object/#\\{",
      "qualifier": "q/#\\{"
    }]
  }],
  "objects": [
    {
      "id": "object/#\\{",
      "type": "Type / #",
      "attributes": [
        {"name":"label","time":"1970-01-01T00:00:00Z","value":"plain"},
        {"name":"amount","time":"1970-01-01T00:00:00Z","value":7},
        {"name":"ratio","time":"1970-01-01T00:00:00Z","value":1.0},
        {"name":"active","time":"1970-01-01T00:00:00Z","value":true},
        {"name":"observed","time":"1970-01-01T00:00:00Z","value":"2026-08-18T06:00:00.654321Z"},
        {"name":"ratio","time":"2026-08-18T06:00:00.222333Z","value":2.5}
      ],
      "relationships": [{
        "objectId": "target/2",
        "qualifier": "contains/#\\{"
      }]
    },
    {"id":"target/2","type":"Type / #","attributes":[],"relationships":[]}
  ]
}"#;

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

    let exported_csv = doc.export_csv().unwrap();
    let reparsed_csv = OcelDocumentCore::new(&exported_csv, Some("csv")).unwrap();
    assert_same_structural_summary(&json_value(&reparsed_csv.summary_json()), &original);

    let exported_sqlite = doc.export_sqlite().unwrap();
    let reparsed_sqlite =
        OcelDocumentCore::from_bytes(&exported_sqlite, Some("roundtrip.sqlite")).unwrap();
    assert_same_structural_summary(&json_value(&reparsed_sqlite.summary_json()), &original);

    let exported_bundle = doc.export_bundle().unwrap();
    let reparsed_bundle =
        OcelDocumentCore::from_bytes(&exported_bundle, Some("roundtrip.ocel.zip")).unwrap();
    assert_same_structural_summary(&json_value(&reparsed_bundle.summary_json()), &original);
}

#[test]
fn structured_formats_preserve_microseconds_typed_values_and_escaped_references() {
    let document = OcelDocumentCore::new(PRECISION_AND_ESCAPING_EXAMPLE, Some("json")).unwrap();
    let expected = canonical_ocel_json(&document.export_json().unwrap(), false, false);

    let csv = document.export_csv().unwrap();
    assert!(csv.contains(r"object\/\#"));
    let csv_document = OcelDocumentCore::new(&csv, Some("csv")).unwrap();
    assert_eq!(
        canonical_ocel_json(&csv_document.export_json().unwrap(), false, false),
        expected,
    );

    let sqlite = document.export_sqlite().unwrap();
    let sqlite_document = OcelDocumentCore::from_bytes(&sqlite, Some("precision.sqlite3")).unwrap();
    assert_eq!(
        canonical_ocel_json(&sqlite_document.export_json().unwrap(), false, false),
        expected,
    );

    let bundle = document.export_bundle().unwrap();
    let bundle_document =
        OcelDocumentCore::from_bytes(&bundle, Some("precision.ocel.zip")).unwrap();
    assert_eq!(
        canonical_ocel_json(&bundle_document.export_json().unwrap(), false, false),
        expected,
    );
}

#[test]
fn imports_checked_in_csv_sqlite_and_bundle_fixtures() {
    for fixture_path in structured_ocel_fixture_paths() {
        let fixture_name = fixture_path.display().to_string();
        let bytes = fs::read(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {fixture_name}: {err}"));
        let file_name = fixture_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("fixture should have a file name");
        let document = OcelDocumentCore::from_bytes(&bytes, Some(file_name))
            .unwrap_or_else(|err| panic!("failed to import {fixture_name}: {err}"));
        let summary = json_value(&document.summary_json());

        let expected_format = if file_name.ends_with(".ocel.csv") {
            "csv"
        } else if file_name.ends_with(".sqlite") {
            "sqlite"
        } else {
            "bundle"
        };
        assert_eq!(summary["source_format"], expected_format, "{fixture_name}");

        let base_name = file_name
            .strip_suffix(".ocel.csv")
            .or_else(|| file_name.strip_suffix(".sqlite"))
            .or_else(|| file_name.strip_suffix(".ocel.zip"))
            .expect("known fixture suffix");
        let json_path = fixture_dir("ocel2").join(format!("{base_name}.json"));
        let json_input = fs::read_to_string(&json_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", json_path.display()));
        let json_document = OcelDocumentCore::new(&json_input, Some("json")).unwrap();
        assert_same_structural_summary(&summary, &json_value(&json_document.summary_json()));
        let csv_scalar_inference = expected_format == "csv";
        let actual =
            canonical_ocel_json(&document.export_json().unwrap(), true, csv_scalar_inference);
        let expected = canonical_ocel_json(
            &json_document.export_json().unwrap(),
            true,
            csv_scalar_inference,
        );
        if actual != expected {
            panic!(
                "semantic content differs for {fixture_name}: {}",
                first_json_difference(&actual, &expected, "$"),
            );
        }
    }
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

#[test]
fn rejects_malformed_structured_formats_with_clear_errors() {
    let csv_error = OcelDocumentCore::new(
        "id,activity,timestamp\ne1,create,not-a-timestamp\n",
        Some("csv"),
    )
    .err()
    .expect("CSV with an invalid timestamp should fail");
    assert!(csv_error.to_string().contains("invalid OCEL CSV timestamp"));

    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute("CREATE TABLE unrelated (value TEXT)", [])
        .unwrap();
    let invalid_sqlite = connection.serialize("main").unwrap().to_vec();
    let sqlite_error = OcelDocumentCore::from_bytes(&invalid_sqlite, Some("sqlite"))
        .err()
        .expect("SQLite with a missing OCEL schema should fail");
    assert!(sqlite_error.to_string().contains("event_map_type"));

    let valid_document =
        OcelDocumentCore::new(PRECISION_AND_ESCAPING_EXAMPLE, Some("json")).unwrap();
    let valid_sqlite = valid_document.export_sqlite().unwrap();
    let mut connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .deserialize_read_exact("main", valid_sqlite.as_slice(), valid_sqlite.len(), false)
        .unwrap();
    let object_mapping: String = connection
        .query_row(
            "SELECT ocel_type_map FROM object_map_type WHERE ocel_type = 'Type / #'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute(
            &format!(
                "UPDATE \"object_{object_mapping}\" SET \"active\" = 2 WHERE \"active\" IS NOT NULL"
            ),
            [],
        )
        .unwrap();
    let invalid_boolean_sqlite = connection.serialize("main").unwrap().to_vec();
    let boolean_error = OcelDocumentCore::from_bytes(&invalid_boolean_sqlite, Some("sqlite"))
        .err()
        .expect("SQLite BOOLEAN values other than 0 and 1 should fail");
    assert!(boolean_error.to_string().contains("expected 0 or 1"));

    let bundle_error = OcelDocumentCore::from_bytes(b"PK\x03\x04", Some("bundle"))
        .err()
        .expect("malformed bundle should fail");
    assert!(bundle_error.to_string().contains("bundle"));

    let invalid_metadata = serde_json::json!({
        "ocelVersion": "3.0",
        "bundleFormatVersion": "1.0",
        "storageFormat": "parquet",
        "eventTypes": {},
        "objectTypes": {},
        "relations": {"e2o": "relations/e2o.parquet", "o2o": "relations/o2o.parquet"}
    });
    let invalid_bundle = bundle_with_metadata(&invalid_metadata.to_string());
    let bundle_schema_error = OcelDocumentCore::from_bytes(&invalid_bundle, Some("bundle"))
        .err()
        .expect("bundle with an unsupported OCEL version should fail");
    assert!(bundle_schema_error
        .to_string()
        .contains("unsupported OCEL bundle OCEL version"));
}

#[test]
fn csv_export_rejects_schema_elements_the_format_cannot_reconstruct() {
    let document = OcelDocumentCore::new(
        r#"{
          "eventTypes":[{"name":"unused","attributes":[]}],
          "objectTypes":[],
          "events":[],
          "objects":[]
        }"#,
        Some("json"),
    )
    .unwrap();
    let error = document
        .export_csv()
        .expect_err("an event type without events is not representable in CSV");
    assert!(error.to_string().contains("cannot be reconstructed"));
}

fn bundle_with_metadata(metadata: &str) -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut archive = zip::ZipWriter::new(cursor);
    archive
        .start_file("ocel-meta.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(metadata.as_bytes()).unwrap();
    archive.finish().unwrap().into_inner()
}

fn canonical_ocel_json(
    input: &str,
    normalize_attribute_types: bool,
    stringify_attribute_values: bool,
) -> Value {
    fn canonicalize(
        value: &mut Value,
        normalize_attribute_types: bool,
        stringify_attribute_values: bool,
    ) {
        match value {
            Value::Array(values) => {
                for value in values.iter_mut() {
                    canonicalize(value, normalize_attribute_types, stringify_attribute_values);
                }
                values.sort_by_cached_key(|value| value.to_string());
            }
            Value::Object(values) => {
                if normalize_attribute_types
                    && values.len() == 2
                    && values.contains_key("name")
                    && values.contains_key("type")
                {
                    values.remove("type");
                }
                if stringify_attribute_values && values.contains_key("name") {
                    if let Some(attribute_value) = values.get_mut("value") {
                        match attribute_value {
                            Value::Bool(value) => {
                                *attribute_value = Value::String(value.to_string());
                            }
                            Value::Number(value) => {
                                let text = value
                                    .as_f64()
                                    .filter(|value| value.fract() == 0.0)
                                    .map(|value| format!("{value:.0}"))
                                    .unwrap_or_else(|| value.to_string());
                                *attribute_value = Value::String(text);
                            }
                            Value::String(value) => {
                                if let Some(number) = serde_json::from_str::<Number>(value)
                                    .ok()
                                    .and_then(|number| number.as_f64())
                                    .filter(|number| number.is_finite())
                                {
                                    *value = if number.fract() == 0.0 {
                                        format!("{number:.0}")
                                    } else {
                                        number.to_string()
                                    };
                                }
                            }
                            Value::Null | Value::Array(_) | Value::Object(_) => {}
                        }
                    }
                }
                for value in values.values_mut() {
                    canonicalize(value, normalize_attribute_types, stringify_attribute_values);
                }
            }
            Value::Number(number) if normalize_attribute_types => {
                if let Some(integer) = number
                    .as_f64()
                    .filter(|number| number.fract() == 0.0)
                    .and_then(|number| i64::try_from(number as i128).ok())
                {
                    *value = Value::Number(integer.into());
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    let mut value = json_value(input);
    canonicalize(
        &mut value,
        normalize_attribute_types,
        stringify_attribute_values,
    );
    value
}

fn first_json_difference(actual: &Value, expected: &Value, path: &str) -> String {
    match (actual, expected) {
        (Value::Array(actual), Value::Array(expected)) => {
            if actual.len() != expected.len() {
                return format!(
                    "{path} has {} entries, expected {}",
                    actual.len(),
                    expected.len()
                );
            }
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                if actual != expected {
                    return first_json_difference(actual, expected, &format!("{path}[{index}]"));
                }
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            let actual_keys = actual.keys().collect::<Vec<_>>();
            let expected_keys = expected.keys().collect::<Vec<_>>();
            if actual_keys != expected_keys {
                return format!("{path} has keys {actual_keys:?}, expected {expected_keys:?}");
            }
            for key in actual.keys() {
                if actual[key] != expected[key] {
                    return first_json_difference(
                        &actual[key],
                        &expected[key],
                        &format!("{path}.{key}"),
                    );
                }
            }
        }
        _ if actual != expected => {
            return format!("{path} is {actual}, expected {expected}");
        }
        _ => {}
    }
    "unknown difference".to_owned()
}
