mod common;

use common::json_value;
use ocel_core::OcelDocumentCore;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const OBSERVED: &str =
    include_str!("../../../files/ocel2/evaluation/manufacturing_golden/observed.ocel.json");
const BEHAVIOR: &str = include_str!(
    "../../../files/ocel2/evaluation/manufacturing_golden/observed.behavior.ocel.json"
);
const QUERY: &str = include_str!("../../../queries/manufacturing_state.sql");
const STATE_TRUTH: &str =
    include_str!("../../../files/ocel2/evaluation/manufacturing_golden/truth/state_at_event.csv");
const TRANSITION_TRUTH: &str =
    include_str!("../../../files/ocel2/evaluation/manufacturing_golden/truth/transitions.csv");
const MANIFEST: &str =
    include_str!("../../../files/ocel2/evaluation/manufacturing_golden/manifest.json");

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

fn event_states(exported: &Value) -> BTreeMap<String, String> {
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

#[test]
fn manufacturing_golden_import_and_state_query_match_independent_oracle() {
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
    assert_eq!(result["leading_object_type"], "Machine");
    assert_eq!(result["assigned_events"], truth.len());
    assert_eq!(
        event_states(&json_value(&doc.export_json().unwrap())),
        truth
    );
}

#[test]
fn manufacturing_missing_telemetry_is_unknown_not_a_default_operating_state() {
    let mut doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    doc.apply_state_query(QUERY).unwrap();
    let states = event_states(&json_value(&doc.export_json().unwrap()));
    assert_eq!(
        states.get("MFG-E-000059").map(String::as_str),
        Some("Unknown")
    );
    assert_eq!(
        states.get("MFG-E-000060").map(String::as_str),
        Some("Running")
    );
}

#[derive(Default)]
struct ExpectedTransition {
    durations: Vec<i64>,
    objects: BTreeSet<String>,
}

#[test]
fn manufacturing_golden_transition_kpis_match_independent_oracle() {
    let mut doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    doc.apply_state_query(QUERY).unwrap();
    let actual = json_value(
        &doc.state_transition_kpis_json(r#"{"object_type":"Machine","stuck_limit":20}"#)
            .unwrap(),
    );
    let mut expected = BTreeMap::<(String, String), ExpectedTransition>::new();
    for row in csv_rows(TRANSITION_TRUTH) {
        let key = (row["from_state"].clone(), row["to_state"].clone());
        let accumulator = expected.entry(key).or_default();
        accumulator
            .durations
            .push((row["duration_minutes"].parse::<f64>().unwrap() * 60_000.0).round() as i64);
        accumulator.objects.insert(row["leading_object_id"].clone());
    }
    let actual_rows = actual["transitions"].as_array().unwrap();
    assert_eq!(actual_rows.len(), expected.len());
    for row in actual_rows {
        let key = (
            row["from_state"].as_str().unwrap().to_owned(),
            row["to_state"].as_str().unwrap().to_owned(),
        );
        let oracle = expected.get_mut(&key).expect("unexpected transition");
        oracle.durations.sort();
        let total = oracle.durations.iter().sum::<i64>();
        assert_eq!(row["count"], oracle.durations.len());
        assert_eq!(row["object_count"], oracle.objects.len());
        assert_eq!(
            row["min_duration_ms"].as_i64(),
            oracle.durations.first().copied()
        );
        assert_eq!(
            row["median_duration_ms"].as_i64(),
            Some(oracle.durations[oracle.durations.len() / 2])
        );
        assert_eq!(
            row["max_duration_ms"].as_i64(),
            oracle.durations.last().copied()
        );
        let average = total as f64 / oracle.durations.len() as f64;
        assert!((row["avg_duration_ms"].as_f64().unwrap() - average).abs() < 1e-6);
    }
}

#[test]
fn manufacturing_golden_graph_and_patterns_expose_fault_and_recovery_mechanisms() {
    let mut doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    doc.apply_state_query(QUERY).unwrap();
    let graph = json_value(&doc.state_aware_ocdfg_json().unwrap());
    let labels = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["label"].as_str())
        .collect::<BTreeSet<_>>();
    for expected in [
        "CHANGE Running -> Degraded",
        "CHANGE Degraded -> Down",
        "CHANGE Down -> Recovery",
        "CHANGE Recovery -> Running",
        "CHANGE Degraded -> Quality Hold",
        "CHANGE Running -> Unknown",
    ] {
        assert!(labels.contains(expected), "missing graph node {expected}");
    }
    let mut behavior = OcelDocumentCore::new(BEHAVIOR, Some("json")).unwrap();
    behavior.apply_state_query(QUERY).unwrap();
    let patterns = json_value(&behavior.state_patterns_json().unwrap());
    let summaries = patterns["intra"]
        .as_array()
        .unwrap()
        .iter()
        .chain(patterns["inter"].as_array().unwrap())
        .collect::<Vec<_>>();
    for activity in [
        "Critical Alarm Raised",
        "Quality Hold Started",
        "Component Replaced",
        "Part Unavailable",
        "Machine Restarted",
        "Test Failed",
    ] {
        assert!(
            summaries.iter().any(|pattern| {
                pattern["sequence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item.as_str().is_some_and(|label| label.contains(activity)))
            }),
            "missing canonical manufacturing activity {activity}"
        );
    }
}

#[test]
fn manufacturing_explicit_recovery_targets_do_not_depend_on_state_names() {
    let mut doc = OcelDocumentCore::new(OBSERVED, Some("json")).unwrap();
    doc.apply_state_query(QUERY).unwrap();
    let result = json_value(
        &doc.state_transition_kpis_json(
            r#"{
                "object_type":"Machine",
                "recovery_transitions":[
                    ["Down","Recovery"],
                    ["Recovery","Running"]
                ]
            }"#,
        )
        .unwrap(),
    );
    let recovery = result["recovery"].as_array().unwrap();
    assert_eq!(recovery.len(), 2);
    let pairs = recovery
        .iter()
        .map(|row| {
            (
                row["from_state"].as_str().unwrap(),
                row["to_state"].as_str().unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        pairs,
        BTreeSet::from([("Down", "Recovery"), ("Recovery", "Running")])
    );
}
