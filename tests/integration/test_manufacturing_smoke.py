import csv
import json
from collections import Counter
from pathlib import Path

from saocpm_eval.config import load_config
from saocpm_eval.generation import generate_run
from saocpm_eval.validation import validate_run

REPOSITORY_ROOT = Path(__file__).parents[2]


def test_manufacturing_smoke_meets_forced_support_contract(tmp_path: Path) -> None:
    config_path = REPOSITORY_ROOT / "configs" / "manufacturing_smoke.yaml"
    config = load_config(config_path, "manufacturing")
    output = tmp_path / "manufacturing-smoke"
    generate_run(config=config, config_path=config_path, output_dir=output)
    validate_run(output)
    with (output / "truth" / "injected_pattern_instances.csv").open(newline="") as source:
        support = Counter(row["pattern_id"] for row in csv.DictReader(source))
    assert support == {
        "MFG-P1": 8,
        "MFG-P2": 6,
        "MFG-P3": 6,
        "MFG-P4": 6,
        "MFG-P5": 4,
        "MFG-P6": 4,
    }
    answer_key = json.loads((output / "tasks" / "answer_key.json").read_text())
    assert [row["task_id"] for row in answer_key["tasks"]] == [
        "MFG-TASK-1",
        "MFG-TASK-2",
        "MFG-TASK-3",
        "MFG-TASK-4",
    ]
    causal = json.loads((output / "truth" / "causal_truth.json").read_text())
    assert len(causal["paired_potential_outcomes"]) == max(24, config.entities["machines"])
    assert all(value < 0 for value in causal["true_effects"].values())
