mod common;

use common::json_value;
use ocel_core::OcelDocumentCore;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const OBSERVED: &str =
    include_str!("../../../files/ocel2/evaluation/inventory_golden/observed.ocel.json");
const BEHAVIOR: &str =
    include_str!("../../../files/ocel2/evaluation/inventory_golden/observed.behavior.ocel.json");
const QUERY: &str = include_str!("../../../queries/inventory_state.sql");
const STATE_TRUTH: &str =
    include_str!("../../../files/ocel2/evaluation/inventory_golden/truth/state_at_event.csv");
const TRANSITION_TRUTH: &str =
    include_str!("../../../files/ocel2/evaluation/inventory_golden/truth/transitions.csv");
const MANIFEST: &str =
    include_str!("../../../files/ocel2/evaluation/inventory_golden/manifest.json");

fn csv_rows(input: &str) -> Vec<BTreeMap<String, String>> {
    let mut reader = csv::Reader::from_reader(input.as_bytes());
    let headers = reader.headers().unwrap().clone();
    reader
        .records()
        .map(|record| {
            headers
                .iter()
                .zip(record.unwrap().iter())
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect()
        })
        .collect()
}

fn exported_event_states(exported: &Value) -> BTreeMap<String, String> {
    exported["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| {
            let state = event["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|attribute| attribute["name"] == "state")?;
            Some((
                event["id"].as_str().unwrap().to_owned(),
                state["value"].as_str().unwrap().to_owned(),
            ))
        })
        .collect()
}

fn assert_same_structural_summary(actual: &Value, expected: &Value) {
    for field in [
        "event_types",
        "object_types",
        "events",
        "objects",
        "e2o_relationships",
        "o2o_relationships",
    ] {
        assert_eq!(actual[field], expected[field], "mismatch in {field}");
    }
}

#[test]
fn inventory_golden_import_and_state_query_match_independent_oracle() {
    let manifest = json_value(MANIFEST);
    let mut doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    let summary = json_value(&doc.summary_json());
    assert_eq!(summary["events"], manifest["counts"]["events"]);
    assert_eq!(summary["objects"], manifest["counts"]["objects"]);
    assert_eq!(summary["e2o_relationships"], manifest["counts"]["e2o"]);
    assert_eq!(summary["o2o_relationships"], manifest["counts"]["o2o"]);

    let result = json_value(&doc.apply_state_query(QUERY).unwrap());
    let truth = csv_rows(STATE_TRUTH)
        .into_iter()
        .map(|row| (row["event_id"].clone(), row["reference_state"].clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(result["leading_object_type"], "ItemLocation");
    assert_eq!(result["assigned_events"], truth.len());

    let exported = json_value(&doc.export_json().unwrap());
    assert_eq!(exported_event_states(&exported), truth);
}

#[test]
fn inventory_golden_round_trips_through_every_supported_exchange_format() {
    let doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    let expected = json_value(&doc.summary_json());

    let json = doc.export_json().unwrap();
    let from_json = OcelDocumentCore::new(&json, Some("json")).unwrap();
    assert_same_structural_summary(&json_value(&from_json.summary_json()), &expected);

    let xml = doc.export_xml().unwrap();
    let from_xml = OcelDocumentCore::new(&xml, Some("xml")).unwrap();
    assert_same_structural_summary(&json_value(&from_xml.summary_json()), &expected);

    // The wide OCEL CSV exchange format has no schema-only row with which to
    // encode a declared event type that has zero instances. Use the complete
    // evaluation event/object population with only those empty declarations
    // removed for the CSV round trip.
    let mut csv_source = json_value(OBSERVED);
    let used_event_types = csv_source["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["type"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    csv_source["eventTypes"]
        .as_array_mut()
        .unwrap()
        .retain(|type_def| used_event_types.contains(type_def["name"].as_str().unwrap()));
    let csv_document =
        OcelDocumentCore::new(&serde_json::to_string(&csv_source).unwrap(), Some("json")).unwrap();
    let csv_expected = json_value(&csv_document.summary_json());
    let csv = csv_document.export_csv().unwrap();
    let from_csv = OcelDocumentCore::new(&csv, Some("csv")).unwrap();
    assert_same_structural_summary(&json_value(&from_csv.summary_json()), &csv_expected);

    let sqlite = doc.export_sqlite().unwrap();
    let from_sqlite = OcelDocumentCore::from_bytes(&sqlite, Some("inventory.sqlite")).unwrap();
    assert_same_structural_summary(&json_value(&from_sqlite.summary_json()), &expected);

    let bundle = doc.export_bundle().unwrap();
    let from_bundle = OcelDocumentCore::from_bytes(&bundle, Some("inventory.ocel.zip")).unwrap();
    assert_same_structural_summary(&json_value(&from_bundle.summary_json()), &expected);
}

#[test]
fn inventory_missingness_is_unknown_and_filter_then_reapply_preserves_behavior() {
    let mut doc = OcelDocumentCore::new(BEHAVIOR, Some("json")).unwrap();
    doc.apply_filter(
        r#"{
            "event_types":["Data Gap Started","Data Gap Ended"],
            "object_types":["ItemLocation"]
        }"#,
    )
    .unwrap();
    doc.apply_state_query(QUERY).unwrap();

    let states = exported_event_states(&json_value(&doc.export_json().unwrap()));
    assert_eq!(
        states.get("INV-E-000036").map(String::as_str),
        Some("Unknown")
    );
    assert_eq!(
        states.get("INV-E-000037").map(String::as_str),
        Some("Normal")
    );

    let patterns = json_value(&doc.state_patterns_json().unwrap());
    let inter = patterns["inter"].as_array().unwrap();
    assert!(inter.iter().any(|pattern| {
        pattern["from_state"] == "Unknown"
            && pattern["to_state"] == "Normal"
            && pattern["sequence"].as_array().is_some_and(|sequence| {
                sequence
                    .iter()
                    .any(|item| item == "Data Gap Started [Unknown]")
                    && sequence
                        .iter()
                        .any(|item| item == "Data Gap Ended [Normal]")
            })
    }));
}

#[derive(Default)]
struct ExpectedTransition {
    durations: Vec<i64>,
    objects: BTreeSet<String>,
}

#[test]
fn inventory_golden_transition_kpis_match_independent_oracle() {
    let mut doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    doc.apply_state_query(QUERY).unwrap();
    let actual = json_value(
        &doc.state_transition_kpis_json(r#"{"object_type":"ItemLocation","stuck_limit":20}"#)
            .unwrap(),
    );
    let mut expected = BTreeMap::<(String, String), ExpectedTransition>::new();
    for row in csv_rows(TRANSITION_TRUTH) {
        let key = (row["from_state"].clone(), row["to_state"].clone());
        let accumulator = expected.entry(key).or_default();
        let duration_minutes = row["duration_minutes"].parse::<f64>().unwrap();
        accumulator
            .durations
            .push((duration_minutes * 60_000.0).round() as i64);
        accumulator.objects.insert(row["leading_object_id"].clone());
    }
    let actual_rows = actual["transitions"].as_array().unwrap();
    assert_eq!(actual_rows.len(), expected.len());
    for row in actual_rows {
        let key = (
            row["from_state"].as_str().unwrap().to_owned(),
            row["to_state"].as_str().unwrap().to_owned(),
        );
        let expected_row = expected.get_mut(&key).expect("unexpected transition row");
        expected_row.durations.sort();
        let total = expected_row.durations.iter().sum::<i64>();
        assert_eq!(row["count"], expected_row.durations.len());
        assert_eq!(row["object_count"], expected_row.objects.len());
        assert_eq!(
            row["min_duration_ms"].as_i64(),
            expected_row.durations.first().copied()
        );
        assert_eq!(
            row["median_duration_ms"],
            expected_row.durations[expected_row.durations.len() / 2]
        );
        assert_eq!(
            row["max_duration_ms"].as_i64(),
            expected_row.durations.last().copied()
        );
        let average = total as f64 / expected_row.durations.len() as f64;
        assert!((row["avg_duration_ms"].as_f64().unwrap() - average).abs() < 1e-6);
    }
}

#[test]
fn inventory_golden_state_aware_graph_and_patterns_expose_injected_mechanisms() {
    let mut doc = OcelDocumentCore::new(BEHAVIOR, Some("json")).unwrap();
    doc.apply_state_query(QUERY).unwrap();
    let graph = json_value(&doc.state_aware_ocdfg_json().unwrap());
    let labels = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["label"].as_str())
        .collect::<BTreeSet<_>>();
    for expected in [
        "CHANGE Normal -> Understock",
        "CHANGE Understock -> Critical Understock",
        "CHANGE Normal -> Overstock",
        "CHANGE Normal -> Unknown",
        "CHANGE Unknown -> Normal",
    ] {
        assert!(labels.contains(expected), "missing graph node {expected}");
    }

    let patterns = json_value(&doc.state_patterns_json().unwrap());
    let summaries = patterns["intra"]
        .as_array()
        .unwrap()
        .iter()
        .chain(patterns["inter"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert!(summaries
        .iter()
        .all(|pattern| pattern["support"].as_u64().unwrap() >= 1));
    for activity in [
        "Backorder Registered",
        "Delivery Delayed",
        "Transfer Receive",
        "Cycle Count Performed",
    ] {
        assert!(
            summaries.iter().any(|pattern| {
                pattern["sequence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item.as_str().is_some_and(|label| label.contains(activity)))
            }),
            "missing canonical activity {activity}"
        );
    }
}

#[test]
fn inventory_configured_patterns_return_bounded_occurrences() {
    let mut doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    doc.apply_state_query(QUERY).unwrap();
    let configured = json_value(
        &doc.state_patterns_with_request_json(
            r#"{
                "leading_object_type":"ItemLocation",
                "family":"inter",
                "pre_radius":2,
                "post_radius":2,
                "ignored_event_types":["Simulation End Snapshot"],
                "min_support":1,
                "include_occurrences":true
            }"#,
        )
        .unwrap(),
    );
    assert!(configured["intra"].as_array().unwrap().is_empty());
    let patterns = configured["inter"].as_array().unwrap();
    assert!(!patterns.is_empty());
    for pattern in patterns {
        assert!(pattern["sequence"].as_array().unwrap().len() <= 7);
        let occurrences = pattern["occurrences"].as_array().unwrap();
        assert_eq!(
            occurrences.len() as u64,
            pattern["support"].as_u64().unwrap()
        );
        assert!(occurrences.iter().all(|occurrence| {
            occurrence["object_id"]
                .as_str()
                .is_some_and(|id| id.starts_with("IL-"))
                && occurrence["start_event"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("INV-E-"))
                && occurrence["end_time_ms"].as_i64().unwrap()
                    >= occurrence["start_time_ms"].as_i64().unwrap()
        }));
    }
}

#[test]
fn inventory_state_detection_assignment_export_is_complete_and_deterministic() {
    let doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    let request = r#"{
        "object_type":"ItemLocation",
        "window_size":3,
        "som_width":3,
        "som_height":3,
        "epochs":25,
        "max_training_windows":10
    }"#;
    let first_text = doc.state_detection_assignments_json(request).unwrap();
    let second_text = doc.state_detection_assignments_json(request).unwrap();
    assert_eq!(first_text, second_text);
    let assignments = json_value(&first_text);
    assert_eq!(
        assignments["windows"].as_array().unwrap().len() as u64,
        assignments["window_count"].as_u64().unwrap()
    );
    assert_eq!(assignments["object_type"], "ItemLocation");
    assert_eq!(assignments["window_size"], 3);
    assert_eq!(assignments["som_width"], 3);
    assert_eq!(assignments["som_height"], 3);
    assert_eq!(assignments["training_window_count"], 10);
    assert_eq!(assignments["window_count"], 36);
    assert!(assignments["feature_count"].as_u64().unwrap() > 0);
    assert!(assignments["som"]["quantization_error"].as_f64().is_some());
    let csv = doc.state_detection_assignments_csv(request).unwrap();
    assert_eq!(
        csv.lines().count() - 1,
        assignments["window_count"].as_u64().unwrap() as usize
    );
}
