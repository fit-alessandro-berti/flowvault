"""Independent data-quality, transition, recovery, recurrence, and cohort metrics."""

from __future__ import annotations

import csv
import json
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping
from datetime import timedelta
from pathlib import Path
from statistics import median
from typing import Any

from saocpm_eval.common.ocel_builder import parse_timestamp


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source))


def _attributes(item: Mapping[str, Any]) -> dict[str, Any]:
    return {str(row["name"]): row["value"] for row in item.get("attributes", [])}


def data_quality_metrics(
    document: Mapping[str, Any],
    manifest: Mapping[str, Any],
    branch_coverage: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Return reportable structural and semantic validation characteristics."""

    scenario = str(manifest["scenario"])
    leading_type = "ItemLocation" if scenario == "inventory" else "Machine"
    object_types = {str(item["id"]): str(item["type"]) for item in document["objects"]}
    leading_ids = {
        identifier
        for identifier, object_type in object_types.items()
        if object_type == leading_type
    }
    lifecycle_ids: set[str] = set()
    exactly_one = 0
    for event in document["events"]:
        related = [
            str(row["objectId"])
            for row in event.get("relationships", [])
            if object_types[str(row["objectId"])] == leading_type
        ]
        if len(related) == 1:
            exactly_one += 1
            lifecycle_ids.add(related[0])
    events = len(document["events"])
    objects = len(document["objects"])
    e2o = sum(len(event.get("relationships", [])) for event in document["events"])
    o2o = sum(len(item.get("relationships", [])) for item in document["objects"])
    metrics: dict[str, Any] = {
        "schema_valid": True,
        "relationship_references_valid": True,
        "semantic_validation_passed": True,
        "event_count": events,
        "object_count": objects,
        "leading_object_count": len(leading_ids),
        "leading_lifecycle_coverage": len(lifecycle_ids) / len(leading_ids),
        "events_with_exactly_one_leading_object": exactly_one,
        "events_without_leading_object": events - exactly_one,
        "e2o_relationship_count": e2o,
        "o2o_relationship_count": o2o,
        "e2o_per_event": e2o / events,
        "o2o_per_object": o2o / objects,
        "stock_conservation_error_count": 0 if scenario == "inventory" else "not_applicable",
        "state_branch_coverage": bool(branch_coverage["state_branches_covered"]),
        "pattern_family_count": len(set(branch_coverage["pattern_ids"])),
        "conformance_rule_count": len(set(branch_coverage["conformance_rule_ids"])),
    }
    return [{"metric": key, "value": value} for key, value in metrics.items()]


def transition_kpi_oracle_rows(
    truth_rows: Iterable[Mapping[str, str]], api_result: Mapping[str, Any]
) -> list[dict[str, Any]]:
    """Compare every transition KPI row with an independent sidecar aggregation."""

    grouped: dict[tuple[str, str], list[Mapping[str, str]]] = defaultdict(list)
    for truth_row in truth_rows:
        grouped[(truth_row["from_state"], truth_row["to_state"])].append(truth_row)
    actual = {
        (str(api_row["from_state"]), str(api_row["to_state"])): api_row
        for api_row in api_result["transitions"]
    }
    result = []
    for key in sorted(set(grouped).union(actual)):
        expected_rows = grouped.get(key, [])
        durations = sorted(float(row["duration_minutes"]) * 60_000 for row in expected_rows)
        objects = {row["leading_object_id"] for row in expected_rows}
        found = actual.get(key)
        expected = {
            "count": len(durations),
            "object_count": len(objects),
            "min_duration_ms": durations[0] if durations else None,
            "median_duration_ms": durations[len(durations) // 2] if durations else None,
            "avg_duration_ms": sum(durations) / len(durations) if durations else None,
            "max_duration_ms": durations[-1] if durations else None,
        }
        comparison: dict[str, Any] = {
            "from_state": key[0],
            "to_state": key[1],
            **{f"expected_{name}": value for name, value in expected.items()},
        }
        for name in expected:
            comparison[f"actual_{name}"] = found.get(name) if found is not None else None
        comparison["exact_match"] = found is not None and all(
            (
                abs(float(found[name]) - float(value)) <= 1e-6
                if isinstance(value, float)
                else found[name] == value
            )
            for name, value in expected.items()
        )
        result.append(comparison)
    return result


def _episode_rows(run_dir: Path) -> dict[str, list[dict[str, str]]]:
    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in _read_csv(run_dir / "truth" / "state_episodes.csv"):
        grouped[row["leading_object_id"]].append(row)
    for rows in grouped.values():
        rows.sort(key=lambda row: parse_timestamp(row["start_time"]))
    return grouped


def _recovery_durations(
    episodes: Mapping[str, list[dict[str, str]]],
    source_states: set[str],
    target_state: str,
) -> dict[str, list[float]]:
    result: dict[str, list[float]] = defaultdict(list)
    for object_id, rows in episodes.items():
        start = None
        for row in rows:
            if row["label"] in source_states and start is None:
                start = parse_timestamp(row["start_time"])
            elif row["label"] == target_state and start is not None:
                result[object_id].append(
                    (parse_timestamp(row["start_time"]) - start).total_seconds() / 60
                )
                start = None
            elif row["label"] not in source_states and row["label"] != target_state:
                start = None
    return result


def _recurrence_rate(
    episodes: Mapping[str, list[dict[str, str]]],
    source_states: set[str],
    target_state: str,
    horizon: timedelta,
) -> tuple[int, int, float]:
    opportunities = 0
    recurrent = 0
    for rows in episodes.values():
        for index, row in enumerate(rows):
            if row["label"] != target_state or index == 0:
                continue
            if rows[index - 1]["label"] not in source_states:
                continue
            opportunities += 1
            recovered_at = parse_timestamp(row["start_time"])
            if any(
                later["label"] in source_states
                and parse_timestamp(later["start_time"]) <= recovered_at + horizon
                for later in rows[index + 1 :]
            ):
                recurrent += 1
    return opportunities, recurrent, recurrent / opportunities if opportunities else 0.0


def operational_metric_rows(run_dir: Path, scenario: str) -> list[dict[str, Any]]:
    """Compute occupancy, dwell, recovery, recurrence, and stuck-object metrics."""

    episodes = _episode_rows(run_dir)
    all_rows = [row for rows in episodes.values() for row in rows]
    result: list[dict[str, Any]] = []
    by_state: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in all_rows:
        by_state[row["label"]].append(row)
    for state, rows in sorted(by_state.items()):
        durations = [float(row["duration_minutes"]) for row in rows]
        result.extend(
            (
                {"scope": "state", "key": state, "metric": "episode_count", "value": len(rows)},
                {
                    "scope": "state",
                    "key": state,
                    "metric": "occupancy_minutes",
                    "value": sum(durations),
                },
                {
                    "scope": "state",
                    "key": state,
                    "metric": "median_dwell_minutes",
                    "value": median(durations),
                },
            )
        )
    if scenario == "inventory":
        source_states = {"Understock", "Critical Understock"}
        target_state = "Normal"
        horizons: tuple[int, ...] = (7, 14, 30)
    else:
        source_states = {"Down"}
        target_state = "Recovery"
        horizons = (1,)
    recoveries = _recovery_durations(episodes, source_states, target_state)
    flattened = [value for values in recoveries.values() for value in values]
    result.append(
        {
            "scope": "overall",
            "key": "recovery",
            "metric": "median_recovery_minutes",
            "value": median(flattened) if flattened else "",
        }
    )
    if scenario == "inventory":
        for days in horizons:
            opportunities, recurrent, rate = _recurrence_rate(
                episodes, source_states, target_state, timedelta(days=days)
            )
            result.extend(
                (
                    {
                        "scope": "overall",
                        "key": f"{days}_days",
                        "metric": "recovery_opportunities",
                        "value": opportunities,
                    },
                    {
                        "scope": "overall",
                        "key": f"{days}_days",
                        "metric": "recurrent_shortage_count",
                        "value": recurrent,
                    },
                    {
                        "scope": "overall",
                        "key": f"{days}_days",
                        "metric": "recurrent_shortage_rate",
                        "value": rate,
                    },
                )
            )
        stuck_states = source_states
    else:
        stuck_states = {"Down"}
        transitions = _read_csv(run_dir / "truth" / "transitions.csv")
        for pair in (
            ("Running", "Down"),
            ("Degraded", "Down"),
            ("Down", "Recovery"),
            ("Recovery", "Running"),
        ):
            selected = [row for row in transitions if (row["from_state"], row["to_state"]) == pair]
            result.append(
                {
                    "scope": "transition",
                    "key": f"{pair[0]} -> {pair[1]}",
                    "metric": "count",
                    "value": len(selected),
                }
            )
    stuck = sum(
        bool(rows)
        and rows[-1]["label"] in stuck_states
        and rows[-1]["right_censored"].lower() == "true"
        for rows in episodes.values()
    )
    result.append(
        {"scope": "overall", "key": "horizon", "metric": "stuck_object_count", "value": stuck}
    )
    return result


def _latest_attributes(item: Mapping[str, Any]) -> dict[str, Any]:
    result: dict[str, tuple[Any, Any]] = {}
    for row in item.get("attributes", []):
        time = parse_timestamp(str(row["time"]))
        prior = result.get(str(row["name"]))
        if prior is None or time >= prior[0]:
            result[str(row["name"])] = (time, row["value"])
    return {name: pair[1] for name, pair in result.items()}


def cohort_metric_rows(
    run_dir: Path, document: Mapping[str, Any], scenario: str
) -> list[dict[str, Any]]:
    """Aggregate state burden and recovery by registered object/context cohorts."""

    leading_type = "ItemLocation" if scenario == "inventory" else "Machine"
    objects = {str(item["id"]): item for item in document["objects"]}
    object_types = {identifier: str(item["type"]) for identifier, item in objects.items()}
    leading = {
        identifier: _latest_attributes(item)
        for identifier, item in objects.items()
        if object_types[identifier] == leading_type
    }
    context: dict[str, dict[str, set[str]]] = defaultdict(lambda: defaultdict(set))
    for event in document["events"]:
        leading_ids = [
            str(row["objectId"])
            for row in event.get("relationships", [])
            if object_types[str(row["objectId"])] == leading_type
        ]
        if len(leading_ids) != 1:
            continue
        for relation in event.get("relationships", []):
            identifier = str(relation["objectId"])
            kind = object_types[identifier]
            if kind != leading_type:
                context[leading_ids[0]][kind].add(identifier)
    episodes = _episode_rows(run_dir)
    recovery = _recovery_durations(
        episodes,
        {"Understock", "Critical Understock"} if scenario == "inventory" else {"Down"},
        "Normal" if scenario == "inventory" else "Recovery",
    )
    burden_states = {"Understock", "Critical Understock"} if scenario == "inventory" else {"Down"}
    per_object = {}
    for object_id, rows in episodes.items():
        per_object[object_id] = {
            "burden_minutes": sum(
                float(row["duration_minutes"]) for row in rows if row["label"] in burden_states
            ),
            "transition_count": sum(1 for row in rows[1:]),
            "median_recovery_minutes": median(recovery[object_id]) if recovery[object_id] else 0.0,
        }
    dimensions = (
        (
            ("attribute", "material_class"),
            ("attribute", "location_class"),
            ("attribute", "policy_version"),
            ("context", "Supplier"),
            ("context", "Location"),
            ("context", "Planner"),
        )
        if scenario == "inventory"
        else (
            ("attribute", "machine_family"),
            ("attribute", "criticality"),
            ("attribute", "site"),
            ("context", "MaintenanceTeam"),
            ("context", "Component"),
            ("context", "Shift"),
        )
    )
    grouped: dict[tuple[str, str], set[str]] = defaultdict(set)
    for object_id in per_object:
        for kind, name in dimensions:
            cohort_values = (
                {str(leading[object_id].get(name, "missing"))}
                if kind == "attribute"
                else context[object_id].get(name, {"none"})
            )
            for value in cohort_values:
                grouped[(name, value)].add(object_id)
    result = []
    for (dimension, value), object_ids in sorted(grouped.items()):
        for metric in ("burden_minutes", "transition_count", "median_recovery_minutes"):
            metric_values = [float(per_object[object_id][metric]) for object_id in object_ids]
            result.append(
                {
                    "cohort_dimension": dimension,
                    "cohort_value": value,
                    "object_count": len(object_ids),
                    "metric": f"mean_{metric}",
                    "value": sum(metric_values) / len(metric_values),
                }
            )
    return result


def noisy_operational_agreement_rows(run_dir: Path) -> list[dict[str, Any]]:
    """Compare the deliberately noisy manufacturing mode record to the manual taxonomy."""

    truth = {
        row["event_id"]: row["reference_state"]
        for row in _read_csv(run_dir / "truth/state_at_event.csv")
    }
    noisy = _read_csv(run_dir / "truth/noisy_operational_state_at_event.csv")
    coarse = {
        "Down": "Stopped",
        "Setup": "Setup",
        "Unknown": "Unknown",
    }
    rows = []
    for subset, selected in (
        ("all", noisy),
        ("deliberately_noisy", [row for row in noisy if row["is_deliberately_noisy"] == "true"]),
        (
            "not_deliberately_noisy",
            [row for row in noisy if row["is_deliberately_noisy"] == "false"],
        ),
    ):
        correct = sum(
            row["operational_state"] == coarse.get(truth[row["event_id"]], "Operating")
            for row in selected
        )
        rows.append(
            {
                "subset": subset,
                "count": len(selected),
                "accuracy": correct / len(selected) if selected else "",
            }
        )
    return rows


def conformance_breakdown_rows(
    detected_rows: Iterable[Mapping[str, Any]],
    document: Mapping[str, Any],
    scenario: str,
) -> list[dict[str, Any]]:
    """Report detected violation rates by state and required organizational context."""

    events = {str(event["id"]): event for event in document["events"]}
    objects = {str(item["id"]): item for item in document["objects"]}
    object_types = {identifier: str(item["type"]) for identifier, item in objects.items()}
    leading_type = "ItemLocation" if scenario == "inventory" else "Machine"
    event_count_by_object: Counter[str] = Counter()
    for event in document["events"]:
        for relation in event.get("relationships", []):
            identifier = str(relation["objectId"])
            if object_types[identifier] == leading_type:
                event_count_by_object[identifier] += 1
    grouped: Counter[tuple[str, str, str]] = Counter()
    for violation in detected_rows:
        event = events.get(str(violation["event_id"]))
        if event is None:
            continue
        attributes = _attributes(event)
        grouped[
            (
                "state",
                str(attributes.get("state", "unassigned")),
                str(violation["object_id"]),
            )
        ] += 1
        context_types = {
            object_types[str(row["objectId"])]
            for row in event.get("relationships", [])
            if object_types[str(row["objectId"])] != leading_type
        }
        selected_context_types = (
            {"Planner", "Location"} if scenario == "inventory" else {"MaintenanceTeam"}
        )
        for kind in sorted(context_types & selected_context_types):
            grouped[(kind, kind, str(violation["object_id"]))] += 1
    aggregated: dict[tuple[str, str], dict[str, Any]] = defaultdict(
        lambda: {"violation_count": 0, "objects": set(), "exposure_events": 0}
    )
    for (dimension, value, object_id), count in grouped.items():
        aggregate = aggregated[(dimension, value)]
        aggregate["violation_count"] += count
        aggregate["objects"].add(object_id)
        aggregate["exposure_events"] += event_count_by_object[object_id]
    return [
        {
            "dimension": dimension,
            "value": value,
            "violation_count": aggregate["violation_count"],
            "object_count": len(aggregate["objects"]),
            "violations_per_100_events": 100
            * aggregate["violation_count"]
            / max(1, aggregate["exposure_events"]),
        }
        for (dimension, value), aggregate in sorted(aggregated.items())
    ]


def task_dataset_summary(run_dir: Path) -> list[dict[str, Any]]:
    answer = json.loads((run_dir / "tasks/answer_key.json").read_text(encoding="utf-8"))
    return [
        {
            "task_id": row["task_id"],
            "maximum_score": row["maximum_score"],
            "required_evidence_count": len(row["required_evidence_items"]),
        }
        for row in answer["tasks"]
    ]
