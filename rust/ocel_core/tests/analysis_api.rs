mod common;

use common::{json_value, JSON_EXAMPLE};
use ocel_core::OcelDocumentCore;

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
