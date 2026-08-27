"""Typed configuration loading shared by all evaluation commands."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Any, Literal

import yaml
from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator

Scenario = Literal["inventory", "manufacturing"]


class ConfigEnvelope(BaseModel):
    """Fields required in every simulation configuration.

    Scenario modules validate their complete configuration after this common envelope is
    accepted. Extra fields are intentionally retained so that configuration hashing covers
    the exact document supplied to the generator.
    """

    model_config = ConfigDict(extra="allow", frozen=True)

    scenario: Scenario
    profile: str = Field(min_length=1)
    seed: int = Field(ge=0)
    start_time: datetime
    horizon_days: int = Field(gt=0)

    @field_validator("start_time")
    @classmethod
    def require_timezone(cls, value: datetime) -> datetime:
        if value.tzinfo is None or value.utcoffset() is None:
            raise ValueError("start_time must include a UTC offset")
        return value


class ScaleProfile(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)

    id: str = Field(min_length=1)
    scenario: Scenario
    target_events: int = Field(gt=0)
    leading_objects: int = Field(gt=0)


class ScaleMatrix(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)

    profiles: tuple[ScaleProfile, ...]
    repetitions: int = Field(gt=0)
    warmup_repetitions: int = Field(ge=0)
    operations: tuple[str, ...]
    record: tuple[str, ...]


def load_yaml(path: Path) -> dict[str, Any]:
    """Read a YAML mapping, producing a stable and actionable validation error."""

    try:
        raw = yaml.safe_load(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ValueError(f"cannot read configuration {path}: {exc}") from exc
    except yaml.YAMLError as exc:
        raise ValueError(f"invalid YAML in {path}: {exc}") from exc
    if not isinstance(raw, dict):
        raise ValueError(f"configuration {path} must contain a YAML mapping")
    return raw


def load_config(path: Path, expected_scenario: Scenario | None = None) -> ConfigEnvelope:
    """Load and validate the common configuration envelope."""

    try:
        config = ConfigEnvelope.model_validate(load_yaml(path))
    except ValidationError as exc:
        raise ValueError(f"invalid configuration {path}: {exc}") from exc
    if expected_scenario is not None and config.scenario != expected_scenario:
        raise ValueError(
            f"configuration scenario is {config.scenario!r}, expected {expected_scenario!r}"
        )
    return config


def load_scale_matrix(path: Path) -> ScaleMatrix:
    """Load the typed performance benchmark matrix."""

    try:
        return ScaleMatrix.model_validate(load_yaml(path))
    except ValidationError as exc:
        raise ValueError(f"invalid scale matrix {path}: {exc}") from exc
