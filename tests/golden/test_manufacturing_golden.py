import csv
import json
from collections import Counter
from copy import deepcopy
from pathlib import Path

import yaml

from saocpm_eval.analytics.conformance_rules import detect_conformance
from saocpm_eval.common.hashing import sha256_file
from saocpm_eval.config import load_config
from saocpm_eval.generation import generate_run
from saocpm_eval.validation import validate_run

REPOSITORY_ROOT = Path(__file__).parents[2]
CONFIG_PATH = REPOSITORY_ROOT / "configs" / "manufacturing_golden.yaml"


def generate(output: Path, config_path: Path = CONFIG_PATH) -> None:
    config = load_config(config_path, "manufacturing")
    generate_run(config=config, config_path=config_path, output_dir=output)


def test_manufacturing_golden_covers_states_patterns_and_rules(tmp_path: Path) -> None:
    output = tmp_path / "run"
    generate(output)
    validate_run(output)
    coverage = json.loads((output / "expected" / "branch_coverage.json").read_text())
    assert coverage["state_branches_covered"] is True
    assert set(coverage["pattern_ids"]) == {f"MFG-P{index}" for index in range(1, 7)}
    assert coverage["conformance_rule_ids"] == [f"MFG-C{index}" for index in range(1, 7)]
    summary = json.loads((output / "expected" / "summary.json").read_text())
    assert summary["counts"] == {
        "e2o": 202,
        "event_types": 30,
        "events": 68,
        "leading_objects": 8,
        "o2o": 17,
        "object_types": 11,
        "objects": 39,
    }
    assert (
        sha256_file(output / "observed.ocel.json")
        == "4c2ea160b89881ef3202720d10dc080e3ae4f944c021c5a0dd2450aeff1e99a9"
    )
    with (output / "truth" / "state_at_event.csv").open(newline="") as source:
        state_counts = Counter(row["reference_state"] for row in csv.DictReader(source))
    assert state_counts == {
        "Running": 22,
        "Down": 20,
        "Degraded": 11,
        "Recovery": 8,
        "Quality Hold": 3,
        "Setup": 2,
        "Idle": 1,
        "Unknown": 1,
    }
    assertions = json.loads((output / "expected" / "golden_assertions.json").read_text())
    assert assertions["state_sequences"] == {
        "M-001": ["Running", "Degraded", "Down", "Recovery", "Running"],
        "M-002": ["Running", "Down", "Recovery", "Running"],
        "M-003": ["Running", "Down", "Recovery", "Running", "Degraded"],
        "M-004": ["Running", "Degraded", "Down", "Recovery"],
        "M-005": ["Running", "Recovery"],
        "M-006": ["Running", "Degraded", "Quality Hold", "Degraded"],
        "M-007": ["Setup", "Running"],
        "M-008": ["Idle", "Setup", "Running", "Unknown", "Running"],
    }
    assert assertions["pattern_support"] == {f"MFG-P{index}": 1 for index in range(1, 7)}
    assert assertions["prediction_positive_count"] == 26
    with (output / "truth" / "state_episodes.csv").open(newline="") as source:
        episodes = [
            (
                row["leading_object_id"],
                row["label"],
                float(row["duration_minutes"]),
            )
            for row in csv.DictReader(source)
        ]
    assert episodes[:5] == [
        ("M-001", "Running", 1440 + 1 / 6),
        ("M-001", "Degraded", 2 / 3),
        ("M-001", "Down", 1440 + 1 / 6),
        ("M-001", "Recovery", 119.0),
        ("M-001", "Running", 40200.0),
    ]
    with (output / "truth" / "prediction_samples.csv").open(newline="") as source:
        prediction_counts = Counter(
            (row["label_name"], row["label"]) for row in csv.DictReader(source)
        )
    assert prediction_counts == {
        ("Down within 4 hours", "true"): 5,
        ("Down within 4 hours", "false"): 43,
        ("Down within 24 hours", "true"): 5,
        ("Down within 24 hours", "false"): 43,
        ("Time to Down", "true"): 9,
        ("Time to Down", "false"): 39,
        ("Stable Running recovery within 8 hours", "true"): 3,
        ("Stable Running recovery within 8 hours", "false"): 2,
        ("Time to stable recovery", "true"): 3,
        ("Time to stable recovery", "false"): 2,
        ("Recurrent Degraded or Down within 24 hours", "true"): 1,
        ("Recurrent Degraded or Down within 24 hours", "false"): 4,
    }
    document = json.loads((output / "observed.ocel.json").read_text())
    assert Counter(row.rule_id for row in detect_conformance(document, "manufacturing")) == {
        "MFG-C1": 1,
        "MFG-C2": 4,
        "MFG-C3": 1,
        "MFG-C4": 1,
        "MFG-C5": 1,
        "MFG-C6": 1,
    }


def test_manufacturing_golden_is_byte_reproducible(tmp_path: Path) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    generate(first)
    generate(second)
    first_files = sorted(path.relative_to(first) for path in first.rglob("*") if path.is_file())
    for relative in first_files:
        assert (first / relative).read_bytes() == (second / relative).read_bytes()


def test_manufacturing_different_seed_changes_content_but_preserves_coverage(
    tmp_path: Path,
) -> None:
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
