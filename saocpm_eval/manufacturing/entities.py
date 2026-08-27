"""Observed manufacturing state and preprocessing state machines."""

from __future__ import annotations

from dataclasses import dataclass

from saocpm_eval.manufacturing.config import HealthConfig


@dataclass(slots=True)
class DegradationHysteresis:
    config: HealthConfig
    latched: bool = False
    high_sensor_samples: int = 0
    healthy_samples: int = 0

    def update(
        self,
        *,
        health_index: float,
        vibration_rms: float,
        temperature_c: float,
        critical_alarm: bool = False,
    ) -> bool:
        high_sensor = vibration_rms >= 7.0 or temperature_c >= 85.0
        below_exit = vibration_rms < 5.0 and temperature_c < 75.0
        self.high_sensor_samples = self.high_sensor_samples + 1 if high_sensor else 0
        if (
            health_index <= self.config.degraded_entry_health
            or critical_alarm
            or self.high_sensor_samples >= self.config.degraded_entry_samples
        ):
            self.latched = True
            self.healthy_samples = 0
            return self.latched
        healthy = health_index >= self.config.degraded_exit_health and below_exit
        self.healthy_samples = self.healthy_samples + 1 if healthy else 0
        if self.latched and self.healthy_samples >= self.config.degraded_exit_samples:
            self.latched = False
            self.high_sensor_samples = 0
        return self.latched


@dataclass(slots=True)
class MachineState:
    mode: str = "RUNNING"
    health_index: float = 0.95
    vibration_rms: float = 2.0
    temperature_c: float = 55.0
    power_kw: float = 80.0
    load_fraction: float = 0.7
    alarm_severity: str = "none"
    degraded_latched: bool = False
    down_active: bool = False
    recovery_active: bool = False
    quality_hold_active: bool = False
    maintenance_open: bool = False
    stable_run_minutes: int = 0
    data_complete: bool = True
    fault_family_observed: str = "none"

    def dynamic_attributes(self) -> dict[str, str | int | float | bool]:
        return {
            "mode": self.mode,
            "health_index": self.health_index,
            "vibration_rms": self.vibration_rms,
            "temperature_c": self.temperature_c,
            "power_kw": self.power_kw,
            "load_fraction": self.load_fraction,
            "alarm_severity": self.alarm_severity,
            "degraded_latched": self.degraded_latched,
            "down_active": self.down_active,
            "recovery_active": self.recovery_active,
            "quality_hold_active": self.quality_hold_active,
            "maintenance_open": self.maintenance_open,
            "stable_run_minutes": self.stable_run_minutes,
            "data_complete": self.data_complete,
            "fault_family_observed": self.fault_family_observed,
        }
