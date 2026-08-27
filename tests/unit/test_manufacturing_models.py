import numpy as np
import pytest

from saocpm_eval.manufacturing.config import load_manufacturing_config
from saocpm_eval.manufacturing.entities import DegradationHysteresis, MachineState
from saocpm_eval.manufacturing.physics import MachinePhysics
from saocpm_eval.manufacturing.state_reference import reference_state


def test_physics_health_bounds_sensors_and_replacement(tmp_path: object) -> None:
    del tmp_path
    physics = MachinePhysics()
    rng = np.random.default_rng(42)
    for _ in range(500):
        physics.advance(rng, load_fraction=0.9, hours=1)
        assert 0 <= physics.health_index() <= 1
        vibration, temperature, power = physics.sensors(
            rng, load_fraction=0.9, noise_multiplier=1.0
        )
        assert vibration >= 0
        assert temperature > 0
        assert power >= 0
    previous = physics.bearing_wear
    physics.replace_component("bearing")
    assert physics.bearing_wear < previous
    assert physics.bearing_wear == pytest.approx(0.02)


def test_degradation_hysteresis_requires_entry_and_exit_samples() -> None:
    from pathlib import Path

    config = load_manufacturing_config(Path("configs/manufacturing_golden.yaml"))
    tracker = DegradationHysteresis(config.health)
    assert tracker.update(health_index=0.9, vibration_rms=8, temperature_c=60) is False
    assert tracker.update(health_index=0.9, vibration_rms=8, temperature_c=60) is True
    for _ in range(config.health.degraded_exit_samples - 1):
        assert tracker.update(health_index=0.9, vibration_rms=2, temperature_c=60) is True
    assert tracker.update(health_index=0.9, vibration_rms=2, temperature_c=60) is False


@pytest.mark.parametrize(
    ("updates", "expected"),
    [
        ({"data_complete": False}, "Unknown"),
        ({"down_active": True}, "Down"),
        ({"quality_hold_active": True}, "Quality Hold"),
        ({"recovery_active": True}, "Recovery"),
        ({"mode": "SETUP"}, "Setup"),
        ({"degraded_latched": True}, "Degraded"),
        ({"mode": "RUNNING"}, "Running"),
        ({"mode": "IDLE"}, "Idle"),
    ],
)
def test_manufacturing_reference_state_precedence(
    updates: dict[str, object], expected: str
) -> None:
    state = MachineState()
    for name, value in updates.items():
        setattr(state, name, value)
    assert reference_state(state).name == expected
