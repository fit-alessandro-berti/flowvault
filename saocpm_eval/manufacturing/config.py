"""Typed manufacturing scenario configuration."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, ValidationError, model_validator

from saocpm_eval.config import load_yaml


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)


class ManufacturingEntities(StrictModel):
    machines: int = Field(gt=0)
    machine_families: int = Field(gt=0)
    operators: int = Field(gt=0)
    maintenance_teams: int = Field(gt=0)
    component_families: int = Field(gt=0)
    material_lot_suppliers: int = Field(gt=0)


class ProductionConfig(StrictModel):
    utilization_range: tuple[float, float]
    production_orders_per_machine_day: tuple[float, float]
    setup_probability: float = Field(ge=0, le=1)
    operation_duration_hours: tuple[float, float]


class HealthConfig(StrictModel):
    initial_health: tuple[float, float]
    degraded_entry_health: float = Field(ge=0, le=1)
    degraded_exit_health: float = Field(ge=0, le=1)
    degraded_entry_samples: int = Field(gt=0)
    degraded_exit_samples: int = Field(gt=0)
    failure_hazard_scale: float = Field(gt=0)
    sensor_noise_multiplier: float = Field(ge=0)


class MaintenanceConfig(StrictModel):
    warning_request_probability: float = Field(ge=0, le=1)
    critical_request_sla_minutes: int = Field(gt=0)
    down_start_sla_minutes_by_criticality: dict[str, int]
    dispatch_delay_minutes: tuple[float, float]
    part_unavailable_probability: float = Field(ge=0, le=1)
    repair_effectiveness: tuple[float, float]
    recovery_stable_minutes: int = Field(gt=0)
    recurrence_horizon_hours: int = Field(gt=0)


class PatternConfig(StrictModel):
    noise_event_probability: float = Field(ge=0, le=1)
    exact_behavior_instances_per_pattern: int = Field(gt=0)


class MissingnessConfig(StrictModel):
    telemetry_mcar: float = Field(ge=0, le=1)
    process_event_mcar: float = Field(ge=0, le=1)
    relationship_mcar: float = Field(ge=0, le=1)
    timestamp_jitter_minutes: float = Field(ge=0)


class StateDetectionConfig(StrictModel):
    window_sizes: tuple[int, ...]
    som_grids: tuple[tuple[int, int], ...]
    epochs: int = Field(gt=0)
    max_training_windows: int | None = Field(default=None, gt=0)


class ManufacturingConfig(StrictModel):
    scenario: Literal["manufacturing"]
    profile: Literal["golden", "smoke", "paper", "scale"]
    seed: int = Field(ge=0)
    start_time: datetime
    horizon_days: int = Field(gt=0)
    telemetry_interval_minutes: int = Field(gt=0)
    entities: ManufacturingEntities
    production: ProductionConfig
    health: HealthConfig
    maintenance: MaintenanceConfig
    forced_episodes: dict[str, int]
    patterns: PatternConfig
    missingness: MissingnessConfig
    state_detection: StateDetectionConfig

    @model_validator(mode="after")
    def validate_semantics(self) -> ManufacturingConfig:
        if self.start_time.tzinfo is None or self.start_time.utcoffset() is None:
            raise ValueError("start_time must include a UTC offset")
        if self.entities.machines < 1:
            raise ValueError("at least one machine is required")
        ranges = (
            self.production.utilization_range,
            self.production.production_orders_per_machine_day,
            self.production.operation_duration_hours,
            self.health.initial_health,
            self.maintenance.dispatch_delay_minutes,
            self.maintenance.repair_effectiveness,
        )
        if any(low > high for low, high in ranges):
            raise ValueError("configuration ranges must be ordered [minimum, maximum]")
        if self.health.degraded_exit_health <= self.health.degraded_entry_health:
            raise ValueError("Degraded exit health must exceed entry health for hysteresis")
        if set(self.maintenance.down_start_sla_minutes_by_criticality) != {
            "high",
            "medium",
            "low",
        }:
            raise ValueError("Down-to-maintenance SLAs must define high, medium, and low")
        if any(value < 0 for value in self.forced_episodes.values()):
            raise ValueError("forced episode counts must be non-negative")
        if not self.state_detection.window_sizes or any(
            size < 2 for size in self.state_detection.window_sizes
        ):
            raise ValueError("state-detection window sizes must be at least two")
        return self


def load_manufacturing_config(path: Path) -> ManufacturingConfig:
    try:
        return ManufacturingConfig.model_validate(load_yaml(path))
    except ValidationError as exc:
        raise ValueError(f"invalid manufacturing configuration {path}: {exc}") from exc
