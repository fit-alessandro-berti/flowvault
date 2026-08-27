"""Truth-grounded analyst cases with baseline/treatment views and answer keys."""

from __future__ import annotations

import csv
import json
from pathlib import Path
from typing import Any

from saocpm_eval.common.truth_writer import RunWriter


def _csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source))


def _rubric() -> list[dict[str, Any]]:
    return [
        {"criterion": "correct conclusion", "points": 40},
        {"criterion": "required evidence identified", "points": 35},
        {"criterion": "timeline and object context are consistent", "points": 15},
        {"criterion": "alternatives are considered", "points": 10},
    ]


def _case(
    *,
    identifier: str,
    title: str,
    prompt: str,
    cohort: list[str],
    start: str,
    end: str,
    answer: Any,
    evidence: list[str],
    alternatives: list[str],
) -> dict[str, Any]:
    return {
        "task_id": identifier,
        "title": title,
        "prompt": prompt,
        "object_cohort": cohort,
        "time_range": {"start": start, "end": end},
        "correct_answer": answer,
        "required_evidence_items": evidence,
        "acceptable_alternative_explanations": alternatives,
        "scoring_rubric": _rubric(),
        "maximum_score": 100,
        "baseline_view": ["object lifecycles", "OCDFG", "conventional KPIs"],
        "treatment_view": [
            "object lifecycles",
            "OCDFG",
            "conventional KPIs",
            "state calendars",
            "SA-OCDFG",
            "transition KPIs",
            "state patterns",
            "boundary windows",
        ],
        "pre_registered": True,
    }


def _inventory(root: Path) -> list[dict[str, Any]]:
    transitions = _csv(root / "truth" / "transitions.csv")
    latent = {row["event_id"]: row for row in _csv(root / "truth" / "latent_regime_at_event.csv")}
    patterns = _csv(root / "truth" / "injected_pattern_instances.csv")
    violations = _csv(root / "truth" / "conformance_violations.csv")
    outcomes = _csv(root / "truth" / "outcomes_by_object.csv")
    shortage = next(row for row in transitions if row["to_state"] == "Understock")
    excess = next(row for row in patterns if row["pattern_id"] == "INV-P3")
    missing_action = next(row for row in violations if row["rule_id"] == "INV-C1")
    compared = outcomes[:2]
    return [
        _case(
            identifier="INV-TASK-1",
            title="Dominant Understock cause",
            prompt="Identify the dominant mechanism behind the selected Understock entry.",
            cohort=[shortage["leading_object_id"]],
            start=shortage["from_state_started_at"],
            end=shortage["event_time"],
            answer={"primary_regime": latent[shortage["event_id"]]["primary_regime"]},
            evidence=[shortage["transition_id"], shortage["event_id"]],
            alternatives=["supplier delay", "policy threshold change", "count discrepancy"],
        ),
        _case(
            identifier="INV-TASK-2",
            title="Receipt-driven Overstock",
            prompt="Distinguish receipt-driven excess from demand- or policy-driven Overstock.",
            cohort=[excess["leading_object_id"]],
            start=latent[excess["start_event_id"]]["event_time"],
            end=latent[excess["end_event_id"]]["event_time"],
            answer={"mechanism": "receipt-driven excess", "pattern_id": "INV-P3"},
            evidence=[excess["instance_id"], excess["start_event_id"], excess["end_event_id"]],
            alternatives=["stable low movement", "policy threshold reduction"],
        ),
        _case(
            identifier="INV-TASK-3",
            title="Critical shortage without action",
            prompt="Find a Critical Understock episode without timely replenishment action.",
            cohort=[missing_action["leading_object_id"]],
            start=missing_action["event_time"],
            end=missing_action["expected_deadline"],
            answer={"rule_id": "INV-C1", "violation_id": missing_action["violation_id"]},
            evidence=[missing_action["event_id"], missing_action["violation_id"]],
            alternatives=["proposal was late rather than absent"],
        ),
        _case(
            identifier="INV-TASK-4",
            title="Recovery cohort comparison",
            prompt="Compare shortage and transition outcomes for two item-locations.",
            cohort=[row["leading_object_id"] for row in compared],
            start=shortage["from_state_started_at"],
            end=max(row["event_time"] for row in transitions),
            answer={row["leading_object_id"]: row for row in compared},
            evidence=[row["leading_object_id"] for row in compared],
            alternatives=["supplier mix", "location policy", "demand mix"],
        ),
    ]


def _manufacturing(root: Path) -> list[dict[str, Any]]:
    patterns = _csv(root / "truth" / "injected_pattern_instances.csv")
    violations = _csv(root / "truth" / "conformance_violations.csv")
    latent = {row["event_id"]: row for row in _csv(root / "truth" / "latent_regime_at_event.csv")}
    by_pattern = {row["pattern_id"]: row for row in patterns}
    policy = [row for row in violations if row["rule_id"] in {"MFG-C3", "MFG-C5"}]
    p1 = by_pattern["MFG-P1"]
    p3 = by_pattern["MFG-P3"]
    p4 = by_pattern["MFG-P4"]
    p5 = by_pattern["MFG-P5"]
    return [
        _case(
            identifier="MFG-TASK-1",
            title="Path from Degraded to Down",
            prompt="Identify the dominant path from Degraded to Down for the selected machine.",
            cohort=[p1["leading_object_id"]],
            start=latent[p1["start_event_id"]]["event_time"],
            end=latent[p1["end_event_id"]]["event_time"],
            answer={"pattern_id": "MFG-P1", "sequence": json.loads(p1["expected_sequence_json"])},
            evidence=[p1["instance_id"], p1["start_event_id"], p1["end_event_id"]],
            alternatives=["thermal drift", "quality failure"],
        ),
        _case(
            identifier="MFG-TASK-2",
            title="Quick versus slow recovery",
            prompt="Explain why one recovery was slower than the comparison recovery.",
            cohort=[p3["leading_object_id"], p4["leading_object_id"]],
            start=min(
                latent[p3["start_event_id"]]["event_time"],
                latent[p4["start_event_id"]]["event_time"],
            ),
            end=max(
                latent[p3["end_event_id"]]["event_time"],
                latent[p4["end_event_id"]]["event_time"],
            ),
            answer={
                "quick_pattern": "MFG-P3",
                "slow_pattern": "MFG-P4",
                "delay_step": "Part Unavailable",
            },
            evidence=[p3["instance_id"], p4["instance_id"]],
            alternatives=["test failure", "repair rework", "team response"],
        ),
        _case(
            identifier="MFG-TASK-3",
            title="Post-repair recurrence",
            prompt="Find a machine returning to Degraded shortly after repair.",
            cohort=[p5["leading_object_id"]],
            start=latent[p5["start_event_id"]]["event_time"],
            end=latent[p5["end_event_id"]]["event_time"],
            answer={"pattern_id": "MFG-P5", "instance_id": p5["instance_id"]},
            evidence=[p5["start_event_id"], p5["end_event_id"]],
            alternatives=["new fault", "sensor false positive"],
        ),
        _case(
            identifier="MFG-TASK-4",
            title="Restart and quality policy",
            prompt="Verify whether restart and quality-hold policies were followed.",
            cohort=sorted({row["leading_object_id"] for row in policy}),
            start=min(row["event_time"] for row in policy),
            end=max(row["event_time"] for row in policy),
            answer={"violations": [row["violation_id"] for row in policy]},
            evidence=[row["event_id"] for row in policy],
            alternatives=["passed inspection was recorded on another work order"],
        ),
    ]


def generate_analyst_tasks(run_dir: Path, scenario: str) -> None:
    cases = _inventory(run_dir) if scenario == "inventory" else _manufacturing(run_dir)
    writer = RunWriter(run_dir)
    for case in cases:
        writer.write_json(f"tasks/{case['task_id'].lower()}.json", case)
    writer.write_json(
        "tasks/answer_key.json",
        {
            "scenario": scenario,
            "tasks": [
                {
                    "task_id": case["task_id"],
                    "correct_answer": case["correct_answer"],
                    "required_evidence_items": case["required_evidence_items"],
                    "maximum_score": case["maximum_score"],
                }
                for case in cases
            ],
        },
    )
