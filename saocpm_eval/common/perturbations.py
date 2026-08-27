"""Controlled OCEL perturbations used by robustness and stochastic profiles."""

from __future__ import annotations

from copy import deepcopy
from datetime import timedelta
from typing import Any

from numpy.random import Generator

from saocpm_eval.common.ocel_builder import OcelBuilder, format_timestamp, parse_timestamp


def delete_context_relationships(
    document: dict[str, Any],
    probability: float,
    rng: Generator,
    *,
    leading_object_type: str,
) -> dict[str, Any]:
    """Delete E2O context links while preserving exactly one leading-object link."""

    if not 0 <= probability <= 1:
        raise ValueError("relationship deletion probability must be in [0, 1]")
    result = deepcopy(document)
    object_types = {item["id"]: item["type"] for item in result["objects"]}
    for event in result["events"]:
        retained = []
        for relationship in event.get("relationships", []):
            is_leading = object_types[relationship["objectId"]] == leading_object_type
            if is_leading or float(rng.random()) >= probability:
                retained.append(relationship)
        event["relationships"] = retained
    OcelBuilder.from_dict(result, leading_object_type=leading_object_type)
    return result


def mask_event_attributes(
    document: dict[str, Any],
    probability: float,
    rng: Generator,
    *,
    protected: frozenset[str] = frozenset({"data_complete", "passive_observation"}),
    event_types: frozenset[str] | None = None,
) -> dict[str, Any]:
    """Apply MCAR attribute masking and mark affected snapshots incomplete."""

    if not 0 <= probability <= 1:
        raise ValueError("attribute missingness probability must be in [0, 1]")
    result = deepcopy(document)
    for event in result["events"]:
        if event_types is not None and event["type"] not in event_types:
            continue
        attributes = event.get("attributes", [])
        kept = []
        removed = False
        for attribute in attributes:
            if attribute["name"] in protected or float(rng.random()) >= probability:
                kept.append(attribute)
            else:
                removed = True
        if removed:
            complete = next((item for item in kept if item["name"] == "data_complete"), None)
            if complete is not None:
                complete["value"] = False
        event["attributes"] = kept
    return result


def delete_events(
    document: dict[str, Any],
    probability: float,
    rng: Generator,
    *,
    eligible_event_types: frozenset[str],
    leading_object_type: str,
) -> dict[str, Any]:
    """Delete eligible process events without removing lifecycle boundary observations."""

    if not 0 <= probability <= 1:
        raise ValueError("event deletion probability must be in [0, 1]")
    result = deepcopy(document)
    result["events"] = [
        event
        for event in result["events"]
        if event["type"] not in eligible_event_types or float(rng.random()) >= probability
    ]
    OcelBuilder.from_dict(result, leading_object_type=leading_object_type)
    return result


def jitter_event_timestamps(
    document: dict[str, Any],
    maximum_minutes: float,
    rng: Generator,
    *,
    leading_object_type: str,
) -> dict[str, Any]:
    """Jitter event times and deterministically retain strict lifecycle ordering."""

    if maximum_minutes < 0:
        raise ValueError("maximum timestamp jitter must be non-negative")
    result = deepcopy(document)
    object_types = {item["id"]: item["type"] for item in result["objects"]}
    by_leading: dict[str, list[dict[str, Any]]] = {}
    context_only: list[dict[str, Any]] = []
    for event in result["events"]:
        leading = [
            relationship["objectId"]
            for relationship in event.get("relationships", [])
            if object_types[relationship["objectId"]] == leading_object_type
        ]
        (by_leading.setdefault(leading[0], []) if leading else context_only).append(event)
    for events in (*by_leading.values(), context_only):
        events.sort(key=lambda event: (parse_timestamp(event["time"]), event["id"]))
        previous = None
        for event in events:
            original = parse_timestamp(event["time"])
            offset = float(rng.uniform(-maximum_minutes, maximum_minutes))
            # OCEL output is canonicalized to whole seconds. Compare on that
            # same grid so two distinct sub-second draws cannot collapse to an
            # invalid equal lifecycle timestamp during serialization.
            jittered = parse_timestamp(format_timestamp(original + timedelta(minutes=offset)))
            if previous is not None and jittered <= previous:
                jittered = previous + timedelta(seconds=1)
            event["time"] = format_timestamp(jittered)
            previous = parse_timestamp(event["time"])
    result["events"].sort(key=lambda event: (parse_timestamp(event["time"]), event["id"]))
    OcelBuilder.from_dict(result, leading_object_type=leading_object_type)
    return result
