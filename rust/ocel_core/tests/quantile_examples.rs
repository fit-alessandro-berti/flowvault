use ocel_core::OcelDocumentCore;
use serde_json::Value;
use std::{fs, path::PathBuf};

#[test]
fn every_ui_quantile_example_executes_on_its_real_log() {
    // The UI and this test use the same queries, so examples cannot silently drift.
    let presets: Value = serde_json::from_str(include_str!(
        "../../../src/app/quantile-state-query-presets.json"
    ))
    .unwrap();
    assert_eq!(presets.as_array().unwrap().len(), 6);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../files/ocel2");
    for preset in presets.as_array().unwrap() {
        let key = preset["logKey"].as_str().unwrap();
        let (paths, low, high) = match key {
            "evaluation_inventory" => (
                vec![root.join("evaluation/inventory_golden/observed.ocel.json")],
                "Understock",
                "Overstock",
            ),
            "evaluation_manufacturing" => (
                vec![root.join("evaluation/manufacturing_golden/observed.ocel.json")],
                "Low Health",
                "High Health",
            ),
            "ocel20_example" => (
                vec![
                    root.join(format!("{key}.json")),
                    root.join(format!("{key}.xml")),
                ],
                "Small Purchase",
                "Large Purchase",
            ),
            "container_logistics" => (
                vec![
                    root.join(format!("{key}.json")),
                    root.join(format!("{key}.xml")),
                ],
                "Small Load",
                "Large Load",
            ),
            "order-management" => (
                vec![
                    root.join(format!("{key}.json")),
                    root.join(format!("{key}.xml")),
                ],
                "Low Value",
                "High Value",
            ),
            "inventory_management_simulated" => (
                vec![
                    root.join(format!("{key}.json")),
                    root.join(format!("{key}.xml")),
                ],
                "Understock",
                "Overstock",
            ),
            _ => panic!("Unverified log family: {key}"),
        };
        for path in paths {
            let mut doc =
                OcelDocumentCore::from_bytes(&fs::read(&path).unwrap(), path.to_str()).unwrap();
            let result: Value = serde_json::from_str(
                &doc.apply_state_query(preset["query"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap();
            assert!(result["assigned_events"].as_u64().unwrap() > 0, "{path:?}");
            let exported: Value = serde_json::from_str(&doc.export_json().unwrap()).unwrap();
            let labels = exported["events"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|event| event["attributes"].as_array().unwrap())
                .filter(|attr| attr["name"] == "state")
                .map(|attr| attr["value"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert!(labels.contains(&low), "{path:?} should demonstrate {low}");
            assert!(labels.contains(&high), "{path:?} should demonstrate {high}");
        }
    }
}
