from copy import deepcopy
from datetime import UTC, datetime
from pathlib import Path

import pytest

from saocpm_eval.common.ocel_builder import OcelBuilder, OcelValidationError

REPOSITORY_ROOT = Path(__file__).parents[2]


def valid_document() -> dict[str, object]:
    attributes = [
        {"name": "on_hand_after", "type": "float"},
        {"name": "data_complete", "type": "boolean"},
    ]
    return {
        "objectTypes": [
            {
                "name": "ItemLocation",
                "attributes": [{"name": "on_hand", "type": "float"}],
            },
            {"name": "Material", "attributes": [{"name": "class", "type": "string"}]},
        ],
        "eventTypes": [{"name": "Initialize Inventory", "attributes": attributes}],
        "objects": [
            {
                "id": "IL-001",
                "type": "ItemLocation",
                "attributes": [{"name": "on_hand", "time": "2025-01-01T00:00:00Z", "value": 10.0}],
                "relationships": [{"objectId": "MAT-001", "qualifier": "material"}],
            },
            {
                "id": "MAT-001",
                "type": "Material",
                "attributes": [
                    {"name": "class", "time": "2025-01-01T00:00:00Z", "value": "smooth"}
                ],
            },
        ],
        "events": [
            {
                "id": "E-001",
                "type": "Initialize Inventory",
                "time": "2025-01-01T00:00:00Z",
                "attributes": [
                    {"name": "on_hand_after", "value": 10.0},
                    {"name": "data_complete", "value": True},
                ],
                "relationships": [{"objectId": "IL-001", "qualifier": "inventory perspective"}],
            }
        ],
    }


def test_valid_document_round_trips_canonically() -> None:
    builder = OcelBuilder.from_dict(valid_document(), leading_object_type="ItemLocation")
    assert builder.to_json_bytes() == builder.to_json_bytes()
    assert builder.to_dict()["events"][0]["id"] == "E-001"


def test_blueprint_example_is_accepted() -> None:
    builder = OcelBuilder.read_json(
        REPOSITORY_ROOT / "examples" / "ocel_fragment.json",
        leading_object_type="ItemLocation",
    )
    assert len(builder.objects) == 3
    assert len(builder.events) == 2


@pytest.mark.parametrize(
    ("filename", "message"),
    [
        ("duplicate_id.json", "duplicate event ID"),
        ("undeclared_attribute.json", "undeclared attribute"),
        ("invalid_value_type.json", "must be boolean"),
        ("unknown_relationship.json", "unknown object"),
        ("multiple_leading_objects.json", "multiple 'ItemLocation' objects"),
    ],
)
def test_blueprint_validation_rule_fixtures_are_rejected(filename: str, message: str) -> None:
    with pytest.raises(OcelValidationError, match=message):
        OcelBuilder.read_json(
            REPOSITORY_ROOT / "tests" / "fixtures" / "invalid_ocel" / filename,
            leading_object_type="ItemLocation",
        )


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        (lambda doc: doc["objects"].append(deepcopy(doc["objects"][0])), "duplicate object ID"),
        (lambda doc: doc["events"].append(deepcopy(doc["events"][0])), "duplicate event ID"),
        (
            lambda doc: doc["events"][0]["attributes"].append(
                {"name": "undeclared", "value": "bad"}
            ),
            "undeclared attribute",
        ),
        (
            lambda doc: doc["events"][0]["attributes"].__setitem__(
                1, {"name": "data_complete", "value": "true"}
            ),
            "must be boolean",
        ),
        (
            lambda doc: doc["events"][0]["relationships"].__setitem__(
                0, {"objectId": "UNKNOWN", "qualifier": "inventory perspective"}
            ),
            "unknown object",
        ),
        (
            lambda doc: (
                doc["objects"].append(
                    {
                        "id": "IL-002",
                        "type": "ItemLocation",
                        "attributes": [
                            {
                                "name": "on_hand",
                                "time": "2025-01-01T00:00:00Z",
                                "value": 2.0,
                            }
                        ],
                    }
                ),
                doc["events"][0]["relationships"].append(
                    {"objectId": "IL-002", "qualifier": "inventory perspective"}
                ),
            ),
            "multiple 'ItemLocation' objects",
        ),
    ],
)
def test_invalid_documents_are_rejected(mutation: object, message: str) -> None:
    document = valid_document()
    mutation(document)  # type: ignore[operator]
    with pytest.raises(OcelValidationError, match=message):
        OcelBuilder.from_dict(document, leading_object_type="ItemLocation")


def test_events_are_sorted_chronologically() -> None:
    document = valid_document()
    later = deepcopy(document["events"][0])  # type: ignore[index]
    later["id"] = "E-002"
    later["time"] = "2025-01-02T00:00:00+00:00"
    document["events"].insert(0, later)  # type: ignore[union-attr]
    builder = OcelBuilder.from_dict(document, leading_object_type="ItemLocation")
    assert [event["id"] for event in builder.to_dict()["events"]] == ["E-001", "E-002"]


def test_same_leading_object_timestamp_is_rejected() -> None:
    document = valid_document()
    duplicate_time = deepcopy(document["events"][0])  # type: ignore[index]
    duplicate_time["id"] = "E-002"
    document["events"].append(duplicate_time)  # type: ignore[union-attr]
    with pytest.raises(OcelValidationError, match="non-increasing"):
        OcelBuilder.from_dict(document, leading_object_type="ItemLocation")


def test_timestamp_is_normalized_to_utc() -> None:
    document = valid_document()
    document["events"][0]["time"] = "2025-01-01T01:00:00+01:00"  # type: ignore[index]
    builder = OcelBuilder.from_dict(document, leading_object_type="ItemLocation")
    assert builder.to_dict()["events"][0]["time"] == "2025-01-01T00:00:00Z"


def test_naive_timestamp_is_rejected() -> None:
    document = valid_document()
    document["events"][0]["time"] = (
        datetime(2025, 1, 1, tzinfo=UTC)
        .replace(  # type: ignore[index]
            tzinfo=None
        )
        .isoformat()
    )
    with pytest.raises(OcelValidationError, match="no UTC offset"):
        OcelBuilder.from_dict(document, leading_object_type="ItemLocation")
