"""Typed inventory generator configuration."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, ValidationError, model_validator

from saocpm_eval.config import load_yaml


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)


class RangeModel(StrictModel):
    pass


class InventoryEntities(StrictModel):
    materials: int = Field(gt=0)
    locations: int = Field(gt=0)
    item_locations: int = Field(gt=0)
    suppliers: int = Field(gt=0)
    planners: int = Field(gt=0)


class InventoryStock(StrictModel):
    initial_cover_days: tuple[float, float]
    allow_negative_on_hand: bool
    final_snapshot: bool
    numeric_tolerance: float = Field(gt=0)


class InventoryPolicy(StrictModel):
    review_interval_days: tuple[int, int]
    service_level_range: tuple[float, float]
    safety_stock_cover_days: tuple[float, float]
    order_up_to_cover_days: tuple[float, float]
    minimum_order_quantity: tuple[int, int]
    lot_size: tuple[int, int]
    planner_approval_delay_hours: tuple[float, float]
    critical_demand_horizon_days: int = Field(gt=0)
    critical_action_sla_hours: float = Field(gt=0)


class InventoryDemand(StrictModel):
    class_mix: dict[str, float]
    base_daily_rate: tuple[float, float]
    order_quantity_mean: tuple[float, float]
    backorder_cancel_probability: float = Field(ge=0, le=1)

    @model_validator(mode="after")
    def validate_mix(self) -> InventoryDemand:
        if abs(sum(self.class_mix.values()) - 1.0) > 1e-9:
            raise ValueError("demand.class_mix probabilities must sum to 1")
        if any(value < 0 for value in self.class_mix.values()):
            raise ValueError("demand.class_mix probabilities must be non-negative")
        return self


class InventorySupply(StrictModel):
    lead_time_days: tuple[float, float]
    lead_time_cv: tuple[float, float]
    fill_rate: tuple[float, float]
    rejection_probability: float = Field(ge=0, le=1)
    expedite_lead_time_multiplier: float = Field(gt=0)


class InventoryPatterns(StrictModel):
    noise_event_probability: float = Field(ge=0, le=1)
    exact_behavior_instances_per_pattern: int = Field(gt=0)


class InventoryMissingness(StrictModel):
    event_attribute_mcar: float = Field(ge=0, le=1)
    relationship_mcar: float = Field(ge=0, le=1)
    timestamp_jitter_minutes: float = Field(ge=0)


class InventoryStateDetection(StrictModel):
    window_sizes: tuple[int, ...]
    som_grids: tuple[tuple[int, int], ...]
    epochs: int = Field(gt=0)
    max_training_windows: int | None = Field(default=None, gt=0)


class InventoryConfig(StrictModel):
    scenario: Literal["inventory"]
    profile: Literal["golden", "smoke", "paper", "scale"]
    seed: int = Field(ge=0)
    start_time: datetime
    horizon_days: int = Field(gt=0)
    entities: InventoryEntities
    stock: InventoryStock
    policy: InventoryPolicy
    demand: InventoryDemand
    supply: InventorySupply
    forced_episodes: dict[str, int]
    patterns: InventoryPatterns
    missingness: InventoryMissingness
    state_detection: InventoryStateDetection

    @model_validator(mode="after")
    def validate_semantics(self) -> InventoryConfig:
        if self.start_time.tzinfo is None or self.start_time.utcoffset() is None:
            raise ValueError("start_time must include a UTC offset")
        if self.entities.item_locations > self.entities.materials * self.entities.locations:
            raise ValueError("item_locations exceeds distinct material-location combinations")
        ranges: tuple[tuple[float | int, float | int], ...] = (
            self.stock.initial_cover_days,
            self.policy.review_interval_days,
            self.policy.service_level_range,
            self.policy.safety_stock_cover_days,
            self.policy.order_up_to_cover_days,
            self.policy.minimum_order_quantity,
            self.policy.lot_size,
            self.policy.planner_approval_delay_hours,
            self.demand.base_daily_rate,
            self.demand.order_quantity_mean,
            self.supply.lead_time_days,
            self.supply.lead_time_cv,
            self.supply.fill_rate,
        )
        if any(low > high for low, high in ranges):
            raise ValueError("configuration ranges must be ordered [minimum, maximum]")
        if any(value < 0 for value in self.forced_episodes.values()):
            raise ValueError("forced episode counts must be non-negative")
        if not self.state_detection.window_sizes or any(
            size < 2 for size in self.state_detection.window_sizes
        ):
            raise ValueError("state-detection window sizes must be at least two")
        return self


def load_inventory_config(path: Path) -> InventoryConfig:
    try:
        return InventoryConfig.model_validate(load_yaml(path))
    except ValidationError as exc:
        raise ValueError(f"invalid inventory configuration {path}: {exc}") from exc
