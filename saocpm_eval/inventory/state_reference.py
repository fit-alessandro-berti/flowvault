"""Independent inventory reference-state implementation.

This module deliberately does not share parsing or expression code with FLOWVAULT's state
query evaluator. It operates on the clean simulator state before truth is written.
"""

from __future__ import annotations

from dataclasses import dataclass

from saocpm_eval.inventory.entities import InventoryState

INVENTORY_STATES = (
    "Unknown",
    "Critical Understock",
    "Understock",
    "Overstock",
    "Normal",
)


@dataclass(frozen=True, slots=True)
class StateReference:
    name: str
    reason: str


def reference_state(state: InventoryState) -> StateReference:
    if not state.data_complete:
        return StateReference("Unknown", "required stock or threshold data are incomplete")
    if state.critical_understock:
        return StateReference(
            "Critical Understock",
            "confirmed demand exceeds usable stock plus timely inbound",
        )
    if state.on_hand < state.lower_threshold:
        return StateReference("Understock", "post-event on-hand is below the lower threshold")
    if state.on_hand > state.upper_threshold:
        return StateReference("Overstock", "post-event on-hand is above the upper threshold")
    return StateReference("Normal", "post-event on-hand is within policy thresholds")
