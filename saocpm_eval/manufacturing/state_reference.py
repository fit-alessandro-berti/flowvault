"""Independent manufacturing manual-state oracle."""

from __future__ import annotations

from dataclasses import dataclass

from saocpm_eval.manufacturing.entities import MachineState

MANUFACTURING_STATES = (
    "Unknown",
    "Down",
    "Quality Hold",
    "Recovery",
    "Setup",
    "Degraded",
    "Running",
    "Idle",
)


@dataclass(frozen=True, slots=True)
class StateReference:
    name: str
    reason: str


def reference_state(state: MachineState) -> StateReference:
    if not state.data_complete:
        return StateReference("Unknown", "telemetry or mode data are incomplete")
    if state.down_active or state.mode == "DOWN":
        return StateReference("Down", "failure stop or DOWN mode is active")
    if state.quality_hold_active:
        return StateReference("Quality Hold", "quality hold is active")
    if state.recovery_active:
        return StateReference("Recovery", "restart validation has not reached stable-run duration")
    if state.mode == "SETUP":
        return StateReference("Setup", "machine mode is SETUP")
    if state.degraded_latched:
        return StateReference("Degraded", "observed degradation hysteresis is latched")
    if state.mode == "RUNNING":
        return StateReference("Running", "machine mode is RUNNING")
    return StateReference("Idle", "machine is available and not processing")
