"""Stochastic manufacturing smoke and paper profile simulation."""

from __future__ import annotations

import math
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, ClassVar

from saocpm_eval.common.ocel_builder import Relationship, format_timestamp
from saocpm_eval.common.perturbations import (
    delete_context_relationships,
    delete_events,
    jitter_event_timestamps,
    mask_event_attributes,
)
from saocpm_eval.manufacturing.config import ManufacturingConfig
from saocpm_eval.manufacturing.simulation import (
    MANUFACTURING_EVENT_TYPES,
    ManufacturingGoldenSimulation,
)


class ManufacturingStochasticSimulation(ManufacturingGoldenSimulation):
    PATTERN_TO_FORCED: ClassVar[dict[str, str]] = {
        "MFG-P1": "bearing_degradation_to_down",
        "MFG-P2": "thermal_quality_drift",
        "MFG-P3": "quick_recovery",
        "MFG-P4": "slow_recovery",
        "MFG-P5": "recurrent_degradation",
        "MFG-P6": "unsafe_restart",
    }

    def __init__(self, config: ManufacturingConfig) -> None:
        if config.profile not in {"smoke", "paper"}:
            raise ValueError("stochastic manufacturing requires smoke or paper profile")
        super().__init__(config)
        self.forced_cursor = self.at(5)
        self.forced_rng = self.seed_tree.stream("forced mechanisms")

    def _slot(self) -> datetime:
        result = self.forced_cursor
        self.forced_cursor += timedelta(hours=4)
        if self.forced_cursor >= self.end - timedelta(days=2):
            raise ValueError("configured manufacturing forced episodes exceed the horizon")
        return result

    def _machine(self, instance: int, offset: int = 0) -> str:
        index = (instance + offset - 1) % self.config.entities.machines + 1
        return f"M-{index:03d}"

    def _flags(self, instance: int) -> tuple[bool, float]:
        guaranteed = instance <= self.config.patterns.exact_behavior_instances_per_pattern
        noisy = not guaranteed and (
            float(self.forced_rng.random()) < self.config.patterns.noise_event_probability
        )
        return noisy, 1.0 if noisy else 0.0

    def _reset(self, machine: str, time: datetime, *, down: bool = False) -> None:
        self.emit(
            machine,
            "Repair Performed",
            time,
            updates={
                "mode": "DOWN" if down else "RUNNING",
                "health_index": 0.9,
                "vibration_rms": 2.2,
                "temperature_c": 60.0,
                "power_kw": 0.0 if down else 80.0,
                "alarm_severity": "none",
                "degraded_latched": False,
                "down_active": down,
                "recovery_active": False,
                "quality_hold_active": False,
                "maintenance_open": down,
                "stable_run_minutes": 0,
                "data_complete": True,
                "fault_family_observed": "none",
            },
            regime="Active Repair" if down else "Healthy Steady Run",
        )

    def _noise(self, machine: str, time: datetime, enabled: bool, regime: str) -> None:
        if enabled:
            self.emit(
                machine,
                "Alarm Acknowledged",
                time,
                regime=regime,
            )

    def _inject_p1(self, instance: int) -> None:
        base = self._slot()
        machine = self._machine(instance)
        key = f"MFG-P1#{instance:03d}"
        noisy, noise_level = self._flags(instance)
        self._reset(machine, base)
        self.telemetry(
            machine,
            base + timedelta(seconds=10),
            health=0.62,
            vibration=8.3,
            temperature=70.0,
            power=106.0,
            regime="Bearing Degradation",
            true_fault="bearing",
            pattern_key=key,
        )
        events = (
            ("Warning Alarm Raised", timedelta(seconds=20)),
            ("Maintenance Request Created", timedelta(seconds=30)),
            ("Critical Alarm Raised", timedelta(seconds=40)),
            ("Automatic Stop", timedelta(seconds=50)),
        )
        for index, (event_type, offset) in enumerate(events):
            if index == 2:
                self._noise(machine, base + timedelta(seconds=35), noisy, "Alarm Escalation")
            updates: dict[str, str | int | float | bool] = {}
            if event_type == "Warning Alarm Raised":
                updates = {"alarm_severity": "warning", "fault_family_observed": "bearing"}
            elif event_type == "Maintenance Request Created":
                updates = {"maintenance_open": True}
            elif event_type == "Critical Alarm Raised":
                updates = {"alarm_severity": "critical", "degraded_latched": True}
            elif event_type == "Automatic Stop":
                updates = {"mode": "DOWN", "down_active": True, "power_kw": 0.0}
            self.emit(
                machine,
                event_type,
                base + offset,
                updates=updates,
                regime=("Failed" if event_type == "Automatic Stop" else "Alarm Escalation"),
                true_fault="bearing",
                pattern_key=key,
            )
        self.add_pattern(
            "MFG-P1",
            machine,
            family="inter",
            from_state="Running",
            to_state="Down",
            sequence=(
                "Sensor Snapshot",
                "Warning Alarm Raised",
                "Maintenance Request Created",
                "Critical Alarm Raised",
                "Automatic Stop",
            ),
            object_types=("Machine", "Alarm", "MaintenanceTeam", "Component"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_p2(self, instance: int) -> None:
        base = self._slot()
        machine = self._machine(instance, 8)
        key = f"MFG-P2#{instance:03d}"
        noisy, noise_level = self._flags(instance)
        self._reset(machine, base)
        self.telemetry(
            machine,
            base + timedelta(seconds=10),
            health=0.64,
            vibration=4.0,
            temperature=88.0,
            power=112.0,
            regime="Thermal Drift",
            true_fault="thermal",
            pattern_key=key,
        )
        self.emit(
            machine,
            "Defect Detected",
            base + timedelta(seconds=20),
            regime="Quality Drift",
            true_fault="thermal",
            pattern_key=key,
        )
        self.emit(
            machine,
            "Quality Hold Started",
            base + timedelta(seconds=30),
            updates={"quality_hold_active": True},
            regime="Quality Drift",
            true_fault="thermal",
            pattern_key=key,
        )
        self._noise(machine, base + timedelta(seconds=35), noisy, "Quality Drift")
        inspection = self._inspection(machine, base + timedelta(seconds=40), "passed", "quality")
        self.emit(
            machine,
            "Inspection Performed",
            base + timedelta(seconds=40),
            context=[(inspection, "inspection")],
            regime="Quality Drift",
            pattern_key=key,
        )
        self.emit(
            machine,
            "Calibration Performed",
            base + timedelta(seconds=50),
            updates={"temperature_c": 70.0},
            context=[(inspection, "inspection")],
            regime="Active Repair",
            pattern_key=key,
        )
        self.add_pattern(
            "MFG-P2",
            machine,
            family="intra",
            from_state="Degraded",
            to_state="Quality Hold",
            sequence=(
                "Sensor Snapshot",
                "Defect Detected",
                "Quality Hold Started",
                "Inspection Performed",
                "Calibration Performed",
            ),
            object_types=("Machine", "Inspection", "Component"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_recovery(self, instance: int, *, slow: bool) -> None:
        pattern = "MFG-P4" if slow else "MFG-P3"
        base = self._slot()
        machine = self._machine(instance, 16 if slow else 12)
        key = f"{pattern}#{instance:03d}"
        noisy, noise_level = self._flags(instance)
        self._reset(machine, base, down=True)
        work_order = self._work_order(machine, base + timedelta(seconds=1), "bearing")
        context = [(work_order, "work order"), ("TEAM-001", "maintenance team")]
        names = (
            (
                "Maintenance Started",
                "Diagnosis Performed",
                "Part Unavailable",
                "Component Replaced",
                "Test Failed",
                "Repair Performed",
                "Test Run Completed",
                "Machine Restarted",
            )
            if slow
            else (
                "Maintenance Started",
                "Diagnosis Performed",
                "Component Replaced",
                "Inspection Performed",
                "Test Run Completed",
                "Machine Restarted",
            )
        )
        spacing = timedelta(minutes=20 if slow else 5)
        for index, event_type in enumerate(names, start=1):
            if index == 3:
                self._noise(
                    machine,
                    base + spacing * index - timedelta(seconds=1),
                    noisy,
                    "Active Repair",
                )
            updates: dict[str, str | int | float | bool] = {}
            if event_type == "Component Replaced":
                self.physics[machine].replace_component("bearing")
                updates = {"health_index": 0.9, "vibration_rms": 2.2}
            elif event_type == "Machine Restarted":
                updates = {
                    "mode": "RUNNING",
                    "down_active": False,
                    "recovery_active": True,
                    "stable_run_minutes": 0,
                }
            self.emit(
                machine,
                event_type,
                base + spacing * index,
                updates=updates,
                context=context,
                regime=(
                    "Post-Repair Recovery"
                    if event_type in {"Test Run Completed", "Machine Restarted"}
                    else "Active Repair"
                ),
                pattern_key=key,
            )
        self.add_pattern(
            pattern,
            machine,
            family="inter",
            from_state="Down",
            to_state="Recovery",
            sequence=names,
            object_types=("Machine", "WorkOrder", "Component", "Inspection", "MaintenanceTeam"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_p5(self, instance: int) -> None:
        base = self._slot()
        machine = self._machine(instance, 24)
        key = f"MFG-P5#{instance:03d}"
        noisy, noise_level = self._flags(instance)
        self._reset(machine, base, down=True)
        self.emit(
            machine,
            "Machine Restarted",
            base + timedelta(seconds=10),
            updates={"mode": "RUNNING", "down_active": False, "recovery_active": True},
            regime="Post-Repair Recovery",
            pattern_key=key,
        )
        self.emit(
            machine,
            "Operation Completed",
            base + timedelta(minutes=30),
            updates={"recovery_active": False, "stable_run_minutes": 30},
            regime="Healthy Steady Run",
            pattern_key=key,
        )
        self._noise(machine, base + timedelta(minutes=45), noisy, "Healthy Steady Run")
        self.emit(
            machine,
            "Warning Alarm Raised",
            base + timedelta(hours=1),
            updates={"degraded_latched": True, "health_index": 0.63, "vibration_rms": 8.0},
            regime="Bearing Degradation",
            true_fault="bearing",
            pattern_key=key,
        )
        self.add_pattern(
            "MFG-P5",
            machine,
            family="inter",
            from_state="Recovery",
            to_state="Degraded",
            sequence=("Machine Restarted", "Operation Completed", "Warning Alarm Raised"),
            object_types=("Machine", "WorkOrder", "Component"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_p6(self, instance: int) -> None:
        base = self._slot()
        machine = self._machine(instance, 30)
        key = f"MFG-P6#{instance:03d}"
        noisy, noise_level = self._flags(instance)
        self._reset(machine, base)
        self.emit(
            machine,
            "Critical Alarm Raised",
            base + timedelta(seconds=10),
            updates={"alarm_severity": "critical", "degraded_latched": True},
            regime="Alarm Escalation",
            true_fault="bearing",
            pattern_key=key,
        )
        self._noise(machine, base + timedelta(seconds=15), noisy, "Alarm Escalation")
        self.emit(
            machine,
            "Automatic Stop",
            base + timedelta(seconds=20),
            updates={"mode": "DOWN", "down_active": True},
            regime="Failed",
            true_fault="bearing",
            pattern_key=key,
        )
        self.emit(
            machine,
            "Machine Restarted",
            base + timedelta(minutes=20),
            updates={"mode": "RUNNING", "down_active": False, "recovery_active": True},
            regime="Post-Repair Recovery",
            pattern_key=key,
        )
        self.add_pattern(
            "MFG-P6",
            machine,
            family="inter",
            from_state="Degraded",
            to_state="Recovery",
            sequence=("Critical Alarm Raised", "Automatic Stop", "Machine Restarted"),
            object_types=("Machine", "Alarm", "Component"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_forced_support(self) -> None:
        injectors = {
            "MFG-P1": self._inject_p1,
            "MFG-P2": self._inject_p2,
            "MFG-P3": lambda instance: self._inject_recovery(instance, slow=False),
            "MFG-P4": lambda instance: self._inject_recovery(instance, slow=True),
            "MFG-P5": self._inject_p5,
            "MFG-P6": self._inject_p6,
        }
        for pattern, forced_name in self.PATTERN_TO_FORCED.items():
            for instance in range(2, self.config.forced_episodes.get(forced_name, 0) + 1):
                injectors[pattern](instance)
        for instance in range(2, self.config.forced_episodes.get("data_gap", 0) + 1):
            base = self._slot()
            machine = self._machine(instance, 36)
            self.emit(
                machine,
                "Sensor Snapshot",
                base + timedelta(seconds=10),
                updates={"data_complete": False},
                regime="Data Gap",
                passive=True,
            )
            self.emit(
                machine,
                "Sensor Snapshot",
                base + timedelta(hours=1),
                updates={"data_complete": True},
                regime="Healthy Steady Run",
                passive=True,
            )

    def _background_machine(self, machine: str, start: datetime, index: int) -> None:
        schedule_rng = self.seed_tree.stream("exogenous demand or production schedule")
        sensor_rng = self.seed_tree.stream("sensor or stock noise")
        response_rng = self.seed_tree.stream("supplier or maintenance response")
        utilization = float(schedule_rng.uniform(*self.config.production.utilization_range))
        cadence = timedelta(minutes=self.config.telemetry_interval_minutes)
        time = start
        last_day = -1
        while time < self.end - cadence:
            state = self.states[machine]
            day = int((time - self.start).total_seconds() // 86400)
            if day != last_day:
                last_day = day
                if float(schedule_rng.random()) < min(
                    1.0, self.config.production.production_orders_per_machine_day[1]
                ):
                    order_id = self.new_context(
                        "ProductionOrder",
                        time,
                        {
                            "product_family": f"PRODUCT-{day % 4 + 1}",
                            "priority": "standard",
                            "quantity": int(schedule_rng.integers(10, 101)),
                            "due_time": format_timestamp(time + timedelta(days=1)),
                        },
                        [Relationship(machine, "assigned-to")],
                    )
                    self.emit(
                        machine,
                        "Production Order Released",
                        time,
                        context=[(order_id, "production order")],
                        regime="Healthy Steady Run",
                    )
                    if float(schedule_rng.random()) < self.config.production.setup_probability:
                        self.emit(
                            machine,
                            "Setup Started",
                            time + timedelta(seconds=1),
                            updates={"mode": "SETUP"},
                            context=[(order_id, "production order")],
                            regime="Setup or Changeover",
                        )
                        self.emit(
                            machine,
                            "Setup Completed",
                            time + timedelta(seconds=2),
                            updates={"mode": "RUNNING"},
                            context=[(order_id, "production order")],
                            regime="Healthy Steady Run",
                        )
            if state.down_active:
                if state.maintenance_open and float(response_rng.random()) < 0.05:
                    self.physics[machine].repair(
                        float(response_rng.uniform(*self.config.maintenance.repair_effectiveness))
                    )
                    health = self.physics[machine].health_index()
                    self.emit(
                        machine,
                        "Repair Performed",
                        time + timedelta(seconds=3),
                        updates={"health_index": health, "vibration_rms": 2.5},
                        regime="Active Repair",
                    )
                    self.emit(
                        machine,
                        "Machine Restarted",
                        time + timedelta(seconds=4),
                        updates={
                            "mode": "RUNNING",
                            "down_active": False,
                            "recovery_active": True,
                            "stable_run_minutes": 0,
                        },
                        regime="Post-Repair Recovery",
                    )
                time += cadence
                continue
            load = utilization * float(schedule_rng.uniform(0.75, 1.15))
            self.physics[machine].advance(
                sensor_rng,
                load_fraction=min(load, 1.2),
                hours=self.config.telemetry_interval_minutes / 60,
            )
            vibration, temperature, power = self.physics[machine].sensors(
                sensor_rng,
                load_fraction=load,
                noise_multiplier=self.config.health.sensor_noise_multiplier,
            )
            health = self.physics[machine].health_index()
            regime = "Healthy Steady Run"
            fault = "none"
            if self.physics[machine].bearing_wear > 0.35:
                regime = "Bearing Degradation"
                fault = "bearing"
            elif self.physics[machine].thermal_wear > 0.35:
                regime = "Thermal Drift"
                fault = "thermal"
            self.states[machine].load_fraction = load
            self.telemetry(
                machine,
                time + timedelta(seconds=10),
                health=health,
                vibration=vibration,
                temperature=temperature,
                power=power,
                regime=regime,
                true_fault=fault,
            )
            state = self.states[machine]
            if state.recovery_active:
                state.stable_run_minutes += self.config.telemetry_interval_minutes
                if state.stable_run_minutes >= self.config.maintenance.recovery_stable_minutes:
                    self.emit(
                        machine,
                        "Operation Completed",
                        time + timedelta(seconds=11),
                        updates={
                            "recovery_active": False,
                            "degraded_latched": False,
                            "maintenance_open": False,
                        },
                        regime="Healthy Steady Run",
                    )
            if state.degraded_latched and state.alarm_severity == "none":
                self.emit(
                    machine,
                    "Warning Alarm Raised",
                    time + timedelta(seconds=12),
                    updates={"alarm_severity": "warning", "fault_family_observed": fault},
                    regime="Alarm Escalation",
                    true_fault=fault,
                )
                if (
                    float(response_rng.random())
                    < self.config.maintenance.warning_request_probability
                ):
                    self.emit(
                        machine,
                        "Maintenance Request Created",
                        time + timedelta(seconds=13),
                        updates={"maintenance_open": True},
                        regime="Waiting for Maintenance",
                        true_fault=fault,
                    )
            hazard_logit = -8.0 + 10.0 * (1.0 - health) + 1.5 * load
            hazard = 1.0 / (1.0 + math.exp(-hazard_logit))
            hazard *= self.config.health.failure_hazard_scale
            if float(response_rng.random()) < min(hazard, 0.25):
                self.emit(
                    machine,
                    "Critical Alarm Raised",
                    time + timedelta(seconds=14),
                    updates={"alarm_severity": "critical", "degraded_latched": True},
                    regime="Alarm Escalation",
                    true_fault=fault,
                )
                self.emit(
                    machine,
                    "Automatic Stop",
                    time + timedelta(seconds=15),
                    updates={
                        "mode": "DOWN",
                        "down_active": True,
                        "maintenance_open": True,
                        "power_kw": 0.0,
                    },
                    regime="Failed",
                    true_fault=fault,
                )
            time += cadence

    def _background(self) -> None:
        start = self.forced_cursor + timedelta(days=1)
        for index, machine in enumerate(sorted(self.states)):
            self._background_machine(machine, start, index)

    def simulate(self) -> None:
        self._initialize()
        self._script_m1_bearing_and_quick_recovery()
        self._script_m2_slow_recovery()
        self._script_m3_recurrence()
        self._script_m4_unsafe_restart()
        self._script_m5_completion_violation()
        self._script_m6_quality()
        self._script_m7_component_violation()
        self._script_m8_data_gap()
        self._inject_forced_support()
        self._background()
        self._finalize()
        self.builder.validate()

    def observed_document(self) -> dict[str, Any]:
        document = super().observed_document()
        missingness = self.config.missingness
        rng = self.seed_tree.stream("missingness and corruption")
        if missingness.telemetry_mcar:
            document = mask_event_attributes(
                document,
                missingness.telemetry_mcar,
                rng,
                event_types=frozenset({"Sensor Snapshot"}),
            )
        if missingness.process_event_mcar:
            eligible = frozenset(MANUFACTURING_EVENT_TYPES).difference(
                {"Initialize Machine", "Simulation End Snapshot", "Sensor Snapshot"}
            )
            document = delete_events(
                document,
                missingness.process_event_mcar,
                rng,
                eligible_event_types=eligible,
                leading_object_type="Machine",
            )
        if missingness.relationship_mcar:
            document = delete_context_relationships(
                document,
                missingness.relationship_mcar,
                rng,
                leading_object_type="Machine",
            )
        if missingness.timestamp_jitter_minutes:
            document = jitter_event_timestamps(
                document,
                missingness.timestamp_jitter_minutes,
                self.seed_tree.stream("timestamp jitter"),
                leading_object_type="Machine",
            )
        return document

    def perturbation_manifest(self) -> list[dict[str, object]]:
        missingness = self.config.missingness
        return [
            {
                "id": "profile-observation-noise",
                "telemetry_mcar": missingness.telemetry_mcar,
                "process_event_mcar": missingness.process_event_mcar,
                "relationship_mcar": missingness.relationship_mcar,
                "timestamp_jitter_minutes": missingness.timestamp_jitter_minutes,
            }
        ]


def generate_manufacturing_stochastic(
    config: ManufacturingConfig, config_path: Path, output_dir: Path
) -> None:
    simulation = ManufacturingStochasticSimulation(config)
    simulation.simulate()
    simulation.write(output_dir, config_path)
