import csv
import json
from collections import Counter
from pathlib import Path

from saocpm_eval.config import load_config
from saocpm_eval.generation import generate_run
from saocpm_eval.validation import validate_run

REPOSITORY_ROOT = Path(__file__).parents[2]


def test_inventory_smoke_meets_forced_support_contract(tmp_path: Path) -> None:
    config_path = REPOSITORY_ROOT / "configs" / "inventory_smoke.yaml"
    config = load_config(config_path, "inventory")
    output = tmp_path / "inventory-smoke"
    generate_run(config=config, config_path=config_path, output_dir=output)
    validate_run(output)
    with (output / "truth" / "injected_pattern_instances.csv").open(newline="") as source:
        support = Counter(row["pattern_id"] for row in csv.DictReader(source))
    assert support == {
        "INV-P1": 10,
        "INV-P2": 10,
        "INV-P3": 8,
        "INV-P4": 8,
        "INV-P5": 6,
        "INV-P6": 6,
    }
    answer_key = json.loads((output / "tasks" / "answer_key.json").read_text())
    assert [row["task_id"] for row in answer_key["tasks"]] == [
        "INV-TASK-1",
        "INV-TASK-2",
        "INV-TASK-3",
        "INV-TASK-4",
    ]
    causal = json.loads((output / "truth" / "causal_truth.json").read_text())
    assert len(causal["paired_potential_outcomes"]) == config.entities["item_locations"]
    assert all(value < 0 for value in causal["true_effects"].values())
