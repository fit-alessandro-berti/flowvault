use ocel_core::OcelDocumentCore;
use serde_json::{json, Value};

fn timestamp(index: usize) -> String {
    format!("2025-01-01T00:{:02}:{:02}Z", index / 60, index % 60)
}

fn fixture(values: &[Option<&str>]) -> Value {
    json!({
        "eventTypes": [{"name":"Tick","attributes":[{"name":"stock","type":"string"},{"name":"flag","type":"boolean"}]}],
        "objectTypes": [{"name":"Item","attributes":[{"name":"stock","type":"float"}]},{"name":"Other","attributes":[{"name":"stock","type":"float"}]}],
        "events": values.iter().enumerate().map(|(index, value)| {
            let mut attributes = vec![json!({"name":"flag", "value":true})];
            if let Some(value) = value { attributes.push(json!({"name":"stock", "value":value})); }
            json!({"id":format!("e{index}"),"type":"Tick","time":timestamp(index),"attributes":attributes,
                "relationships":[{"objectId":"item","qualifier":""}]})
        }).collect::<Vec<_>>(),
        "objects":[{"id":"item","type":"Item","attributes":[],"relationships":[]},
            {"id":"other","type":"Other","attributes":[{"name":"stock","time":timestamp(0),"value":1000000.0}],"relationships":[]}]
    })
}

fn document(input: &Value) -> OcelDocumentCore {
    OcelDocumentCore::new(&input.to_string(), Some("json")).unwrap()
}
fn export(doc: &OcelDocumentCore) -> Value {
    serde_json::from_str(&doc.export_json().unwrap()).unwrap()
}
fn attribute(event: &Value, name: &str) -> Option<String> {
    event["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attr| attr["name"] == name)
        .map(|attr| attr["value"].as_str().unwrap().to_owned())
}
fn threshold_query(field: &str, probability: &str) -> String {
    format!("STATE threshold FOR LEADING OBJECT TYPE 'Item' AS CASE WHEN event.type IS NOT NULL THEN QUANTILE({field}, {probability}) END")
}
const TAILS: &str = "STATE state FOR LEADING OBJECT TYPE 'Item' AS CASE WHEN event.stock IS NULL OR QUANTILE(event.stock, 0.5) IS NULL THEN 'Unknown' WHEN event.stock = QUANTILE(event.stock, 0.1) AND event.stock = QUANTILE(event.stock, 0.9) THEN 'Normal' WHEN event.stock <= QUANTILE(event.stock, 0.1) THEN 'Understock' WHEN event.stock >= QUANTILE(event.stock, 0.9) THEN 'Overstock' ELSE 'Normal' END";

#[test]
fn selects_bottom_and_top_ten_percent_with_event_weighting_and_leading_type_scope() {
    let values = (1..=100).map(|value| value.to_string()).collect::<Vec<_>>();
    let mut input = fixture(
        &values
            .iter()
            .map(|value| Some(value.as_str()))
            .collect::<Vec<_>>(),
    );
    input["objects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"second","type":"Item","attributes":[],"relationships":[]}));
    // Neither a second leading object nor duplicated qualifiers weight this event twice.
    input["events"][0]["relationships"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"objectId":"item","qualifier":"duplicate"}),
            json!({"objectId":"second","qualifier":""}),
        ]);
    // An unrelated event's extreme value must not enter this population.
    input["events"].as_array_mut().unwrap().push(json!({"id":"excluded","type":"Tick","time":timestamp(120),"attributes":[{"name":"stock","value":"1000000"}],"relationships":[{"objectId":"other","qualifier":""}]}));
    let mut doc = document(&input);
    doc.apply_state_query(TAILS).unwrap();
    let output = export(&doc);
    for (label, expected) in [("Understock", 10), ("Normal", 80), ("Overstock", 10)] {
        assert_eq!(
            output["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| attribute(event, "state").as_deref() == Some(label))
                .count(),
            expected
        );
    }
    assert!(attribute(&output["events"][100], "state").is_none());
    for (probability, expected) in [
        ("0", 1.0),
        ("0.1", 10.9),
        ("0.5", 50.5),
        ("0.9", 90.1),
        ("1", 100.0),
    ] {
        doc.apply_state_query(&threshold_query("event.stock", probability))
            .unwrap();
        let actual = attribute(&export(&doc)["events"][0], "threshold")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!(
            (actual - expected).abs() < 1e-10,
            "p={probability}: {actual}"
        );
    }
}

#[test]
fn object_quantiles_use_event_time_snapshots_without_duplicate_relationships_or_future_values() {
    let mut input = fixture(&[Some("10"), Some("20"), Some("30")]);
    input["objects"][0]["attributes"] = json!([
        {"name":"stock","time":timestamp(0),"value":10.0},
        {"name":"stock","time":timestamp(1),"value":30.0},
        {"name":"stock","time":timestamp(10),"value":999.0}]);
    input["objects"].as_array_mut().unwrap().push(json!({"id":"second","type":"Item","attributes":[{"name":"stock","time":timestamp(0),"value":50.0}],"relationships":[]}));
    input["events"][0]["relationships"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"objectId":"item","qualifier":"duplicate"}),
            json!({"objectId":"second","qualifier":""}),
            json!({"objectId":"other","qualifier":""}),
        ]);
    let mut doc = document(&input);
    // Eligible samples are [10, 50, 30, 30].
    for (probability, expected) in [("0.25", 25.0), ("0.5", 30.0), ("1", 50.0)] {
        doc.apply_state_query(&threshold_query("object.stock", probability))
            .unwrap();
        assert_eq!(
            attribute(&export(&doc)["events"][0], "threshold")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            expected
        );
    }
    doc.apply_state_query("STATE labels FOR LEADING OBJECT TYPE 'Item' AS CASE WHEN object.stock <= QUANTILE(object.stock, 0.25) THEN 'Low' ELSE CONCAT('At most ', QUANTILE(object.stock, 1)) END").unwrap();
    assert_eq!(
        attribute(&export(&doc)["events"][0], "labels").as_deref(),
        Some("Low")
    );
    assert_eq!(
        attribute(&export(&doc)["events"][2], "labels").as_deref(),
        Some("At most 50")
    );
    // Removing the first history value omits unobserved values instead of looking ahead.
    input["objects"][0]["attributes"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    let mut missing = document(&input);
    missing
        .apply_state_query(&threshold_query("object.stock", "0"))
        .unwrap();
    assert_eq!(
        attribute(&export(&missing)["events"][0], "threshold").as_deref(),
        Some("30")
    );
}

#[test]
fn handles_numeric_strings_missing_invalid_and_boolean_values() {
    let mut doc = document(&fixture(&[
        Some(" 1 "),
        Some("3"),
        Some("NaN"),
        Some("inf"),
        Some("bad"),
        None,
    ]));
    doc.apply_state_query(&threshold_query("event.stock", "0.5"))
        .unwrap();
    assert_eq!(
        attribute(&export(&doc)["events"][0], "threshold").as_deref(),
        Some("2")
    );
    doc.apply_state_query(TAILS).unwrap();
    let output = export(&doc);
    for index in 2..5 {
        assert_eq!(
            attribute(&output["events"][index], "state").as_deref(),
            Some("Normal")
        );
    }
    assert_eq!(
        attribute(&output["events"][5], "state").as_deref(),
        Some("Unknown")
    );
    doc.apply_state_query("STATE empty FOR LEADING OBJECT TYPE 'Item' AS CASE WHEN QUANTILE(event.flag, 0.5) IS NULL THEN 'No numeric values' END").unwrap();
    assert_eq!(
        attribute(&export(&doc)["events"][0], "empty").as_deref(),
        Some("No numeric values")
    );
    let mut empty = document(&fixture(&[None, Some("bad")]));
    empty.apply_state_query(TAILS).unwrap();
    assert!(export(&empty)["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|event| attribute(event, "state").as_deref() == Some("Unknown")));
}

#[test]
fn ties_singletons_and_coincident_tail_boundaries_are_explicit() {
    for values in [vec![Some("5")], vec![Some("5"); 10]] {
        let mut doc = document(&fixture(&values));
        doc.apply_state_query(TAILS).unwrap();
        assert!(export(&doc)["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|event| attribute(event, "state").as_deref() == Some("Normal")));
    }
    let mut values = vec![Some("5"); 98];
    values.extend([Some("0"), Some("10")]);
    let mut doc = document(&fixture(&values));
    doc.apply_state_query(TAILS).unwrap();
    assert_eq!(
        attribute(&export(&doc)["events"][98], "state").as_deref(),
        Some("Understock")
    );
    assert_eq!(
        attribute(&export(&doc)["events"][99], "state").as_deref(),
        Some("Overstock")
    );
    // Returning a numeric threshold must not saturate it to a 64-bit integer.
    let mut large = document(&fixture(&[Some("1e20"), Some("3e20")]));
    large
        .apply_state_query(&threshold_query("event.stock", "0.5"))
        .unwrap();
    assert_eq!(
        attribute(&export(&large)["events"][0], "threshold")
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        2e20
    );
    // Equal subnormal values must remain equal rather than underflow during interpolation.
    let mut tiny = document(&fixture(&[Some("5e-324"), Some("5e-324")]));
    tiny.apply_state_query(&threshold_query("event.stock", "0.5"))
        .unwrap();
    assert_eq!(
        attribute(&export(&tiny)["events"][0], "threshold")
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        f64::from_bits(1)
    );
    let mut extremes = document(&fixture(&[Some("-1e308"), Some("1e308")]));
    extremes
        .apply_state_query(&threshold_query("event.stock", "0.5"))
        .unwrap();
    assert_eq!(
        attribute(&export(&extremes)["events"][0], "threshold").as_deref(),
        Some("0")
    );
}

#[test]
fn rejects_invalid_quantiles_atomically() {
    let mut doc = document(&fixture(&[Some("1"), Some("2")]));
    doc.apply_state_query(TAILS).unwrap();
    let before = doc.export_json().unwrap();
    let config = doc.state_configuration_json().unwrap();
    for expression in [
        "QUANTILE(event.stock, -0.1)",
        "QUANTILE(event.stock, 1.1)",
        "QUANTILE(event.stock, '0.9')",
        "QUANTILE(event.stock, event.stock)",
        "QUANTILE(event.stock)",
        "QUANTILE(event.stock, 0.1, 0.9)",
        "QUANTILE('stock', 0.9)",
        "QUANTILE(COALESCE(event.stock, 0), 0.9)",
        "QUANTILE(event.typo, 0.5)",
        "QUANTILE(object.typo, 0.5)",
    ] {
        let query = format!("STATE state FOR LEADING OBJECT TYPE 'Item' AS CASE WHEN event.type IS NOT NULL THEN {expression} END");
        let error = doc.apply_state_query(&query).unwrap_err().to_string();
        assert!(error.contains("QUANTILE"), "{expression}: {error}");
        assert_eq!(doc.export_json().unwrap(), before);
        assert_eq!(doc.state_configuration_json().unwrap(), config);
    }
}

#[test]
fn quantiles_are_stable_under_filters_and_available_in_compositions() {
    let mut doc = document(&fixture(&[Some("1"), Some("2"), Some("3"), Some("4")]));
    doc.apply_filter(&json!({"event_types":["Tick"],"object_types":["Item"],"time_range":{"start_ms":1735689602000i64}}).to_string()).unwrap();
    doc.apply_state_query(&threshold_query("event.stock", "0.5"))
        .unwrap();
    assert_eq!(export(&doc)["events"].as_array().unwrap().len(), 2);
    assert_eq!(
        attribute(&export(&doc)["events"][0], "threshold").as_deref(),
        Some("2.5")
    );
    doc.apply_state_composition(&json!({"leading_object_type":"Item","state_expression":"CASE WHEN event.stock >= QUANTILE(event.stock, 0.9) THEN 'High' ELSE 'Other' END","activity_expression":"event.type || ':' || event.combined_state"}).to_string()).unwrap();
    assert_eq!(
        attribute(&export(&doc)["events"][1], "state_activity").as_deref(),
        Some("Tick:High")
    );
    doc.apply_filter(r#"{"event_types":["Tick"],"object_types":["Item","Other"]}"#)
        .unwrap();
    doc.apply_state_composition(&json!({"leading_object_type":"Item","state_expression":"'5'","activity_expression":"CONCAT(QUANTILE(event.combined_state, 0.5), ':', event.combined_state)"}).to_string()).unwrap();
    assert_eq!(
        attribute(&export(&doc)["events"][0], "state_activity").as_deref(),
        Some("5:5")
    );
}
