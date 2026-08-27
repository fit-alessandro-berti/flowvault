from copy import deepcopy

import numpy as np

from saocpm_eval.common.ocel_builder import OcelBuilder, parse_timestamp
from saocpm_eval.common.perturbations import (
    delete_context_relationships,
    jitter_event_timestamps,
    mask_event_attributes,
)
from tests.unit.test_ocel_builder import valid_document


def test_relationship_deletion_preserves_leading_relationship() -> None:
    document = valid_document()
    document["events"][0]["relationships"].append(  # type: ignore[index]
        {"objectId": "MAT-001", "qualifier": "material"}
    )
    perturbed = delete_context_relationships(
        document,
        1.0,
        np.random.default_rng(1),
        leading_object_type="ItemLocation",
    )
    assert perturbed["events"][0]["relationships"] == [  # type: ignore[index]
        {"objectId": "IL-001", "qualifier": "inventory perspective"}
    ]


def test_attribute_masking_marks_event_incomplete() -> None:
    perturbed = mask_event_attributes(valid_document(), 1.0, np.random.default_rng(1))
    attributes = {item["name"]: item["value"] for item in perturbed["events"][0]["attributes"]}  # type: ignore[index]
    assert attributes == {"data_complete": False}


def test_jitter_is_deterministic_and_keeps_lifecycle_strict() -> None:
    document = valid_document()
    later = deepcopy(document["events"][0])  # type: ignore[index]
    later["id"] = "E-002"
    later["time"] = "2025-01-01T00:00:01Z"
    document["events"].append(later)  # type: ignore[union-attr]
    first = jitter_event_timestamps(
        document, 60, np.random.default_rng(4), leading_object_type="ItemLocation"
    )
    second = jitter_event_timestamps(
        document, 60, np.random.default_rng(4), leading_object_type="ItemLocation"
    )
    assert first == second
    OcelBuilder.from_dict(first, leading_object_type="ItemLocation")
    times = [parse_timestamp(event["time"]) for event in first["events"]]
    assert times[0] < times[1]


def test_jitter_resolves_subsecond_values_that_collapse_during_serialization() -> None:
    document = valid_document()
    document["events"][0]["time"] = "2025-01-01T00:00:00.100Z"  # type: ignore[index]
    later = deepcopy(document["events"][0])  # type: ignore[index]
    later["id"] = "E-002"
    later["time"] = "2025-01-01T00:00:00.900Z"
    document["events"].append(later)  # type: ignore[union-attr]

    perturbed = jitter_event_timestamps(
        document, 0, np.random.default_rng(1), leading_object_type="ItemLocation"
    )

    OcelBuilder.from_dict(perturbed, leading_object_type="ItemLocation")
    assert [event["time"] for event in perturbed["events"]] == [
        "2025-01-01T00:00:00Z",
        "2025-01-01T00:00:01Z",
    ]
