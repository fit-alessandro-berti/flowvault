"""Leakage-safe feature extraction from observed OCEL and decision-label sidecars."""

from __future__ import annotations

import csv
import json
from bisect import bisect_right
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

import pandas as pd

from saocpm_eval.common.ocel_builder import parse_timestamp

INVENTORY_RAW = (
    "on_hand_after",
    "reserved_after",
    "backorder_after",
    "on_order_after",
    "inventory_position_after",
    "lower_threshold",
    "upper_threshold",
    "confirmed_demand_horizon",
    "inbound_horizon",
)
MANUFACTURING_RAW = (
    "health_index",
    "vibration_rms",
    "temperature_c",
    "power_kw",
    "load_fraction",
    "stable_run_minutes",
)


def _attributes(items: list[dict[str, Any]]) -> dict[str, Any]:
    return {item["name"]: item["value"] for item in items}


@dataclass(frozen=True)
class PredictionFeatureContext:
    """Indexes shared by all prediction tasks from one immutable evaluation run."""

    scenario: str
    leading_type: str
    object_types: dict[str, str]
    event_by_id: dict[str, dict[str, Any]]
    event_time_by_id: dict[str, datetime]
    lifecycle: dict[str, list[dict[str, Any]]]
    position_by_event: dict[str, int]
    state_features: dict[str, tuple[float, int]]
    static_names: tuple[str, ...]
    static_history: dict[str, dict[str, tuple[list[datetime], list[Any]]]]


def prepare_prediction_context(
    run_dir: Path, document: dict[str, Any] | None = None
) -> PredictionFeatureContext:
    """Build the expensive OCEL indexes once and reuse them across prediction tasks."""

    if document is None:
        document = json.loads((run_dir / "observed.ocel.json").read_text(encoding="utf-8"))
    manifest = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
    scenario = str(manifest["scenario"])
    leading_type = "ItemLocation" if scenario == "inventory" else "Machine"
    objects = {str(item["id"]): item for item in document["objects"]}
    object_types = {identifier: str(item["type"]) for identifier, item in objects.items()}
    event_by_id = {str(event["id"]): event for event in document["events"]}
    event_time_by_id = {
        str(event["id"]): parse_timestamp(event["time"]) for event in document["events"]
    }
    lifecycle: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for event in document["events"]:
        leading = [
            str(relationship["objectId"])
            for relationship in event["relationships"]
            if object_types[str(relationship["objectId"])] == leading_type
        ]
        if leading:
            lifecycle[leading[0]].append(event)
    for events in lifecycle.values():
        events.sort(key=lambda event: (event_time_by_id[str(event["id"])], str(event["id"])))
    position_by_event = {
        str(event["id"]): position
        for events in lifecycle.values()
        for position, event in enumerate(events)
    }
    state_features: dict[str, tuple[float, int]] = {}
    state_tracking: dict[str, tuple[str, datetime, int]] = {}
    with (run_dir / "truth" / "state_at_event.csv").open(encoding="utf-8", newline="") as source:
        for state_row in csv.DictReader(source):
            object_id = state_row["leading_object_id"]
            state = state_row["reference_state"]
            event_time = parse_timestamp(state_row["event_time"])
            previous_state, state_start, transition_count = state_tracking.get(
                object_id, (state, event_time, 0)
            )
            if state != previous_state:
                state_start = event_time
                transition_count += 1
            state_tracking[object_id] = (state, state_start, transition_count)
            state_features[state_row["event_id"]] = (
                (event_time - state_start).total_seconds() / 60,
                transition_count,
            )
    static_names = (
        ("material_class", "location_class")
        if scenario == "inventory"
        else ("machine_family", "age_years", "criticality", "site")
    )
    static_history: dict[str, dict[str, tuple[list[datetime], list[Any]]]] = defaultdict(dict)
    for object_id, item in objects.items():
        if item["type"] != leading_type:
            continue
        for name in static_names:
            values = sorted(
                (
                    (parse_timestamp(attribute["time"]), attribute["value"])
                    for attribute in item.get("attributes", [])
                    if attribute["name"] == name
                ),
                key=lambda row: row[0],
            )
            static_history[object_id][name] = (
                [row[0] for row in values],
                [row[1] for row in values],
            )
    return PredictionFeatureContext(
        scenario=scenario,
        leading_type=leading_type,
        object_types=object_types,
        event_by_id=event_by_id,
        event_time_by_id=event_time_by_id,
        lifecycle=dict(lifecycle),
        position_by_event=position_by_event,
        state_features=state_features,
        static_names=static_names,
        static_history=dict(static_history),
    )


def _decision_key(row: dict[str, str]) -> str:
    return f"{row['label_name']}\x1f{row['label'].lower()}"


def _sample_ordinals(
    path: Path, task: str | None, max_samples: int
) -> dict[str, set[int]] | None:
    """Select label-stratified, evenly spaced row ordinals without loading the sidecar."""

    counts: Counter[str] = Counter()
    with path.open(encoding="utf-8", newline="") as source:
        for row in csv.DictReader(source):
            if task is None or row["label_name"] == task:
                counts[_decision_key(row)] += 1
    total = sum(counts.values())
    if total <= max_samples:
        return None
    keys = sorted(counts)
    quotas = {key: min(counts[key], int(max_samples * counts[key] / total)) for key in keys}
    if max_samples >= len(keys):
        for key in keys:
            quotas[key] = max(1, quotas[key])
    while sum(quotas.values()) > max_samples:
        key = max((key for key in keys if quotas[key] > 1), key=lambda item: quotas[item])
        quotas[key] -= 1
    while sum(quotas.values()) < max_samples:
        key = max(keys, key=lambda item: counts[item] - quotas[item])
        quotas[key] += 1
    return {
        key: {
            min(counts[key] - 1, ((2 * index + 1) * counts[key]) // (2 * quota))
            for index in range(quota)
        }
        for key, quota in quotas.items()
    }


def build_prediction_features(
    run_dir: Path,
    window_events: int = 8,
    *,
    task: str | None = None,
    document: dict[str, Any] | None = None,
    context: PredictionFeatureContext | None = None,
    max_samples: int | None = None,
) -> pd.DataFrame:
    """Build leakage-safe features, optionally using a deterministic bounded sample."""

    if window_events < 1:
        raise ValueError("feature window must contain at least one event")
    if max_samples is not None and max_samples < 1:
        raise ValueError("prediction sample cap must be positive")
    context = context or prepare_prediction_context(run_dir, document)
    scenario = context.scenario
    leading_type = context.leading_type
    object_types = context.object_types
    event_by_id = context.event_by_id
    event_time_by_id = context.event_time_by_id
    lifecycle = context.lifecycle
    position_by_event = context.position_by_event
    state_features = context.state_features
    static_names = context.static_names
    static_history = context.static_history
    rows: list[dict[str, Any]] = []
    decision_path = run_dir / "truth" / "prediction_samples.csv"
    selected = (
        _sample_ordinals(decision_path, task, max_samples) if max_samples is not None else None
    )
    seen: Counter[str] = Counter()
    with decision_path.open(encoding="utf-8", newline="") as source:
        for decision in csv.DictReader(source):
            if task is not None and decision["label_name"] != task:
                continue
            if selected is not None:
                key = _decision_key(decision)
                ordinal = seen[key]
                seen[key] += 1
                if ordinal not in selected[key]:
                    continue
            event = event_by_id.get(decision["cutoff_event_id"])
            if event is None:
                continue
            object_id = decision["leading_object_id"]
            cutoff = parse_timestamp(decision["cutoff_time"])
            position = position_by_event[event["id"]]
            window = lifecycle[object_id][max(0, position - window_events + 1) : position + 1]
            event_attributes = _attributes(event["attributes"])
            feature_start = event_time_by_id[window[0]["id"]] if window else cutoff
            row: dict[str, Any] = {
                "object_id": object_id,
                "cutoff_event_id": decision["cutoff_event_id"],
                "feature_start": feature_start,
                "cutoff_time": cutoff,
                "task": decision["label_name"],
                "horizon_minutes": float(decision["horizon_minutes"]),
                "label": decision["label"].lower() == "true",
                "time_to_event_minutes": (
                    float(decision["time_to_event_minutes"])
                    if decision["time_to_event_minutes"]
                    else float("nan")
                ),
                "split_group": decision["split_group"],
            }
            for name in static_names:
                times, values = static_history[object_id].get(name, ([], []))
                value_index = bisect_right(times, cutoff) - 1
                row[f"static.{name}"] = values[value_index] if value_index >= 0 else None
            for name in INVENTORY_RAW if scenario == "inventory" else MANUFACTURING_RAW:
                row[f"raw.{name}"] = event_attributes.get(name)
            for activity, count in Counter(item["type"] for item in window).items():
                row[f"process.activity::{activity}"] = count
            row["state.current"] = decision["current_state"]
            dwell, transitions = state_features.get(event["id"], (0.0, 0))
            row["state.dwell_minutes"] = dwell
            row["state.transition_count"] = transitions
            context_counts = Counter(
                object_types[relationship["objectId"]]
                for item in window
                for relationship in item["relationships"]
                if object_types[relationship["objectId"]] != leading_type
            )
            for object_type, count in context_counts.items():
                row[f"context.object_type::{object_type}"] = count
            rows.append(row)
    frame = pd.DataFrame(rows)
    feature_columns = [
        column
        for column in frame.columns
        if column.startswith(("static.", "raw.", "process.", "state.", "context."))
    ]
    frame[feature_columns] = frame[feature_columns].fillna(0)
    return frame
