from collections import Counter
from copy import deepcopy
from datetime import timedelta
from typing import Any, cast

from saocpm_eval.analytics.robustness import (
    _deserialize_baseline,
    _serialize_baseline,
    _shift_event_types,
)
from saocpm_eval.common.ocel_builder import OcelBuilder, parse_timestamp
from tests.unit.test_ocel_builder import valid_document


def test_shifted_event_collision_is_resolved_without_losing_requested_delay() -> None:
    document = cast(dict[str, Any], valid_document())
    document["eventTypes"][0]["name"] = "Alarm Raised"
    document["events"][0]["type"] = "Alarm Raised"
    document["eventTypes"].append(
        {
            "name": "Machine Restarted",
            "attributes": deepcopy(document["eventTypes"][0]["attributes"]),
        }
    )
    restart = deepcopy(document["events"][0])
    restart.update(
        {
            "id": "E-002",
            "type": "Machine Restarted",
            "time": "2025-01-01T00:30:00Z",
        }
    )
    document["events"].append(restart)

    _shift_event_types(
        document,
        frozenset({"Alarm Raised"}),
        timedelta(minutes=30),
        "ItemLocation",
    )

    OcelBuilder.from_dict(document, leading_object_type="ItemLocation")
    times = {event["id"]: parse_timestamp(event["time"]) for event in document["events"]}
    assert (times["E-001"] - parse_timestamp("2025-01-01T00:00:00Z")) == timedelta(minutes=30)
    assert times["E-002"] == times["E-001"] + timedelta(seconds=1)


def test_robustness_baseline_cache_round_trips_counters() -> None:
    baseline = {
        "state_coverage": 1.0,
        "prediction_auprc": None,
        "edges": Counter({("A", "B", "Running"): 4}),
        "patterns": Counter({"pattern": 3}),
    }
    assert _deserialize_baseline(_serialize_baseline(baseline)) == baseline
