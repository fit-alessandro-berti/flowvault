"""Stability, transfer, warning, and cell-explanation diagnostics for SOM windows."""

from __future__ import annotations

import csv
import json
import math
from bisect import bisect_right
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from datetime import timedelta
from itertools import pairwise
from pathlib import Path
from statistics import mean, pstdev
from typing import Any

import numpy as np

from saocpm_eval.analytics.episodes import StateEpisode
from saocpm_eval.analytics.prediction import score_episode_alerts
from saocpm_eval.analytics.som_evaluation import align_cells
from saocpm_eval.common.ocel_builder import parse_timestamp


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source))


def _balanced_accuracy(labels: Sequence[str], predicted: Sequence[str]) -> float:
    recalls = []
    for label in sorted(set(labels)):
        indices = [index for index, value in enumerate(labels) if value == label]
        recalls.append(sum(predicted[index] == label for index in indices) / len(indices))
    return mean(recalls) if recalls else 0.0


def bootstrap_stability_rows(
    windows: Sequence[Mapping[str, Any]],
    endpoint: Sequence[str],
    majority: Sequence[str],
    *,
    total_cells: int,
    repetitions: int = 20,
    seed: int = 20260826,
) -> list[dict[str, Any]]:
    """Object-resample cell/label stability with deterministic bootstrap streams."""

    by_object: dict[str, list[int]] = defaultdict(list)
    for index, window in enumerate(windows):
        by_object[str(window["object_id"])].append(index)
    objects = sorted(by_object)
    rng = np.random.default_rng(seed)
    values: dict[str, list[float]] = {"endpoint": [], "majority": []}
    for _ in range(repetitions):
        sampled = rng.choice(objects, size=len(objects), replace=True)
        indices = [index for object_id in sampled for index in by_object[str(object_id)]]
        cells = [f"{windows[index]['cell_x']},{windows[index]['cell_y']}" for index in indices]
        for kind, labels in (("endpoint", endpoint), ("majority", majority)):
            selected = [labels[index] for index in indices]
            values[kind].append(
                align_cells(
                    cells, selected, total_cell_count=total_cells
                ).normalized_mutual_information
            )
    return [
        {
            "diagnostic": "object_bootstrap",
            "label_kind": kind,
            "repetitions": repetitions,
            "mean_nmi": mean(scores),
            "standard_deviation_nmi": pstdev(scores),
            "minimum_nmi": min(scores),
        }
        for kind, scores in values.items()
    ]


def period_stability_rows(
    windows: Sequence[Mapping[str, Any]],
    endpoint: Sequence[str],
    majority: Sequence[str],
    event_times: Mapping[str, Any],
    *,
    total_cells: int,
) -> list[dict[str, Any]]:
    """Compare alignment quality in early and late observation periods."""

    ordered_times = sorted(event_times[str(window["end_event"])] for window in windows)
    split_time = ordered_times[len(ordered_times) // 2]
    result = []
    for period in ("early", "late"):
        indices = [
            index
            for index, window in enumerate(windows)
            if (
                event_times[str(window["end_event"])] <= split_time
                if period == "early"
                else event_times[str(window["end_event"])] > split_time
            )
        ]
        cells = [f"{windows[index]['cell_x']},{windows[index]['cell_y']}" for index in indices]
        for kind, labels in (("endpoint", endpoint), ("majority", majority)):
            selected = [labels[index] for index in indices]
            score = align_cells(cells, selected, total_cell_count=total_cells)
            result.append(
                {
                    "diagnostic": "period",
                    "period": period,
                    "label_kind": kind,
                    "window_count": len(indices),
                    "purity": score.purity,
                    "nmi": score.normalized_mutual_information,
                    "balanced_accuracy": score.balanced_accuracy,
                }
            )
    return result


def _static_attributes(document: Mapping[str, Any]) -> dict[str, dict[str, str]]:
    result = {}
    for item in document["objects"]:
        values: dict[str, str] = {}
        for attribute in item.get("attributes", []):
            values[str(attribute["name"])] = str(attribute["value"])
        result[str(item["id"])] = values
    return result


def transfer_rows(
    windows: Sequence[Mapping[str, Any]],
    labels: Sequence[str],
    document: Mapping[str, Any],
    scenario: str,
) -> list[dict[str, Any]]:
    """Fit cell labels outside each cohort and score held-out class/site cohorts."""

    attributes = _static_attributes(document)
    dimensions = (
        ("material_class", "location_class")
        if scenario == "inventory"
        else ("machine_family", "site")
    )
    result = []
    for dimension in dimensions:
        values = sorted(
            {attributes[str(window["object_id"])].get(dimension, "missing") for window in windows}
        )
        for held_out in values:
            train = [
                index
                for index, window in enumerate(windows)
                if attributes[str(window["object_id"])].get(dimension, "missing") != held_out
            ]
            train_set = set(train)
            test = [index for index in range(len(windows)) if index not in train_set]
            if not train or not test:
                continue
            by_cell: dict[str, Counter[str]] = defaultdict(Counter)
            for index in train:
                window = windows[index]
                cell = f"{window['cell_x']},{window['cell_y']}"
                by_cell[cell][labels[index]] += 1
            fallback = Counter(labels[index] for index in train).most_common(1)[0][0]
            expected = [labels[index] for index in test]
            predicted = [
                by_cell[f"{windows[index]['cell_x']},{windows[index]['cell_y']}"].most_common(1)[0][
                    0
                ]
                if by_cell[f"{windows[index]['cell_x']},{windows[index]['cell_y']}"]
                else fallback
                for index in test
            ]
            result.append(
                {
                    "dimension": dimension,
                    "held_out_value": held_out,
                    "train_windows": len(train),
                    "test_windows": len(test),
                    "balanced_accuracy": _balanced_accuracy(expected, predicted),
                }
            )
    return result


def warning_rows(
    run_dir: Path,
    windows: Sequence[Mapping[str, Any]],
    mapping: Mapping[str, str],
    event_times: Mapping[str, Any],
    scenario: str,
) -> list[dict[str, Any]]:
    """Score entries into mapped risk cells against subsequent adverse-state episodes."""

    risk_labels = (
        {
            "Demand Surge Without Inbound",
            "Supplier Delay",
            "Replenishment In Transit",
            "Forecast or Policy Bias",
        }
        if scenario == "inventory"
        else {
            "Bearing Degradation",
            "Thermal Drift",
            "Quality Drift",
            "Alarm Escalation",
            "Waiting for Maintenance",
            "Failed",
        }
    )
    target_states = {"Understock", "Critical Understock"} if scenario == "inventory" else {"Down"}
    horizon = timedelta(days=7) if scenario == "inventory" else timedelta(hours=24)
    episodes = [
        StateEpisode(
            object_id=row["leading_object_id"],
            state=row["label"],
            start_time=parse_timestamp(row["start_time"]),
            end_time=parse_timestamp(row["end_time"]),
            start_event_id=row["start_event_id"],
            end_event_id=row["end_event_id"],
            event_count=int(row["event_count"]),
            right_censored=row["right_censored"] == "true",
        )
        for row in _read_csv(run_dir / "truth/state_episodes.csv")
        if row["label"] in target_states
    ]
    by_object: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for window in windows:
        by_object[str(window["object_id"])].append(window)
    alerts = []
    for object_id, rows in by_object.items():
        rows.sort(key=lambda row: event_times[str(row["end_event"])])
        was_risk = False
        for row in rows:
            cell = f"{row['cell_x']},{row['cell_y']}"
            is_risk = mapping.get(cell) in risk_labels
            if is_risk and not was_risk:
                alerts.append((object_id, event_times[str(row["end_event"])]))
            was_risk = is_risk
    score = score_episode_alerts(alerts, episodes, horizon)
    manifest = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
    exposure_weeks = max(
        1 / 7,
        (
            parse_timestamp(manifest["end_time"]) - parse_timestamp(manifest["start_time"])
        ).total_seconds()
        / (7 * 86400),
    )
    object_count = max(1, len(by_object))
    return [
        {
            "risk_cell_entry_count": len(alerts),
            "warning_precision": score.true_alerts / len(alerts) if alerts else 1.0,
            "warning_recall": score.event_sensitivity,
            "median_lead_time_minutes": (
                score.median_lead_time_seconds / 60
                if score.median_lead_time_seconds is not None
                else ""
            ),
            "false_entries_per_object_week": score.false_alerts / (object_count * exposure_weeks),
        }
    ]


def _dynamic_attribute_index(
    document: Mapping[str, Any], object_ids: set[str]
) -> dict[str, dict[str, tuple[list[Any], list[float]]]]:
    result: dict[str, dict[str, tuple[list[Any], list[float]]]] = defaultdict(dict)
    for item in document["objects"]:
        object_id = str(item["id"])
        if object_id not in object_ids:
            continue
        grouped: dict[str, list[tuple[Any, float]]] = defaultdict(list)
        for row in item.get("attributes", []):
            value = row["value"]
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                continue
            grouped[str(row["name"])].append((parse_timestamp(str(row["time"])), float(value)))
        for name, rows in grouped.items():
            rows.sort()
            result[object_id][f"attribute::{name}"] = (
                [row[0] for row in rows],
                [row[1] for row in rows],
            )
    return result


def cell_explanation_rows(
    windows: Sequence[Mapping[str, Any]],
    document: Mapping[str, Any],
    event_times: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Explain cells with standardized observed features and boundary windows."""

    events = {str(row["id"]): row for row in document["events"]}
    object_types = {str(row["id"]): str(row["type"]) for row in document["objects"]}
    object_ids = {str(window["object_id"]) for window in windows}
    dynamic = _dynamic_attribute_index(document, object_ids)
    lifecycle: dict[str, list[str]] = defaultdict(list)
    for event in document["events"]:
        for relation in event.get("relationships", []):
            object_id = str(relation["objectId"])
            if object_id in object_ids:
                lifecycle[object_id].append(str(event["id"]))
                break
    positions = {
        object_id: {event_id: index for index, event_id in enumerate(event_ids)}
        for object_id, event_ids in lifecycle.items()
    }
    global_stats: dict[str, list[float]] = defaultdict(lambda: [0.0, 0.0, 0.0])
    cell_stats: dict[str, dict[str, list[float]]] = defaultdict(
        lambda: defaultdict(lambda: [0.0, 0.0, 0.0])
    )
    activities: dict[str, Counter[str]] = defaultdict(Counter)
    contexts: dict[str, Counter[str]] = defaultdict(Counter)
    cell_window_counts: Counter[str] = Counter()
    by_object: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for window in windows:
        object_id = str(window["object_id"])
        cell = f"{window['cell_x']},{window['cell_y']}"
        cell_window_counts[cell] += 1
        end_time = event_times[str(window["end_event"])]
        feature_values: dict[str, float] = {}
        for name, (times, values) in dynamic[object_id].items():
            index = bisect_right(times, end_time) - 1
            if index >= 0:
                feature_values[name] = values[index]
        start = positions[object_id][str(window["start_event"])]
        end = positions[object_id][str(window["end_event"])]
        context_ids: dict[str, set[str]] = defaultdict(set)
        for event_id in lifecycle[object_id][start : end + 1]:
            event = events[event_id]
            activity = str(event["type"])
            activities[cell][activity] += 1
            feature_values[f"activity::{activity}"] = (
                feature_values.get(f"activity::{activity}", 0.0) + 1
            )
            for relation in event.get("relationships", []):
                related = str(relation["objectId"])
                if related != object_id:
                    context_ids[object_types[related]].add(related)
        for kind, identifiers in context_ids.items():
            feature_values[f"related::{kind}"] = float(len(identifiers))
            contexts[cell][kind] += len(identifiers)
        for name, value in feature_values.items():
            for stats in (global_stats[name], cell_stats[cell][name]):
                stats[0] += 1
                stats[1] += value
                stats[2] += value * value
        by_object[object_id].append(window)
    entering: dict[str, list[dict[str, str]]] = defaultdict(list)
    exiting: dict[str, list[dict[str, str]]] = defaultdict(list)
    for object_id, rows in by_object.items():
        rows.sort(key=lambda row: event_times[str(row["end_event"])])
        for left, right in pairwise(rows):
            left_cell = f"{left['cell_x']},{left['cell_y']}"
            right_cell = f"{right['cell_x']},{right['cell_y']}"
            if left_cell == right_cell:
                continue
            boundary = {
                "object_id": object_id,
                "from_end_event": str(left["end_event"]),
                "to_end_event": str(right["end_event"]),
            }
            if len(exiting[left_cell]) < 3:
                exiting[left_cell].append(boundary)
            if len(entering[right_cell]) < 3:
                entering[right_cell].append(boundary)
    result = []
    for cell in sorted(cell_stats):
        differences = []
        for name, values in cell_stats[cell].items():
            all_values = global_stats[name]
            all_count, all_sum, all_sum_squares = all_values
            variance = max(0.0, all_sum_squares / all_count - (all_sum / all_count) ** 2)
            deviation = math.sqrt(variance)
            if deviation == 0:
                continue
            count, total, _ = values
            others_count = all_count - count
            others_mean = (all_sum - total) / others_count if others_count else all_sum / all_count
            differences.append((name, (total / count - others_mean) / deviation))
        differences.sort(key=lambda row: (-abs(row[1]), row[0]))
        result.append(
            {
                "cell": cell,
                "window_count": cell_window_counts[cell],
                "largest_standardized_feature_differences_json": json.dumps(
                    differences[:5], separators=(",", ":")
                ),
                "dominant_activity": (
                    activities[cell].most_common(1)[0][0] if activities[cell] else ""
                ),
                "related_object_counts_json": json.dumps(
                    dict(contexts[cell]), sort_keys=True, separators=(",", ":")
                ),
                "representative_entering_windows_json": json.dumps(
                    entering[cell], sort_keys=True, separators=(",", ":")
                ),
                "representative_exiting_windows_json": json.dumps(
                    exiting[cell], sort_keys=True, separators=(",", ":")
                ),
            }
        )
    return result
