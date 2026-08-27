"""Independent physical and semantic validation for inventory OCEL documents."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from saocpm_eval.common.ocel_builder import OcelBuilder


@dataclass(frozen=True, slots=True)
class ConservationError:
    event_id: str
    object_id: str
    expected_on_hand_after: float
    actual_on_hand_after: float
    difference: float


def _event_attributes(event: dict[str, Any]) -> dict[str, Any]:
    return {attribute["name"]: attribute["value"] for attribute in event["attributes"]}


def stock_conservation_errors(
    document: dict[str, Any], tolerance: float = 1e-9
) -> list[ConservationError]:
    """Recompute the stock equation using only observed event facts."""

    if tolerance <= 0:
        raise ValueError("stock-conservation tolerance must be positive")
    builder = OcelBuilder.from_dict(document, leading_object_type="ItemLocation")
    errors: list[ConservationError] = []
    positive = {"Goods Receipt", "Transfer Receive"}
    negative = {"Goods Issue", "Transfer Ship"}
    for event in builder.to_dict()["events"]:
        if event["type"] == "Initialize Inventory":
            continue
        attributes = _event_attributes(event)
        if not bool(attributes.get("data_complete", True)):
            # Observation noise may remove one or more stock facts. Such rows
            # are intentionally classified as Unknown and cannot support an
            # observed-fact conservation equation.
            continue
        before = float(attributes["on_hand_before"])
        after = float(attributes["on_hand_after"])
        quantity = float(attributes["quantity"])
        delta = 0.0
        if event["type"] in positive:
            delta = quantity
        elif event["type"] in negative:
            delta = -quantity
        elif event["type"] == "Inventory Adjustment":
            delta = quantity
        expected = before + delta
        difference = after - expected
        if abs(difference) <= tolerance:
            continue
        leading = [
            relationship["objectId"]
            for relationship in event["relationships"]
            if builder.objects[relationship["objectId"]].type == "ItemLocation"
        ]
        errors.append(
            ConservationError(
                event_id=str(event["id"]),
                object_id=leading[0] if leading else "",
                expected_on_hand_after=expected,
                actual_on_hand_after=after,
                difference=difference,
            )
        )
    return errors


def validate_inventory_document(document: dict[str, Any], tolerance: float = 1e-9) -> None:
    errors = stock_conservation_errors(document, tolerance)
    if errors:
        first = errors[0]
        raise ValueError(
            f"stock conservation failed at {first.event_id}: expected "
            f"{first.expected_on_hand_after}, found {first.actual_on_hand_after}"
        )
