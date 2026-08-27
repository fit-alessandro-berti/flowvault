from saocpm_eval.common.observed_truth import align_state_truth


def _row(event_id: str, time: str, state: str, transition_id: str = "") -> dict[str, object]:
    changed = bool(transition_id)
    return {
        "scenario": "inventory",
        "leading_object_type": "ItemLocation",
        "leading_object_id": "IL-1",
        "event_id": event_id,
        "event_time": time,
        "reference_state": state,
        "state_reason": "complete observation",
        "policy_or_rule_version": "P1",
        "data_complete": True,
        "state_before": "Normal" if event_id != "E1" else "",
        "state_after": state,
        "is_transition": changed,
        "transition_id": transition_id,
    }


def _event(event_id: str, time: str, complete: bool) -> dict[str, object]:
    return {
        "id": event_id,
        "time": time,
        "attributes": [{"name": "data_complete", "value": complete}],
    }


def test_observable_truth_tracks_unknown_jitter_and_reconstructed_transitions() -> None:
    states = [
        _row("E1", "2025-01-01T00:00:00Z", "Normal"),
        _row("E2", "2025-01-01T00:01:00Z", "Normal"),
        _row("E3", "2025-01-01T00:02:00Z", "Understock", "INV-T-1"),
    ]
    original_transitions = [
        {
            "transition_id": "INV-T-1",
            "event_id": "E3",
        }
    ]
    document = {
        "events": [
            _event("E1", "2025-01-01T00:00:05Z", True),
            _event("E2", "2025-01-01T00:01:07Z", False),
            _event("E3", "2025-01-01T00:02:09Z", True),
        ]
    }

    aligned, transitions = align_state_truth(
        states,
        original_transitions,
        document,
        unknown_reason="incomplete",
        observed_transition_prefix="INV-OBS-T-",
    )

    assert [row["reference_state"] for row in aligned] == [
        "Normal",
        "Unknown",
        "Understock",
    ]
    assert aligned[1]["event_time"] == "2025-01-01T00:01:07Z"
    assert aligned[1]["transition_id"] == "INV-OBS-T-E2"
    assert aligned[2]["transition_id"] == "INV-T-1"
    assert [(row["from_state"], row["to_state"]) for row in transitions] == [
        ("Normal", "Unknown"),
        ("Unknown", "Understock"),
    ]


def test_observable_truth_omits_deleted_events() -> None:
    states = [
        _row("E1", "2025-01-01T00:00:00Z", "Normal"),
        _row("E2", "2025-01-01T00:01:00Z", "Normal"),
        _row("E3", "2025-01-01T00:02:00Z", "Understock", "INV-T-1"),
    ]
    document = {
        "events": [
            _event("E1", "2025-01-01T00:00:00Z", True),
            _event("E3", "2025-01-01T00:02:00Z", True),
        ]
    }

    aligned, transitions = align_state_truth(
        states,
        [{"transition_id": "INV-T-1", "event_id": "E3"}],
        document,
        unknown_reason="incomplete",
        observed_transition_prefix="INV-OBS-T-",
    )

    assert [row["event_id"] for row in aligned] == ["E1", "E3"]
    assert len(transitions) == 1
    assert transitions[0]["transition_id"] == "INV-T-1"
