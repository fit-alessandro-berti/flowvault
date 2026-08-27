"""Hidden component degradation and observed sensor equations."""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np
from numpy.random import Generator


@dataclass(slots=True)
class MachinePhysics:
    bearing_wear: float = 0.05
    thermal_wear: float = 0.05
    calibration_drift: float = 0.02
    ambient_c: float = 22.0
    base_wear_rate: float = 0.002
    _weights: tuple[float, float, float] = field(default=(0.5, 0.35, 0.15), repr=False)

    def health_index(self) -> float:
        total = sum(
            weight * wear
            for weight, wear in zip(
                self._weights,
                (self.bearing_wear, self.thermal_wear, self.calibration_drift),
                strict=True,
            )
        )
        return float(np.clip(1.0 - total, 0.0, 1.0))

    def advance(
        self,
        rng: Generator,
        *,
        load_fraction: float,
        hours: float,
        material_factor: float = 1.0,
        shock: float = 0.0,
    ) -> None:
        if not 0 <= load_fraction <= 1.5 or hours < 0 or material_factor <= 0:
            raise ValueError("invalid physical-update parameter")
        common = self.base_wear_rate * load_fraction**1.4 * hours * material_factor
        self.bearing_wear = float(
            np.clip(self.bearing_wear + common + shock + rng.normal(0, 0.0005), 0, 1)
        )
        self.thermal_wear = float(
            np.clip(self.thermal_wear + common * 0.7 + rng.normal(0, 0.0004), 0, 1)
        )
        self.calibration_drift = float(
            np.clip(self.calibration_drift + common * 0.2 + rng.normal(0, 0.0002), 0, 1)
        )

    def sensors(
        self,
        rng: Generator,
        *,
        load_fraction: float,
        noise_multiplier: float,
    ) -> tuple[float, float, float]:
        vibration = (
            1.8
            + 8.5 * self.bearing_wear
            + 0.9 * load_fraction
            + rng.normal(0, 0.12 * noise_multiplier)
        )
        temperature = (
            self.ambient_c
            + 24.0
            + 38.0 * self.thermal_wear
            + 15.0 * load_fraction
            + 12.0 * self.calibration_drift
            + rng.normal(0, 0.8 * noise_multiplier)
        )
        power = 100.0 * load_fraction * (
            1.0 + 0.35 * (self.bearing_wear + self.thermal_wear)
        ) + rng.normal(0, 1.0 * noise_multiplier)
        return max(0.0, float(vibration)), float(temperature), max(0.0, float(power))

    def replace_component(self, component_family: str) -> None:
        if component_family == "bearing":
            self.bearing_wear = 0.02
        elif component_family == "thermal":
            self.thermal_wear = 0.02
        elif component_family == "calibration":
            self.calibration_drift = 0.0
        else:
            raise ValueError(f"unknown component family {component_family!r}")

    def repair(self, effectiveness: float) -> None:
        if not 0 <= effectiveness <= 1:
            raise ValueError("repair effectiveness must be in [0, 1]")
        remaining = 1.0 - effectiveness
        self.bearing_wear *= remaining
        self.thermal_wear *= remaining
        self.calibration_drift *= remaining
