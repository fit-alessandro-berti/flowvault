"""Streaming minimal OCEL generation for geometric performance profiles."""

from __future__ import annotations

import json
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any, TextIO

from saocpm_eval.common.hashing import canonical_json_bytes
from saocpm_eval.config import ScaleProfile


def _event_attributes(scenario: str, index: int) -> list[dict[str, Any]]:
    phase = index % 20
    if scenario == "inventory":
        on_hand = float((phase * 7) % 101)
        return [
            {"name": "data_complete", "value": phase != 19},
            {"name": "critical_understock", "value": phase in {0, 1}},
            {"name": "on_hand_after", "value": on_hand},
            {"name": "lower_threshold", "value": 20.0},
            {"name": "upper_threshold", "value": 80.0},
            {"name": "passive_observation", "value": False},
        ]
    mode = "DOWN" if phase in {0, 1} else "SETUP" if phase == 2 else "RUNNING"
    return [
        {"name": "data_complete", "value": phase != 19},
        {"name": "down_active", "value": mode == "DOWN"},
        {"name": "mode", "value": mode},
        {"name": "quality_hold_active", "value": phase == 3},
        {"name": "recovery_active", "value": phase in {4, 5}},
        {"name": "degraded_latched", "value": phase in {6, 7, 8}},
        {"name": "passive_observation", "value": False},
    ]


def _event_declaration(scenario: str) -> dict[str, Any]:
    attributes = (
        {
            "data_complete": "boolean",
            "critical_understock": "boolean",
            "on_hand_after": "float",
            "lower_threshold": "float",
            "upper_threshold": "float",
            "passive_observation": "boolean",
        }
        if scenario == "inventory"
        else {
            "data_complete": "boolean",
            "down_active": "boolean",
            "mode": "string",
            "quality_hold_active": "boolean",
            "recovery_active": "boolean",
            "degraded_latched": "boolean",
            "passive_observation": "boolean",
        }
    )
    return {
        "name": "Scale Observation",
        "attributes": [{"name": name, "type": kind} for name, kind in attributes.items()],
    }


def _write_array(source: TextIO, values: list[dict[str, Any]]) -> None:
    for index, value in enumerate(values):
        if index:
            source.write(",")
        source.write(json.dumps(value, sort_keys=True, separators=(",", ":")))


def generate_scale_fixture(profile: ScaleProfile, output_dir: Path) -> None:
    """Write an exact-size, importable OCEL while keeping peak generator memory bounded."""

    output_dir.mkdir(parents=True, exist_ok=True)
    document_path = output_dir / "observed.ocel.json"
    leading_type = "ItemLocation" if profile.scenario == "inventory" else "Machine"
    prefix = "IL" if profile.scenario == "inventory" else "M"
    start = datetime(2025, 1, 1, tzinfo=UTC)
    object_width = max(4, len(str(profile.leading_objects)))

    with document_path.open("w", encoding="utf-8", newline="\n") as target:
        target.write('{"eventTypes":[')
        _write_array(target, [_event_declaration(profile.scenario)])
        target.write('],"events":[')
        for index in range(profile.target_events):
            if index:
                target.write(",")
            object_index = index % profile.leading_objects
            lifecycle_index = index // profile.leading_objects
            object_id = f"{prefix}-{object_index + 1:0{object_width}d}"
            event = {
                "attributes": _event_attributes(profile.scenario, index),
                "id": f"S-E-{index + 1:09d}",
                "relationships": [{"objectId": object_id, "qualifier": "scale perspective"}],
                "time": (start + timedelta(seconds=lifecycle_index))
                .isoformat(timespec="seconds")
                .replace("+00:00", "Z"),
                "type": "Scale Observation",
            }
            target.write(json.dumps(event, sort_keys=True, separators=(",", ":")))
        target.write('],"objectTypes":[')
        _write_array(
            target,
            [
                {
                    "name": leading_type,
                    "attributes": [{"name": "scale_feature", "type": "float"}],
                }
            ],
        )
        target.write('],"objects":[')
        for index in range(profile.leading_objects):
            if index:
                target.write(",")
            value = {
                "attributes": [
                    {
                        "name": "scale_feature",
                        "time": "2025-01-01T00:00:00Z",
                        "value": float(index % 17),
                    }
                ],
                "id": f"{prefix}-{index + 1:0{object_width}d}",
                "type": leading_type,
            }
            target.write(json.dumps(value, sort_keys=True, separators=(",", ":")))
        target.write("]}\n")

    query_name = (
        "inventory_state.sql" if profile.scenario == "inventory" else "manufacturing_state.sql"
    )
    repository_root = Path(__file__).parents[1]
    (output_dir / "state_query.sql").write_bytes(
        (repository_root / "queries" / query_name).read_bytes()
    )
    (output_dir / "scale_profile.json").write_bytes(
        canonical_json_bytes(profile.model_dump(mode="json"), pretty=True)
    )
