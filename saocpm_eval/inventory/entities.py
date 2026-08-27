"""Inventory simulation state and snapshot semantics."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(slots=True)
class InventoryState:
    on_hand: float
    lower_threshold: float
    upper_threshold: float
    reserved: float = 0.0
    backorder: float = 0.0
    on_order: float = 0.0
    confirmed_demand_horizon: float = 0.0
    inbound_horizon: float = 0.0
    data_complete: bool = True
    demand_estimate: float = 1.0
    lead_time_estimate: float = 1.0
    policy_version: str = "P1"

    @property
    def inventory_position(self) -> float:
        return self.on_hand + self.on_order - self.backorder - self.reserved

    @property
    def critical_understock(self) -> bool:
        usable_stock = max(0.0, self.on_hand - self.reserved)
        return self.confirmed_demand_horizon > usable_stock + self.inbound_horizon

    def dynamic_attributes(self) -> dict[str, float | bool | str]:
        return {
            "on_hand": self.on_hand,
            "reserved": self.reserved,
            "backorder": self.backorder,
            "on_order": self.on_order,
            "inventory_position": self.inventory_position,
            "lower_threshold": self.lower_threshold,
            "upper_threshold": self.upper_threshold,
            "demand_estimate": self.demand_estimate,
            "lead_time_estimate": self.lead_time_estimate,
            "policy_version": self.policy_version,
            "data_complete": self.data_complete,
        }
