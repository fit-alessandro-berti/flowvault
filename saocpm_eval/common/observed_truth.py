"""Align event-indexed reference truth with the emitted, perturbed OCEL."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable, Mapping
from typing import Any

from saocpm_eval.common.ocel_builder import parse_timestamp


def observed_event_index(document: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    """Return emitted events by ID, rejecting duplicates defensively."""

    result: dict[str, dict[str, Any]] = {}
    for event in document["events"]:
        identifier = str(event["id"])
        if identifier in result:
            raise ValueError(f"duplicate observed event ID {identifier!r}")
        result[identifier] = event
    return result


def align_event_rows(
    rows: Iterable[Mapping[str, Any]],
    events: Mapping[str, Mapping[str, Any]],
    *,
    retain_unobserved: bool = False,
) -> list[dict[str, Any]]:
    """Copy rows and replace event times with their emitted OCEL timestamps."""

    result: list[dict[str, Any]] = []
    for source in rows:
        event = events.get(str(source["event_id"]))
        if event is None:
            if retain_unobserved:
                result.append(dict(source))
            continue
        row = dict(source)
        row["event_time"] = event["time"]
        result.append(row)
    return result


def align_state_truth(
    state_rows: Iterable[Mapping[str, Any]],
    transition_rows: Iterable[Mapping[str, Any]],
    document: Mapping[str, Any],
    *,
    unknown_reason: str,
    observed_transition_prefix: str,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Make observable state truth exact after deletion, jitter, and MCAR masking.

    Latent/physical state remains available in its dedicated sidecars. The reference
    state used to score the state query follows the emitted ``data_complete`` flag,
    and transitions are reconstructed over the emitted lifecycle.
    """

    events = observed_event_index(document)
    original_transition_ids = {
        str(row["event_id"]): str(row["transition_id"]) for row in transition_rows
    }
    aligned: list[dict[str, Any]] = []
    for source in state_rows:
        event_id = str(source["event_id"])
        event = events.get(event_id)
        if event is None:
            continue
        attributes = {
            str(attribute["name"]): attribute["value"] for attribute in event.get("attributes", [])
        }
        complete = bool(attributes.get("data_complete", True))
        row = dict(source)
        row["event_time"] = event["time"]
        row["data_complete"] = complete
        if not complete:
            row["reference_state"] = "Unknown"
            row["state_reason"] = unknown_reason
        aligned.append(row)

    aligned.sort(
        key=lambda row: (
            str(row["leading_object_id"]),
            parse_timestamp(str(row["event_time"])),
            str(row["event_id"]),
        )
    )
    by_object: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in aligned:
        by_object[str(row["leading_object_id"])].append(row)

    transitions: list[dict[str, Any]] = []
    for object_id in sorted(by_object):
        previous_state: str | None = None
        state_started_at = None
        for row in by_object[object_id]:
            state = str(row["reference_state"])
            event_id = str(row["event_id"])
            event_time = parse_timestamp(str(row["event_time"]))
            changed = previous_state is not None and state != previous_state
            transition_id = ""
            if previous_state is None:
                state_started_at = event_time
            elif changed:
                if state_started_at is None:
                    raise AssertionError("state start must exist after the first observation")
                transition_id = original_transition_ids.get(
                    event_id, f"{observed_transition_prefix}{event_id}"
                )
                transitions.append(
                    {
                        "transition_id": transition_id,
                        "leading_object_id": object_id,
                        "event_id": event_id,
                        "event_time": row["event_time"],
                        "from_state": previous_state,
                        "to_state": state,
                        "from_state_started_at": state_started_at,
                        "duration_minutes": (event_time - state_started_at).total_seconds() / 60.0,
                    }
                )
                state_started_at = event_time
            row["state_before"] = previous_state or ""
            row["state_after"] = state
            row["is_transition"] = changed
            row["transition_id"] = transition_id
            previous_state = state
    return aligned, transitions
