from __future__ import annotations

import csv
import json
from collections import Counter
from copy import deepcopy
from pathlib import Path

import pytest
import yaml

from saocpm_eval.analytics.conformance_rules import detect_conformance
from saocpm_eval.common.hashing import sha256_file
from saocpm_eval.config import load_config
from saocpm_eval.generation import generate_run
from saocpm_eval.inventory.config import load_inventory_config
from saocpm_eval.inventory.simulation import InventoryGoldenSimulation
from saocpm_eval.inventory.validation import stock_conservation_errors
from saocpm_eval.validation import validate_run

REPOSITORY_ROOT = Path(__file__).parents[2]
CONFIG_PATH = REPOSITORY_ROOT / "configs" / "inventory_golden.yaml"


def generate(output: Path, config_path: Path = CONFIG_PATH) -> None:
    config = load_config(config_path, "inventory")
    generate_run(config=config, config_path=config_path, output_dir=output)


def test_inventory_golden_covers_states_patterns_and_rules(tmp_path: Path) -> None:
    output = tmp_path / "run"
    generate(output)
    validate_run(output)
    coverage = json.loads((output / "expected" / "branch_coverage.json").read_text())
    assert coverage["state_branches_covered"] is True
    assert coverage["pattern_ids"] == [f"INV-P{index}" for index in range(1, 7)]
    assert coverage["conformance_rule_ids"] == [f"INV-C{index}" for index in range(1, 7)]
    summary = json.loads((output / "expected" / "summary.json").read_text())
    assert summary["counts"] == {
        "e2o": 212,
        "event_types": 24,
        "events": 52,
        "leading_objects": 8,
        "o2o": 33,
        "object_types": 10,
        "objects": 38,
    }
    assert (
        sha256_file(output / "observed.ocel.json")
        == "91f1fa299e7e40211f0226d4f005ccead6cfe5c260bdb3d09d81b80b014ae255"
    )
    with (output / "truth" / "state_at_event.csv").open(newline="") as source:
        state_counts = Counter(row["reference_state"] for row in csv.DictReader(source))
    assert state_counts == {
        "Normal": 31,
        "Understock": 9,
        "Overstock": 7,
        "Critical Understock": 4,
        "Unknown": 1,
    }
    assertions = json.loads((output / "expected" / "golden_assertions.json").read_text())
    assert assertions["state_sequences"] == {
        "IL-0001": [
            "Normal",
            "Understock",
            "Critical Understock",
            "Understock",
            "Overstock",
            "Normal",
        ],
        "IL-0002": ["Understock", "Normal"],
        "IL-0003": ["Normal", "Overstock", "Normal"],
        "IL-0004": ["Understock", "Normal"],
        "IL-0005": ["Normal", "Understock", "Critical Understock", "Normal"],
        "IL-0006": ["Overstock", "Normal"],
        "IL-0007": ["Normal"],
        "IL-0008": ["Normal", "Unknown", "Normal"],
    }
    assert assertions["pattern_support"] == {f"INV-P{index}": 1 for index in range(1, 7)}
    assert assertions["prediction_positive_count"] == 21
    with (output / "truth" / "state_episodes.csv").open(newline="") as source:
        episodes = [
            (
                row["leading_object_id"],
                row["label"],
                float(row["duration_minutes"]),
            )
            for row in csv.DictReader(source)
        ]
    assert episodes[:6] == [
        ("IL-0001", "Normal", 1440.5),
        ("IL-0001", "Understock", 1 / 6),
        ("IL-0001", "Critical Understock", 2880 + 1 / 3),
        ("IL-0001", "Understock", 2879.0),
        ("IL-0001", "Overstock", 2880.0),
        ("IL-0001", "Normal", 40320.0),
    ]
    with (output / "truth" / "prediction_samples.csv").open(newline="") as source:
        prediction_counts = Counter(
            (row["label_name"], row["label"]) for row in csv.DictReader(source)
        )
    assert prediction_counts == {
        ("Understock within 7 days", "true"): 10,
        ("Understock within 7 days", "false"): 30,
        ("Critical Understock within 7 days", "true"): 6,
        ("Critical Understock within 7 days", "false"): 34,
        ("Recovery to Normal within 3 days", "true"): 1,
        ("Recovery to Normal within 3 days", "false"): 3,
        ("Time to stable Normal recovery", "true"): 4,
    }
    document = json.loads((output / "observed.ocel.json").read_text())
    assert Counter(row.rule_id for row in detect_conformance(document, "inventory")) == {
        "INV-C1": 1,
        "INV-C2": 2,
        "INV-C3": 1,
        "INV-C4": 1,
        "INV-C5": 1,
        "INV-C6": 1,
    }


def test_inventory_golden_is_byte_reproducible(tmp_path: Path) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    generate(first)
    generate(second)
    first_files = sorted(path.relative_to(first) for path in first.rglob("*") if path.is_file())
    second_files = sorted(path.relative_to(second) for path in second.rglob("*") if path.is_file())
    assert first_files == second_files
    for relative in first_files:
        assert (first / relative).read_bytes() == (second / relative).read_bytes()


def test_different_seed_changes_content_but_preserves_coverage(tmp_path: Path) -> None:
    original = yaml.safe_load(CONFIG_PATH.read_text(encoding="utf-8"))
    changed = deepcopy(original)
    changed["seed"] += 1
    changed_path = tmp_path / "changed.yaml"
    changed_path.write_text(yaml.safe_dump(changed, sort_keys=False), encoding="utf-8")
    first = tmp_path / "first"
    second = tmp_path / "second"
    generate(first)
    generate(second, changed_path)
    first_assertions = json.loads((first / "expected" / "golden_assertions.json").read_text())
    second_assertions = json.loads((second / "expected" / "golden_assertions.json").read_text())
    assert first_assertions["observed_sha256"] != second_assertions["observed_sha256"]
    first_coverage = json.loads((first / "expected" / "branch_coverage.json").read_text())
    second_coverage = json.loads((second / "expected" / "branch_coverage.json").read_text())
    assert first_coverage["states"] == second_coverage["states"]
    assert first_coverage["pattern_ids"] == second_coverage["pattern_ids"]


def test_conservation_validator_detects_unexplained_stock_change() -> None:
    simulation = InventoryGoldenSimulation(load_inventory_config(CONFIG_PATH))
    simulation.simulate()
    document = simulation.builder.to_dict()
    assert stock_conservation_errors(document) == []
    broken = deepcopy(document)
    event = next(item for item in broken["events"] if item["type"] == "Goods Issue")
    attribute = next(item for item in event["attributes"] if item["name"] == "on_hand_after")
    attribute["value"] += 1.0
    errors = stock_conservation_errors(broken)
    assert len(errors) == 1
    assert errors[0].event_id == event["id"]


def test_conservation_validator_skips_explicitly_incomplete_observations() -> None:
    simulation = InventoryGoldenSimulation(load_inventory_config(CONFIG_PATH))
    simulation.simulate()
    document = simulation.builder.to_dict()
    event = next(item for item in document["events"] if item["type"] == "Goods Issue")
    attributes = {item["name"]: item for item in event["attributes"]}
    attributes["data_complete"]["value"] = False
    event["attributes"] = [item for item in event["attributes"] if item["name"] != "quantity"]
    assert stock_conservation_errors(document) == []


def test_inventory_configuration_rejects_too_few_golden_objects(tmp_path: Path) -> None:
    raw = yaml.safe_load(CONFIG_PATH.read_text(encoding="utf-8"))
    raw["entities"]["item_locations"] = 7
    path = tmp_path / "invalid.yaml"
    path.write_text(yaml.safe_dump(raw), encoding="utf-8")
    with pytest.raises(ValueError, match="at least eight"):
        InventoryGoldenSimulation(load_inventory_config(path))
