use ocel_core::OcelDocumentCore;
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/multiple_states.json");
const FIRST: &str = "STATE state1 FOR LEADING OBJECT TYPE 'Order' AS CASE WHEN event.type IS NOT NULL THEN object.status END";
const SECOND: &str = "STATE state2 FOR LEADING OBJECT TYPE 'Shipment' AS CASE WHEN object.status IS NOT NULL THEN object.status END";

fn document() -> OcelDocumentCore {
    let mut doc = OcelDocumentCore::new(FIXTURE, Some("json")).unwrap();
    doc.apply_state_query(FIRST).unwrap();
    doc.apply_state_query(SECOND).unwrap();
    doc
}
fn exported(doc: &OcelDocumentCore) -> Value {
    serde_json::from_str(&doc.export_json().unwrap()).unwrap()
}
fn attribute(log: &Value, event: usize, name: &str) -> Value {
    log["events"][event]["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attr| attr["name"] == name)
        .map(|attr| attr["value"].clone())
        .unwrap_or(Value::Null)
}
fn composition(state: &str, activity: &str) -> String {
    json!({ "leading_object_type": "Order", "state_expression": state, "activity_expression": activity }).to_string()
}
fn compose(doc: &mut OcelDocumentCore, state: &str, activity: &str) {
    doc.apply_state_composition(&composition(state, activity))
        .unwrap();
}

#[test]
fn independent_object_states_combine_and_recombine_without_losing_values() {
    let mut doc = document();
    let original = exported(&doc);
    assert_eq!(attribute(&original, 0, "state1"), "Open");
    assert_eq!(attribute(&original, 2, "state1"), "Closed");
    assert_eq!(attribute(&original, 0, "state2"), "Transit");
    assert_eq!(attribute(&original, 1, "state2"), Value::Null);
    assert_eq!(attribute(&original, 2, "state2"), "Delayed");
    // Adding a second definition does not change the selected analysis state.
    assert_eq!(attribute(&original, 2, "combined_state"), "Closed");
    compose(
        &mut doc,
        "CONCAT(event.state1, ' / ', event.state2)",
        "event.combined_state",
    );
    let combined = exported(&doc);
    assert_eq!(attribute(&combined, 0, "state_activity"), "Open / Transit");
    assert_eq!(attribute(&combined, 1, "combined_state"), Value::Null);
    assert_eq!(
        attribute(&combined, 2, "combined_state"),
        "Closed / Delayed"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&doc.summary_json()).unwrap()["stateful_events"],
        2
    );
    let graph: Value = serde_json::from_str(&doc.state_aware_ocdfg_json().unwrap()).unwrap();
    assert!(graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["label"] == "Open / Transit"));
    let patterns: Value = serde_json::from_str(&doc.state_patterns_json().unwrap()).unwrap();
    assert!(patterns["intra"]
        .as_array()
        .unwrap()
        .iter()
        .any(|pattern| pattern["sequence"]
            .as_array()
            .unwrap()
            .contains(&json!("Open / Transit"))));
    compose(
        &mut doc,
        "COALESCE(event.state2, 'Unknown') || ':' || event.state1",
        "CONCAT(event.type, ' [', event.combined_state, ']')",
    );
    let reordered = exported(&doc);
    assert_eq!(
        attribute(&reordered, 1, "state_activity"),
        "Act [Unknown:Open]"
    );
    for event in 0..3 {
        for name in ["state1", "state2"] {
            assert_eq!(
                attribute(&reordered, event, name),
                attribute(&original, event, name)
            );
        }
    }
    doc.apply_state_query(
        &SECOND.replace("THEN object.status", "THEN CONCAT('Ship-', object.status)"),
    )
    .unwrap();
    assert_eq!(
        attribute(&exported(&doc), 2, "state_activity"),
        "Finish [Ship-Delayed:Closed]"
    );
    let config: Value = serde_json::from_str(&doc.state_configuration_json().unwrap()).unwrap();
    assert_eq!(config["definitions"].as_array().unwrap().len(), 2);
}

#[test]
fn conditional_labels_null_fallbacks_and_literal_values_are_supported() {
    let mut doc = document();
    compose(&mut doc, "COALESCE(event.state2, event.state1)", "CASE WHEN event.state2 IS NULL THEN CONCAT('Order ', event.state1) WHEN event.state1 = 'Closed' THEN 'Complete' ELSE event.type || ': ' || event.combined_state END");
    let log = exported(&doc);
    assert_eq!(attribute(&log, 0, "state_activity"), "Act: Transit");
    assert_eq!(attribute(&log, 1, "state_activity"), "Order Open");
    assert_eq!(attribute(&log, 2, "state_activity"), "Complete");
    doc.apply_state_query("STATE state3 FOR LEADING OBJECT TYPE 'Order' AS CASE WHEN object.status = 'Open' THEN CONCAT(true, ':', 12, ':', COALESCE(NULL, 'ok')) ELSE CASE WHEN event.type = 'Finish' THEN 'Done' END END").unwrap();
    assert_eq!(attribute(&exported(&doc), 0, "state3"), "true:12:ok");
    assert_eq!(attribute(&exported(&doc), 2, "state3"), "Done");
}

#[test]
fn failed_edits_are_atomic_and_report_invalid_references() {
    let mut doc = document();
    let before = doc.export_json().unwrap();
    let config = doc.state_configuration_json().unwrap();
    for (state, activity) in [
        ("event.typo", "event.type"),
        ("event.combined_state", "event.type"),
        ("object.status", "event.type"),
        ("event.state1", "event.state_activity"),
        ("event.state1", "event.state2"),
        ("event.state1", "''"),
        ("CONCAT(event.state1,)", "event.type"),
        ("BOGUS(event.state1)", "event.type"),
        ("event.state1 junk", "event.type"),
    ] {
        assert!(
            doc.apply_state_composition(&composition(state, activity))
                .is_err(),
            "{state} / {activity}"
        );
        assert_eq!(doc.export_json().unwrap(), before);
        assert_eq!(doc.state_configuration_json().unwrap(), config);
    }
    for query in [
        FIRST.replace("state1", "combined_state"),
        FIRST.replace("state1", "STATE1"),
        FIRST.replace("object.status END", "event.combined_state END"),
    ] {
        assert!(doc.apply_state_query(&query).is_err());
        assert_eq!(doc.export_json().unwrap(), before);
    }
    compose(&mut doc, "event.state1", "event.state1");
    let before = doc.export_json().unwrap();
    // Empty activity labels from a definition edit must roll back both the state and composition.
    assert!(doc
        .apply_state_query(&FIRST.replace("THEN object.status", "THEN ''"))
        .is_err());
    assert_eq!(doc.export_json().unwrap(), before);
}

#[test]
fn composition_survives_filters_and_every_export_format() {
    let mut doc = document();
    compose(
        &mut doc,
        "CONCAT(event.state1, '/', COALESCE(event.state2, '?'))",
        "event.combined_state",
    );
    let expected = exported(&doc);
    for (bytes, format) in [
        (doc.export_json().unwrap().into_bytes(), "json"),
        (doc.export_xml().unwrap().into_bytes(), "xml"),
        (doc.export_csv().unwrap().into_bytes(), "csv"),
        (doc.export_sqlite().unwrap(), "sqlite"),
        (doc.export_bundle().unwrap(), "bundle"),
    ] {
        let imported = OcelDocumentCore::from_bytes(&bytes, Some(format)).unwrap();
        let log = exported(&imported);
        for event in 0..3 {
            for name in ["state1", "state2", "combined_state", "state_activity"] {
                assert_eq!(
                    attribute(&log, event, name),
                    attribute(&expected, event, name),
                    "{format} {name}"
                );
            }
        }
        let graph = imported.state_aware_ocdfg_json().unwrap();
        assert!(graph.contains("Open/Transit"));
    }
    doc.apply_filter(r#"{"event_types":["Act"],"object_types":["Order","Shipment"]}"#)
        .unwrap();
    assert_eq!(exported(&doc)["events"].as_array().unwrap().len(), 2);
    // A filter with no matching objects must still allow recomposition.
    doc.apply_filter(r#"{"event_types":["Act","Finish"],"object_types":["Order","Shipment"],"text_attributes":[{"scope":"event","name":"state1","values":["Absent"]}]}"#).unwrap();
    compose(&mut doc, "event.state1", "event.state1 || '!'");
    assert_eq!(exported(&doc)["events"].as_array().unwrap().len(), 0);
    assert_eq!(
        serde_json::from_str::<Value>(&doc.summary_json()).unwrap()["stateful_events"],
        0
    );
    doc.apply_filter(r#"{"event_types":["Act","Finish"],"object_types":["Order","Shipment"]}"#)
        .unwrap();
    assert_eq!(attribute(&exported(&doc), 2, "state_activity"), "Closed!");
}

#[test]
fn legacy_state_is_preserved_as_an_independent_composition_input() {
    let mut doc = OcelDocumentCore::new(FIXTURE, Some("json")).unwrap();
    doc.apply_state_query(&FIRST.replace("state1", "state"))
        .unwrap();
    doc.apply_state_query(SECOND).unwrap();
    compose(
        &mut doc,
        "event.state || COALESCE(event.state2, '')",
        "event.combined_state",
    );
    assert_eq!(attribute(&exported(&doc), 0, "state"), "Open");
    assert_eq!(
        attribute(&exported(&doc), 0, "state_activity"),
        "OpenTransit"
    );
}

#[test]
fn all_state_analyses_use_the_selected_combination() {
    let mut doc = document();
    compose(
        &mut doc,
        "event.state1 || '/' || COALESCE(event.state2, '?')",
        "event.combined_state",
    );
    let kpis: Value = serde_json::from_str(&doc.state_transition_kpis_json("{}").unwrap()).unwrap();
    assert!(kpis["transitions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["from_state"] == "Open/Transit" && row["to_state"] == "Open/?"));
    let time: Value = serde_json::from_str(&doc.time_perspective_json("{}").unwrap()).unwrap();
    assert!(time["states"]
        .as_array()
        .unwrap()
        .contains(&json!("Closed/Delayed")));
    let lifecycle: Value =
        serde_json::from_str(&doc.object_lifecycle_detail_json("o1").unwrap()).unwrap();
    assert_eq!(lifecycle["events"][2]["state"], "Closed/Delayed");
    let correlations = doc.state_correlations_json().unwrap();
    assert!(correlations.contains("Closed/Delayed"));
    // Changing only the analysis leading type does not erase states from other objects.
    let request = json!({"leading_object_type":"Shipment", "state_expression":"event.state1 || '/' || COALESCE(event.state2, '?')", "activity_expression":"event.combined_state"}).to_string();
    doc.apply_state_composition(&request).unwrap();
    let patterns: Value = serde_json::from_str(&doc.state_patterns_json().unwrap()).unwrap();
    assert!(patterns["intra"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["leading_object_type"] == "Shipment"));
    assert_eq!(attribute(&exported(&doc), 1, "state1"), "Open");
}

#[test]
fn replacing_existing_attributes_uses_a_snapshot_and_updates_export_schemas() {
    let mut input: Value = serde_json::from_str(FIXTURE).unwrap();
    for kind in input["eventTypes"].as_array_mut().unwrap() {
        kind["attributes"] =
            json!([{"name":"score", "type":"integer"}, {"name":"other", "type":"string"}]);
    }
    for event in input["events"].as_array_mut().unwrap() {
        event["attributes"] =
            json!([{"name":"score", "value":1}, {"name":"other", "value":"Keep"}]);
    }
    let mut doc = OcelDocumentCore::new(&input.to_string(), Some("json")).unwrap();
    doc.apply_state_query("STATE score FOR LEADING OBJECT TYPE 'Order' AS CASE WHEN event.score = 1 THEN CONCAT(event.other, ':', event.score) END").unwrap();
    assert_eq!(attribute(&exported(&doc), 0, "score"), "Keep:1");
    let imported = OcelDocumentCore::new(&doc.export_json().unwrap(), Some("json")).unwrap();
    assert_eq!(attribute(&exported(&imported), 0, "score"), "Keep:1");
}
