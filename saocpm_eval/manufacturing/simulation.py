"""Manufacturing golden simulation, workflow truth, and OCEL output."""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

from saocpm_eval.analyst_tasks import generate_analyst_tasks
from saocpm_eval.causal_truth import manufacturing_causal_truth
from saocpm_eval.common.hashing import canonical_json_bytes, config_sha256, sha256_bytes
from saocpm_eval.common.ids import DeterministicIds
from saocpm_eval.common.observed_truth import (
    align_event_rows,
    align_state_truth,
    observed_event_index,
)
from saocpm_eval.common.ocel_builder import (
    EventAttribute,
    ObjectAttribute,
    OcelBuilder,
    OcelEvent,
    OcelObject,
    OcelType,
    Relationship,
    format_timestamp,
    parse_timestamp,
)
from saocpm_eval.common.rng import SeedTree
from saocpm_eval.common.truth_writer import RunWriter, repository_commit
from saocpm_eval.config import load_yaml
from saocpm_eval.manufacturing.config import ManufacturingConfig
from saocpm_eval.manufacturing.entities import DegradationHysteresis, MachineState
from saocpm_eval.manufacturing.physics import MachinePhysics
from saocpm_eval.manufacturing.state_reference import MANUFACTURING_STATES, reference_state

MANUFACTURING_EVENT_ATTRIBUTES: dict[str, OcelType] = {
    "mode": "string",
    "health_index": "float",
    "vibration_rms": "float",
    "temperature_c": "float",
    "power_kw": "float",
    "load_fraction": "float",
    "alarm_severity": "string",
    "degraded_latched": "boolean",
    "down_active": "boolean",
    "recovery_active": "boolean",
    "quality_hold_active": "boolean",
    "maintenance_open": "boolean",
    "stable_run_minutes": "integer",
    "data_complete": "boolean",
    "passive_observation": "boolean",
    "fault_family_observed": "string",
}

MANUFACTURING_EVENT_TYPES = (
    "Initialize Machine",
    "Production Order Released",
    "Setup Started",
    "Setup Completed",
    "Operation Started",
    "Operation Completed",
    "Sensor Snapshot",
    "Warning Alarm Raised",
    "Critical Alarm Raised",
    "Alarm Acknowledged",
    "Defect Detected",
    "Quality Hold Started",
    "Quality Hold Released",
    "Automatic Stop",
    "Maintenance Request Created",
    "Work Order Created",
    "Maintenance Team Dispatched",
    "Maintenance Started",
    "Diagnosis Performed",
    "Part Unavailable",
    "Component Replaced",
    "Repair Performed",
    "Calibration Performed",
    "Inspection Performed",
    "Test Run Started",
    "Test Run Completed",
    "Test Failed",
    "Machine Restarted",
    "Maintenance Completed",
    "Simulation End Snapshot",
)

STATE_FIELDS = (
    "scenario",
    "leading_object_type",
    "leading_object_id",
    "event_id",
    "event_time",
    "reference_state",
    "state_reason",
    "policy_or_rule_version",
    "data_complete",
    "state_before",
    "state_after",
    "is_transition",
    "transition_id",
)

LATENT_FIELDS = (
    "leading_object_id",
    "event_id",
    "event_time",
    "primary_regime",
    "regime_factors_json",
    "regime_started_at",
    "transition_window",
)

PATTERN_FIELDS = (
    "pattern_id",
    "instance_id",
    "family",
    "leading_object_id",
    "start_event_id",
    "end_event_id",
    "from_state",
    "to_state",
    "expected_sequence_json",
    "expected_object_types_json",
    "noise_level",
    "should_be_exact_in_behavior_log",
)

VIOLATION_FIELDS = (
    "rule_id",
    "violation_id",
    "leading_object_id",
    "event_id",
    "event_time",
    "related_object_id",
    "expected_deadline",
    "details_json",
)


@dataclass(frozen=True, slots=True)
class MachineMetadata:
    family: str
    criticality: str
    site: str


def _attributes(
    time: datetime, values: Mapping[str, str | int | float | bool]
) -> list[ObjectAttribute]:
    return [ObjectAttribute.create(name, time, value) for name, value in values.items()]


class ManufacturingGoldenSimulation:
    def __init__(self, config: ManufacturingConfig) -> None:
        if config.entities.machines < 8:
            raise ValueError("manufacturing simulation requires at least eight machines")
        self.config = config
        self.start = config.start_time.astimezone(UTC)
        self.end = self.start + timedelta(days=config.horizon_days)
        self.builder = OcelBuilder(leading_object_type="Machine")
        self.seed_tree = SeedTree(config.seed)
        self.event_ids = DeterministicIds("MFG-E-", 6)
        self.transition_ids = DeterministicIds("MFG-T-", 5)
        self.object_ids = {
            name: DeterministicIds(prefix, 4)
            for name, prefix in {
                "ProductionOrder": "PROD-",
                "Operation": "OP-",
                "WorkOrder": "WO-",
                "MaterialLot": "LOT-",
                "Alarm": "ALM-",
                "Inspection": "INSP-",
            }.items()
        }
        self.states: dict[str, MachineState] = {}
        self.physics: dict[str, MachinePhysics] = {}
        self.hysteresis: dict[str, DegradationHysteresis] = {}
        self.metadata: dict[str, MachineMetadata] = {}
        self.components: dict[str, str] = {}
        self.last_dynamic: dict[str, dict[str, str | int | float | bool]] = {}
        self.previous_reference: dict[str, str | None] = {}
        self.state_started_at: dict[str, datetime] = {}
        self.previous_regime: dict[str, str | None] = {}
        self.regime_started_at: dict[str, datetime] = {}
        self.state_rows: list[dict[str, Any]] = []
        self.transition_rows: list[dict[str, Any]] = []
        self.latent_rows: list[dict[str, Any]] = []
        self.physical_rows: list[dict[str, Any]] = []
        self.noisy_operational_rows: list[dict[str, Any]] = []
        self.pattern_rows: list[dict[str, Any]] = []
        self.violation_rows: list[dict[str, Any]] = []
        self.pattern_events: dict[str, list[str]] = defaultdict(list)
        self._declare_types()
        self._create_core_objects()

    def at(self, day: float, *, seconds: int = 0) -> datetime:
        return self.start + timedelta(days=day, seconds=seconds)

    def _declare_types(self) -> None:
        self.builder.declare_object_type(
            "Machine",
            {
                "machine_family": "string",
                "age_years": "float",
                "criticality": "string",
                "site": "string",
                **MANUFACTURING_EVENT_ATTRIBUTES,
            },
        )
        declarations: dict[str, dict[str, OcelType]] = {
            "ProductionOrder": {
                "product_family": "string",
                "priority": "string",
                "quantity": "integer",
                "due_time": "time",
            },
            "Operation": {
                "operation_type": "string",
                "planned_duration_minutes": "integer",
                "actual_duration_minutes": "integer",
            },
            "WorkOrder": {
                "priority": "string",
                "fault_family": "string",
                "creation_time": "time",
                "completion_time": "time",
                "status": "string",
            },
            "Component": {
                "component_family": "string",
                "age_hours": "float",
                "expected_life_hours": "float",
                "replacement_cost": "float",
            },
            "MaterialLot": {
                "material_grade": "string",
                "quality_score": "float",
                "supplier": "string",
            },
            "Alarm": {
                "category": "string",
                "severity": "string",
                "threshold": "float",
                "acknowledgement_status": "string",
            },
            "Inspection": {
                "inspection_type": "string",
                "result": "string",
                "measured_value": "float",
            },
            "Operator": {"skill_band": "string", "shift": "string", "team": "string"},
            "MaintenanceTeam": {
                "response_time_profile": "string",
                "skill_mix": "string",
                "workload": "string",
            },
            "Shift": {"shift_name": "string", "staffing_band": "string"},
        }
        for name, attributes in declarations.items():
            self.builder.declare_object_type(name, attributes)
        for name in MANUFACTURING_EVENT_TYPES:
            self.builder.declare_event_type(name, MANUFACTURING_EVENT_ATTRIBUTES)

    def _create_core_objects(self) -> None:
        rng = self.seed_tree.stream("entity parameters")
        for index in range(self.config.entities.operators):
            self.builder.add_object(
                OcelObject(
                    id=f"OPER-{index + 1:03d}",
                    type="Operator",
                    attributes=_attributes(
                        self.start,
                        {
                            "skill_band": ("senior", "mid", "junior")[index % 3],
                            "shift": ("day", "evening", "night")[index % 3],
                            "team": f"PROD-{index % 3 + 1}",
                        },
                    ),
                )
            )
        for index in range(self.config.entities.maintenance_teams):
            self.builder.add_object(
                OcelObject(
                    id=f"TEAM-{index + 1:03d}",
                    type="MaintenanceTeam",
                    attributes=_attributes(
                        self.start,
                        {
                            "response_time_profile": ("fast", "standard", "slow")[index % 3],
                            "skill_mix": ("mechanical", "mixed", "electrical")[index % 3],
                            "workload": ("low", "medium", "high")[index % 3],
                        },
                    ),
                )
            )
        for index, name in enumerate(("day", "evening", "night")):
            self.builder.add_object(
                OcelObject(
                    id=f"SHIFT-{index + 1}",
                    type="Shift",
                    attributes=_attributes(
                        self.start,
                        {"shift_name": name, "staffing_band": ("full", "reduced", "lean")[index]},
                    ),
                )
            )
        initial_modes = (
            "RUNNING",
            "RUNNING",
            "RUNNING",
            "RUNNING",
            "RUNNING",
            "RUNNING",
            "SETUP",
            "IDLE",
        )
        for index in range(self.config.entities.machines):
            machine_id = f"M-{index + 1:03d}"
            mode = initial_modes[index] if index < len(initial_modes) else "RUNNING"
            state = MachineState(
                mode=mode,
                load_fraction=0.0 if mode == "IDLE" else 0.7,
                power_kw=0.0 if mode == "IDLE" else 80.0,
            )
            family = f"FAMILY-{index % self.config.entities.machine_families + 1}"
            criticality = ("high", "medium", "low")[index % 3]
            site = ("SITE-A", "SITE-B")[index % 2]
            self.states[machine_id] = state
            self.physics[machine_id] = MachinePhysics()
            self.hysteresis[machine_id] = DegradationHysteresis(self.config.health)
            self.metadata[machine_id] = MachineMetadata(family, criticality, site)
            self.previous_reference[machine_id] = None
            self.previous_regime[machine_id] = None
            dynamic = state.dynamic_attributes()
            self.last_dynamic[machine_id] = dict(dynamic)
            self.builder.add_object(
                OcelObject(
                    id=machine_id,
                    type="Machine",
                    attributes=_attributes(
                        self.start,
                        {
                            "machine_family": family,
                            "age_years": round(float(rng.uniform(1, 15)), 3),
                            "criticality": criticality,
                            "site": site,
                            "passive_observation": False,
                            **dynamic,
                        },
                    ),
                )
            )
            component_id = f"COMP-{index + 1:04d}"
            self.components[machine_id] = component_id
            self.builder.add_object(
                OcelObject(
                    id=component_id,
                    type="Component",
                    attributes=_attributes(
                        self.start,
                        {
                            "component_family": "bearing",
                            "age_hours": 500.0 + index * 100,
                            "expected_life_hours": 5000.0,
                            "replacement_cost": 1200.0,
                        },
                    ),
                    relationships=[Relationship(machine_id, "installed-in")],
                )
            )

    def new_context(
        self,
        object_type: str,
        time: datetime,
        attributes: Mapping[str, str | int | float | bool],
        relationships: Sequence[Relationship] = (),
    ) -> str:
        identifier = self.object_ids[object_type].next()
        self.builder.add_object(
            OcelObject(
                id=identifier,
                type=object_type,
                attributes=_attributes(time, attributes),
                relationships=list(relationships),
            )
        )
        return identifier

    def _update_machine_history(self, machine_id: str, time: datetime) -> None:
        current = self.states[machine_id].dynamic_attributes()
        previous = self.last_dynamic[machine_id]
        machine = self.builder.objects[machine_id]
        for name, value in current.items():
            if previous.get(name) != value:
                machine.attributes.append(ObjectAttribute.create(name, time, value))
        self.last_dynamic[machine_id] = dict(current)

    def emit(
        self,
        machine_id: str,
        event_type: str,
        time: datetime,
        *,
        updates: Mapping[str, str | int | float | bool] | None = None,
        context: Sequence[tuple[str, str]] = (),
        regime: str,
        regime_factors: Sequence[str] = (),
        true_fault: str = "none",
        passive: bool = False,
        pattern_key: str | None = None,
    ) -> str:
        state = self.states[machine_id]
        for name, value in (updates or {}).items():
            if not hasattr(state, name):
                raise ValueError(f"unknown machine state field {name!r}")
            setattr(state, name, value)
        if not 0 <= state.health_index <= 1:
            raise ValueError("health index left [0, 1]")
        if not 0 <= state.load_fraction <= 1.5:
            raise ValueError("load fraction left [0, 1.5]")
        self._update_machine_history(machine_id, time)
        relationships = [
            Relationship(machine_id, "machine perspective"),
            Relationship(self.components[machine_id], "installed component"),
        ]
        relationships.extend(
            Relationship(identifier, qualifier) for identifier, qualifier in context
        )
        deduplicated: list[Relationship] = []
        seen: set[tuple[str, str]] = set()
        for relationship in relationships:
            key = (relationship.object_id, relationship.qualifier)
            if key not in seen:
                seen.add(key)
                deduplicated.append(relationship)
        event_id = self.event_ids.next()
        values = {**state.dynamic_attributes(), "passive_observation": passive}
        self.builder.add_event(
            OcelEvent.create(
                event_id,
                event_type,
                time,
                attributes=[EventAttribute(name, value) for name, value in values.items()],
                relationships=deduplicated,
            )
        )
        if pattern_key:
            self.pattern_events[pattern_key].append(event_id)
        reference = reference_state(state)
        before = self.previous_reference[machine_id]
        is_transition = before is not None and before != reference.name
        transition_id = self.transition_ids.next() if is_transition else None
        event_time = format_timestamp(time)
        self.state_rows.append(
            {
                "scenario": "manufacturing",
                "leading_object_type": "Machine",
                "leading_object_id": machine_id,
                "event_id": event_id,
                "event_time": event_time,
                "reference_state": reference.name,
                "state_reason": reference.reason,
                "policy_or_rule_version": "MFG-R1",
                "data_complete": state.data_complete,
                "state_before": before,
                "state_after": reference.name,
                "is_transition": is_transition,
                "transition_id": transition_id,
            }
        )
        if before is None:
            self.state_started_at[machine_id] = time
        elif is_transition:
            started = self.state_started_at[machine_id]
            self.transition_rows.append(
                {
                    "transition_id": transition_id,
                    "leading_object_id": machine_id,
                    "event_id": event_id,
                    "event_time": event_time,
                    "from_state": before,
                    "to_state": reference.name,
                    "from_state_started_at": format_timestamp(started),
                    "duration_minutes": (time - started).total_seconds() / 60,
                }
            )
            self.state_started_at[machine_id] = time
        self.previous_reference[machine_id] = reference.name
        previous_regime = self.previous_regime[machine_id]
        if previous_regime != regime:
            self.regime_started_at[machine_id] = time
        self.previous_regime[machine_id] = regime
        self.latent_rows.append(
            {
                "leading_object_id": machine_id,
                "event_id": event_id,
                "event_time": event_time,
                "primary_regime": regime,
                "regime_factors_json": list(dict.fromkeys((regime, *regime_factors))),
                "regime_started_at": format_timestamp(self.regime_started_at[machine_id]),
                "transition_window": previous_regime is not None and previous_regime != regime,
            }
        )
        physics = self.physics[machine_id]
        self.physical_rows.append(
            {
                "leading_object_id": machine_id,
                "event_id": event_id,
                "event_time": event_time,
                "bearing_wear": physics.bearing_wear,
                "thermal_wear": physics.thermal_wear,
                "calibration_drift": physics.calibration_drift,
                "true_health_index": physics.health_index(),
                "true_fault_family": true_fault,
            }
        )
        operational = (
            "Stopped"
            if state.mode == "DOWN"
            else "Operating"
            if state.mode == "RUNNING"
            else state.mode.title()
        )
        noisy = len(self.noisy_operational_rows) % 11 == 7
        if noisy:
            operational = "Operating" if operational == "Stopped" else "Stopped"
        self.noisy_operational_rows.append(
            {
                "leading_object_id": machine_id,
                "event_id": event_id,
                "event_time": event_time,
                "operational_state": operational,
                "is_deliberately_noisy": noisy,
            }
        )
        return event_id

    def telemetry(
        self,
        machine_id: str,
        time: datetime,
        *,
        health: float,
        vibration: float,
        temperature: float,
        power: float,
        critical_alarm: bool = False,
        regime: str,
        true_fault: str,
        pattern_key: str | None = None,
    ) -> str:
        latched = self.hysteresis[machine_id].update(
            health_index=health,
            vibration_rms=vibration,
            temperature_c=temperature,
            critical_alarm=critical_alarm,
        )
        return self.emit(
            machine_id,
            "Sensor Snapshot",
            time,
            updates={
                "health_index": health,
                "vibration_rms": vibration,
                "temperature_c": temperature,
                "power_kw": power,
                "degraded_latched": latched,
            },
            regime=regime,
            true_fault=true_fault,
            passive=True,
            pattern_key=pattern_key,
        )

    def add_pattern(
        self,
        pattern_id: str,
        machine_id: str,
        *,
        family: str,
        from_state: str,
        to_state: str,
        sequence: Sequence[str],
        object_types: Sequence[str],
        event_key: str | None = None,
        instance_number: int = 1,
        noise_level: float = 0.0,
        exact: bool = True,
    ) -> None:
        events = self.pattern_events[event_key or pattern_id]
        self.pattern_rows.append(
            {
                "pattern_id": pattern_id,
                "instance_id": f"{pattern_id}-I{instance_number:03d}",
                "family": family,
                "leading_object_id": machine_id,
                "start_event_id": events[0],
                "end_event_id": events[-1],
                "from_state": from_state,
                "to_state": to_state,
                "expected_sequence_json": list(sequence),
                "expected_object_types_json": list(object_types),
                "noise_level": noise_level,
                "should_be_exact_in_behavior_log": exact,
            }
        )

    def add_violation(
        self,
        rule_id: str,
        machine_id: str,
        event_id: str,
        event_time: datetime,
        *,
        related_object_id: str = "",
        deadline: datetime | None = None,
        details: Mapping[str, object],
    ) -> None:
        self.violation_rows.append(
            {
                "rule_id": rule_id,
                "violation_id": f"MFG-V-{len(self.violation_rows) + 1:03d}",
                "leading_object_id": machine_id,
                "event_id": event_id,
                "event_time": format_timestamp(event_time),
                "related_object_id": related_object_id,
                "expected_deadline": format_timestamp(deadline) if deadline else "",
                "details_json": dict(details),
            }
        )

    def _work_order(
        self, machine_id: str, time: datetime, fault: str, *, status: str = "open"
    ) -> str:
        return self.new_context(
            "WorkOrder",
            time,
            {
                "priority": "urgent",
                "fault_family": fault,
                "creation_time": format_timestamp(time),
                "completion_time": format_timestamp(time + timedelta(days=2)),
                "status": status,
            },
            [Relationship(machine_id, "maintains")],
        )

    def _inspection(
        self, machine_id: str, time: datetime, result: str, inspection_type: str
    ) -> str:
        return self.new_context(
            "Inspection",
            time,
            {
                "inspection_type": inspection_type,
                "result": result,
                "measured_value": 1.0 if result == "passed" else 0.0,
            },
            [Relationship(machine_id, "inspects")],
        )

    def _initialize(self) -> None:
        for machine_id in sorted(self.states):
            state = self.states[machine_id]
            regime = "Idle" if state.mode == "IDLE" else "Healthy Steady Run"
            if state.mode == "SETUP":
                regime = "Setup or Changeover"
            self.emit(
                machine_id,
                "Initialize Machine",
                self.start,
                regime=regime,
            )

    def _script_m1_bearing_and_quick_recovery(self) -> None:
        machine = "M-001"
        p1 = "MFG-P1"
        self.telemetry(
            machine,
            self.at(1, seconds=10),
            health=0.62,
            vibration=8.2,
            temperature=70.0,
            power=105.0,
            regime="Bearing Degradation",
            true_fault="bearing",
            pattern_key=p1,
        )
        alarm_id = self.new_context(
            "Alarm",
            self.at(1, seconds=20),
            {
                "category": "vibration",
                "severity": "warning",
                "threshold": 7.0,
                "acknowledgement_status": "acknowledged",
            },
            [Relationship(machine, "raised-on")],
        )
        self.emit(
            machine,
            "Warning Alarm Raised",
            self.at(1, seconds=20),
            updates={"alarm_severity": "warning", "fault_family_observed": "bearing"},
            context=[(alarm_id, "alarm")],
            regime="Alarm Escalation",
            true_fault="bearing",
            pattern_key=p1,
        )
        self.emit(
            machine,
            "Maintenance Request Created",
            self.at(1, seconds=30),
            updates={"maintenance_open": True},
            context=[(alarm_id, "alarm"), ("TEAM-001", "maintenance team")],
            regime="Alarm Escalation",
            true_fault="bearing",
            pattern_key=p1,
        )
        self.emit(
            machine,
            "Critical Alarm Raised",
            self.at(1, seconds=40),
            updates={"alarm_severity": "critical", "degraded_latched": True},
            context=[(alarm_id, "alarm")],
            regime="Alarm Escalation",
            true_fault="bearing",
            pattern_key=p1,
        )
        self.emit(
            machine,
            "Automatic Stop",
            self.at(1, seconds=50),
            updates={"mode": "DOWN", "down_active": True, "power_kw": 0.0},
            context=[(alarm_id, "alarm")],
            regime="Failed",
            true_fault="bearing",
            pattern_key=p1,
        )
        self.add_pattern(
            p1,
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
        )
        work_order = self._work_order(machine, self.at(1.5), "bearing")
        self.emit(
            machine,
            "Work Order Created",
            self.at(1.5),
            context=[(work_order, "work order"), ("TEAM-001", "maintenance team")],
            regime="Waiting for Maintenance",
            true_fault="bearing",
        )
        p3 = "MFG-P3"
        context = [(work_order, "work order"), ("TEAM-001", "maintenance team")]
        maintenance_started = self.emit(
            machine,
            "Maintenance Started",
            self.at(2, seconds=10),
            context=context,
            regime="Active Repair",
            true_fault="bearing",
            pattern_key=p3,
        )
        self.add_violation(
            "MFG-C2",
            machine,
            maintenance_started,
            self.at(2, seconds=10),
            related_object_id=work_order,
            deadline=self.at(1, seconds=50) + timedelta(minutes=60),
            details={"reason": "quick repair response still exceeded the high-criticality SLA"},
        )
        self.emit(
            machine,
            "Diagnosis Performed",
            self.at(2, seconds=20),
            context=context,
            regime="Active Repair",
            true_fault="bearing",
            pattern_key=p3,
        )
        self.physics[machine].replace_component("bearing")
        self.emit(
            machine,
            "Component Replaced",
            self.at(2, seconds=30),
            updates={"health_index": 0.92, "vibration_rms": 2.2, "temperature_c": 58.0},
            context=[*context, (self.components[machine], "replaced component")],
            regime="Active Repair",
            true_fault="bearing",
            pattern_key=p3,
        )
        inspection = self._inspection(machine, self.at(2, seconds=40), "passed", "safety")
        self.emit(
            machine,
            "Inspection Performed",
            self.at(2, seconds=40),
            context=[*context, (inspection, "inspection")],
            regime="Active Repair",
            pattern_key=p3,
        )
        self.emit(
            machine,
            "Test Run Completed",
            self.at(2, seconds=50),
            context=[*context, (inspection, "inspection")],
            regime="Post-Repair Recovery",
            pattern_key=p3,
        )
        self.emit(
            machine,
            "Machine Restarted",
            self.at(2, seconds=60),
            updates={
                "mode": "RUNNING",
                "down_active": False,
                "recovery_active": True,
                "alarm_severity": "none",
                "stable_run_minutes": 0,
            },
            context=context,
            regime="Post-Repair Recovery",
            pattern_key=p3,
        )
        self.add_pattern(
            p3,
            machine,
            family="inter",
            from_state="Down",
            to_state="Recovery",
            sequence=(
                "Maintenance Started",
                "Diagnosis Performed",
                "Component Replaced",
                "Inspection Performed",
                "Test Run Completed",
                "Machine Restarted",
            ),
            object_types=("Machine", "WorkOrder", "Component", "Inspection", "MaintenanceTeam"),
        )
        stable_time = self.at(2) + timedelta(
            minutes=self.config.maintenance.recovery_stable_minutes
        )
        self.emit(
            machine,
            "Operation Completed",
            stable_time,
            updates={
                "recovery_active": False,
                "degraded_latched": False,
                "stable_run_minutes": self.config.maintenance.recovery_stable_minutes,
            },
            context=context,
            regime="Healthy Steady Run",
        )
        self.emit(
            machine,
            "Maintenance Completed",
            stable_time + timedelta(seconds=10),
            updates={"maintenance_open": False},
            context=context,
            regime="Healthy Steady Run",
        )

    def _script_m2_slow_recovery(self) -> None:
        machine = "M-002"
        stop_time = self.at(1, seconds=100)
        self.emit(
            machine,
            "Automatic Stop",
            stop_time,
            updates={"mode": "DOWN", "down_active": True, "maintenance_open": True},
            regime="Failed",
            true_fault="thermal",
        )
        work_order = self._work_order(machine, stop_time + timedelta(minutes=5), "thermal")
        self.emit(
            machine,
            "Work Order Created",
            stop_time + timedelta(minutes=5),
            context=[(work_order, "work order")],
            regime="Waiting for Maintenance",
            true_fault="thermal",
        )
        p4 = "MFG-P4"
        start = self.at(2, seconds=100)
        context = [(work_order, "work order"), ("TEAM-003", "maintenance team")]
        events = (
            ("Maintenance Started", timedelta(seconds=0)),
            ("Diagnosis Performed", timedelta(minutes=5)),
            ("Part Unavailable", timedelta(minutes=10)),
            ("Component Replaced", timedelta(hours=4)),
            ("Test Failed", timedelta(hours=4, minutes=20)),
            ("Repair Performed", timedelta(hours=5)),
            ("Test Run Completed", timedelta(hours=6)),
            ("Machine Restarted", timedelta(hours=6, minutes=5)),
        )
        event_ids: dict[str, str] = {}
        for event_type, offset in events:
            updates: dict[str, str | int | float | bool] = {}
            if event_type == "Component Replaced":
                self.physics[machine].replace_component("thermal")
                updates = {"health_index": 0.8, "temperature_c": 68.0}
            elif event_type == "Repair Performed":
                self.physics[machine].repair(0.8)
                updates = {"health_index": 0.9, "vibration_rms": 2.5}
            elif event_type == "Machine Restarted":
                updates = {
                    "mode": "RUNNING",
                    "down_active": False,
                    "recovery_active": True,
                    "stable_run_minutes": 0,
                }
            event_ids[event_type] = self.emit(
                machine,
                event_type,
                start + offset,
                updates=updates,
                context=context,
                regime=(
                    "Post-Repair Recovery"
                    if event_type in {"Test Run Completed", "Machine Restarted"}
                    else "Active Repair"
                ),
                true_fault="thermal",
                pattern_key=p4,
            )
        self.add_violation(
            "MFG-C2",
            machine,
            event_ids["Maintenance Started"],
            start,
            related_object_id=work_order,
            deadline=stop_time + timedelta(minutes=60),
            details={"reason": "maintenance began after the high-criticality Down SLA"},
        )
        self.add_pattern(
            p4,
            machine,
            family="inter",
            from_state="Down",
            to_state="Recovery",
            sequence=tuple(item[0] for item in events),
            object_types=("Machine", "WorkOrder", "Component", "MaintenanceTeam"),
        )
        stable = start + timedelta(hours=8)
        self.emit(
            machine,
            "Operation Completed",
            stable,
            updates={
                "recovery_active": False,
                "degraded_latched": False,
                "stable_run_minutes": self.config.maintenance.recovery_stable_minutes,
            },
            context=context,
            regime="Healthy Steady Run",
        )

    def _script_m3_recurrence(self) -> None:
        machine = "M-003"
        pre = self.at(1, seconds=200)
        work_order = self._work_order(machine, pre, "bearing")
        stop = self.emit(
            machine,
            "Automatic Stop",
            pre,
            updates={"mode": "DOWN", "down_active": True, "maintenance_open": True},
            context=[(work_order, "work order")],
            regime="Failed",
            true_fault="bearing",
        )
        self.add_violation(
            "MFG-C2",
            machine,
            stop,
            pre,
            related_object_id=work_order,
            deadline=pre + timedelta(minutes=60),
            details={"reason": "maintenance never started after Down"},
        )
        inspection = self._inspection(machine, self.at(1.5, seconds=200), "passed", "post-repair")
        self.emit(
            machine,
            "Inspection Performed",
            self.at(1.5, seconds=200),
            context=[(work_order, "work order"), (inspection, "inspection")],
            regime="Post-Repair Recovery",
        )
        self.emit(
            machine,
            "Test Run Completed",
            self.at(1.5, seconds=210),
            context=[(work_order, "work order"), (inspection, "inspection")],
            regime="Post-Repair Recovery",
        )
        p5 = "MFG-P5"
        restart = self.at(2, seconds=200)
        context = [(work_order, "work order")]
        self.emit(
            machine,
            "Machine Restarted",
            restart,
            updates={"mode": "RUNNING", "down_active": False, "recovery_active": True},
            context=context,
            regime="Post-Repair Recovery",
            pattern_key=p5,
        )
        self.emit(
            machine,
            "Operation Completed",
            restart + timedelta(minutes=60),
            updates={"recovery_active": False, "stable_run_minutes": 60},
            context=context,
            regime="Healthy Steady Run",
            pattern_key=p5,
        )
        self.emit(
            machine,
            "Warning Alarm Raised",
            restart + timedelta(hours=2),
            updates={
                "alarm_severity": "warning",
                "degraded_latched": True,
                "health_index": 0.63,
                "vibration_rms": 8.0,
            },
            context=context,
            regime="Bearing Degradation",
            true_fault="bearing",
            pattern_key=p5,
        )
        self.add_pattern(
            p5,
            machine,
            family="inter",
            from_state="Recovery",
            to_state="Degraded",
            sequence=("Machine Restarted", "Operation Completed", "Warning Alarm Raised"),
            object_types=("Machine", "WorkOrder", "Component"),
        )

    def _script_m4_unsafe_restart(self) -> None:
        machine = "M-004"
        p6 = "MFG-P6"
        critical_time = self.at(1, seconds=300)
        critical = self.emit(
            machine,
            "Critical Alarm Raised",
            critical_time,
            updates={"alarm_severity": "critical", "degraded_latched": True},
            regime="Alarm Escalation",
            true_fault="bearing",
            pattern_key=p6,
        )
        stop = self.emit(
            machine,
            "Automatic Stop",
            critical_time + timedelta(seconds=10),
            updates={"mode": "DOWN", "down_active": True},
            regime="Failed",
            true_fault="bearing",
            pattern_key=p6,
        )
        self.add_violation(
            "MFG-C2",
            machine,
            stop,
            critical_time + timedelta(seconds=10),
            deadline=critical_time + timedelta(seconds=10, minutes=60),
            details={"reason": "maintenance never started after Down"},
        )
        restart_time = critical_time + timedelta(minutes=30)
        restart = self.emit(
            machine,
            "Machine Restarted",
            restart_time,
            updates={"mode": "RUNNING", "down_active": False, "recovery_active": True},
            regime="Post-Repair Recovery",
            true_fault="bearing",
            pattern_key=p6,
        )
        self.add_pattern(
            p6,
            machine,
            family="inter",
            from_state="Degraded",
            to_state="Recovery",
            sequence=("Critical Alarm Raised", "Automatic Stop", "Machine Restarted"),
            object_types=("Machine", "Alarm", "Component"),
        )
        self.add_violation(
            "MFG-C1",
            machine,
            critical,
            critical_time,
            deadline=critical_time
            + timedelta(minutes=self.config.maintenance.critical_request_sla_minutes),
            details={"reason": "critical alarm had no maintenance request within SLA"},
        )
        self.add_violation(
            "MFG-C3",
            machine,
            restart,
            restart_time,
            details={"reason": "machine restarted without passed inspection or test"},
        )

    def _script_m5_completion_violation(self) -> None:
        machine = "M-005"
        work_order = self._work_order(machine, self.at(1, seconds=400), "general")
        self.emit(
            machine,
            "Maintenance Request Created",
            self.at(1, seconds=400),
            updates={"maintenance_open": True},
            context=[(work_order, "work order")],
            regime="Waiting for Maintenance",
        )
        self.emit(
            machine,
            "Maintenance Started",
            self.at(1, seconds=410),
            context=[(work_order, "work order")],
            regime="Active Repair",
        )
        inspection = self._inspection(machine, self.at(1, seconds=415), "passed", "maintenance")
        self.emit(
            machine,
            "Inspection Performed",
            self.at(1, seconds=415),
            context=[(work_order, "work order"), (inspection, "inspection")],
            regime="Active Repair",
        )
        self.emit(
            machine,
            "Test Run Completed",
            self.at(1, seconds=418),
            context=[(work_order, "work order"), (inspection, "inspection")],
            regime="Post-Repair Recovery",
        )
        self.emit(
            machine,
            "Machine Restarted",
            self.at(1, seconds=420),
            updates={"recovery_active": True, "stable_run_minutes": 0},
            context=[(work_order, "work order")],
            regime="Post-Repair Recovery",
        )
        completion_time = self.at(1, seconds=430)
        completion = self.emit(
            machine,
            "Maintenance Completed",
            completion_time,
            updates={"maintenance_open": False},
            context=[(work_order, "work order")],
            regime="Post-Repair Recovery",
        )
        self.add_violation(
            "MFG-C4",
            machine,
            completion,
            completion_time,
            related_object_id=work_order,
            details={"reason": "maintenance completed before stable recovery"},
        )

    def _script_m6_quality(self) -> None:
        machine = "M-006"
        p2 = "MFG-P2"
        self.telemetry(
            machine,
            self.at(1, seconds=500),
            health=0.64,
            vibration=4.0,
            temperature=88.0,
            power=110.0,
            regime="Thermal Drift",
            true_fault="thermal",
            pattern_key=p2,
        )
        self.emit(
            machine,
            "Defect Detected",
            self.at(1, seconds=510),
            regime="Quality Drift",
            true_fault="thermal",
            pattern_key=p2,
        )
        self.emit(
            machine,
            "Quality Hold Started",
            self.at(1, seconds=520),
            updates={"quality_hold_active": True},
            regime="Quality Drift",
            true_fault="thermal",
            pattern_key=p2,
        )
        inspection = self._inspection(machine, self.at(1, seconds=530), "failed", "quality")
        self.emit(
            machine,
            "Inspection Performed",
            self.at(1, seconds=530),
            context=[(inspection, "inspection")],
            regime="Quality Drift",
            true_fault="thermal",
            pattern_key=p2,
        )
        self.physics[machine].replace_component("calibration")
        self.emit(
            machine,
            "Calibration Performed",
            self.at(1, seconds=540),
            updates={"temperature_c": 70.0},
            context=[(inspection, "inspection")],
            regime="Active Repair",
            true_fault="thermal",
            pattern_key=p2,
        )
        self.add_pattern(
            p2,
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
        )
        release_time = self.at(1, seconds=550)
        release = self.emit(
            machine,
            "Quality Hold Released",
            release_time,
            updates={"quality_hold_active": False},
            context=[(inspection, "inspection")],
            regime="Quality Drift",
            true_fault="thermal",
        )
        self.add_violation(
            "MFG-C5",
            machine,
            release,
            release_time,
            related_object_id=inspection,
            details={"reason": "quality hold released after a failed inspection"},
        )

    def _script_m7_component_violation(self) -> None:
        machine = "M-007"
        self.emit(
            machine,
            "Setup Completed",
            self.at(1, seconds=600),
            updates={"mode": "RUNNING"},
            context=[("OPER-001", "operator")],
            regime="Healthy Steady Run",
        )
        replacement_time = self.at(2, seconds=600)
        self.physics[machine].replace_component("bearing")
        replacement = self.emit(
            machine,
            "Component Replaced",
            replacement_time,
            context=[(self.components[machine], "replaced component")],
            regime="Active Repair",
        )
        self.add_violation(
            "MFG-C6",
            machine,
            replacement,
            replacement_time,
            related_object_id=self.components[machine],
            details={"reason": "component replacement has no open WorkOrder"},
        )

    def _script_m8_data_gap(self) -> None:
        machine = "M-008"
        self.emit(
            machine,
            "Setup Started",
            self.at(1, seconds=700),
            updates={"mode": "SETUP"},
            regime="Setup or Changeover",
        )
        self.emit(
            machine,
            "Setup Completed",
            self.at(1, seconds=710),
            updates={"mode": "RUNNING", "load_fraction": 0.6, "power_kw": 70.0},
            regime="Healthy Steady Run",
        )
        self.emit(
            machine,
            "Sensor Snapshot",
            self.at(2, seconds=700),
            updates={"data_complete": False},
            regime="Data Gap",
            passive=True,
        )
        self.emit(
            machine,
            "Sensor Snapshot",
            self.at(3, seconds=700),
            updates={"data_complete": True},
            regime="Healthy Steady Run",
            passive=True,
        )

    def _finalize(self) -> None:
        for index, machine in enumerate(sorted(self.states)):
            self.emit(
                machine,
                "Simulation End Snapshot",
                self.end + timedelta(seconds=index),
                regime=self.previous_regime[machine] or "Idle",
                passive=True,
            )

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
        self._finalize()
        self.builder.validate()

    def _episodes(
        self, rows: Sequence[Mapping[str, Any]], label_field: str
    ) -> list[dict[str, Any]]:
        grouped: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
        for row in rows:
            grouped[str(row["leading_object_id"])].append(row)
        result: list[dict[str, Any]] = []
        for machine, machine_rows in sorted(grouped.items()):
            ordered = sorted(machine_rows, key=lambda row: parse_timestamp(str(row["event_time"])))
            start = 0
            number = 1
            for index in range(1, len(ordered) + 1):
                boundary = index == len(ordered) or (
                    ordered[index][label_field] != ordered[start][label_field]
                )
                if not boundary:
                    continue
                start_row = ordered[start]
                end_row = ordered[index] if index < len(ordered) else ordered[index - 1]
                start_time = parse_timestamp(str(start_row["event_time"]))
                end_time = parse_timestamp(str(end_row["event_time"]))
                result.append(
                    {
                        "leading_object_id": machine,
                        "episode_id": f"{machine}-EP-{number:03d}",
                        "label": start_row[label_field],
                        "start_event_id": start_row["event_id"],
                        "end_event_id": end_row["event_id"],
                        "start_time": start_row["event_time"],
                        "end_time": end_row["event_time"],
                        "duration_minutes": (end_time - start_time).total_seconds() / 60,
                        "event_count": index - start,
                        "right_censored": index == len(ordered),
                    }
                )
                number += 1
                start = index
        return result

    def _prediction_samples(self) -> list[dict[str, Any]]:
        grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for row in self.state_rows:
            grouped[str(row["leading_object_id"])].append(row)
        result: list[dict[str, Any]] = []
        for machine, rows in grouped.items():
            ordered = sorted(rows, key=lambda row: parse_timestamp(str(row["event_time"])))
            times = [parse_timestamp(str(row["event_time"])) for row in ordered]
            next_down: list[int | None] = [None] * len(ordered)
            next_running: list[int | None] = [None] * len(ordered)
            next_problem: list[int | None] = [None] * len(ordered)
            down_index: int | None = None
            running_index: int | None = None
            problem_index: int | None = None
            for reverse_index in range(len(ordered) - 1, -1, -1):
                next_down[reverse_index] = down_index
                next_running[reverse_index] = running_index
                next_problem[reverse_index] = problem_index
                state = ordered[reverse_index]["reference_state"]
                if state == "Down":
                    down_index = reverse_index
                if state == "Running":
                    running_index = reverse_index
                if state in {"Degraded", "Down"}:
                    problem_index = reverse_index
            for index, row in enumerate(ordered):
                cutoff = times[index]
                common = {
                    "leading_object_id": machine,
                    "cutoff_event_id": row["event_id"],
                    "cutoff_time": row["event_time"],
                    "current_state": row["reference_state"],
                    "split_group": machine,
                }
                if row["reference_state"] != "Down":
                    future_down_index = next_down[index]
                    down_minutes = (
                        (times[future_down_index] - cutoff).total_seconds() / 60
                        if future_down_index is not None
                        else ""
                    )
                    for hours in (4, 24):
                        result.append(
                            {
                                **common,
                                "label_name": f"Down within {hours} hours",
                                "horizon_minutes": hours * 60,
                                "label": future_down_index is not None
                                and float(down_minutes) <= hours * 60,
                                "time_to_event_minutes": down_minutes,
                            }
                        )
                    result.append(
                        {
                            **common,
                            "label_name": "Time to Down",
                            "horizon_minutes": int((self.end - cutoff).total_seconds() / 60),
                            "label": future_down_index is not None,
                            "time_to_event_minutes": down_minutes,
                        }
                    )
                event_type = self.builder.events[str(row["event_id"])].type
                if event_type == "Machine Restarted":
                    future_running_index = next_running[index]
                    recovery_minutes = (
                        (times[future_running_index] - cutoff).total_seconds()
                        / 60
                        if future_running_index is not None
                        else ""
                    )
                    result.extend(
                        (
                            {
                                **common,
                                "label_name": "Stable Running recovery within 8 hours",
                                "horizon_minutes": 480,
                                "label": future_running_index is not None
                                and float(recovery_minutes) <= 480,
                                "time_to_event_minutes": recovery_minutes,
                            },
                            {
                                **common,
                                "label_name": "Time to stable recovery",
                                "horizon_minutes": int((self.end - cutoff).total_seconds() / 60),
                                "label": future_running_index is not None,
                                "time_to_event_minutes": recovery_minutes,
                            },
                        )
                    )
                if event_type == "Machine Restarted":
                    recurrence_index = next_problem[index]
                    recurrence = (
                        recurrence_index is not None
                        and times[recurrence_index] <= cutoff + timedelta(hours=24)
                    )
                    result.append(
                        {
                            **common,
                            "label_name": "Recurrent Degraded or Down within 24 hours",
                            "horizon_minutes": 1440,
                            "label": recurrence,
                            "time_to_event_minutes": (
                                (times[recurrence_index] - cutoff).total_seconds() / 60
                                if recurrence and recurrence_index is not None
                                else ""
                            ),
                        }
                    )
        return result

    def observed_document(self) -> dict[str, Any]:
        return self.builder.to_dict()

    def perturbation_manifest(self) -> list[dict[str, object]]:
        return []

    def write(self, output_dir: Path, config_path: Path) -> None:
        writer = RunWriter(output_dir)
        writer.prepare()
        document = self.observed_document()
        observed_events = observed_event_index(document)
        observed = canonical_json_bytes(document)
        writer.write_bytes("observed.ocel.json", observed)
        behavior = {
            **document,
            "events": [
                event
                for event in document["events"]
                if not next(
                    attribute["value"]
                    for attribute in event["attributes"]
                    if attribute["name"] == "passive_observation"
                )
            ],
        }
        OcelBuilder.from_dict(behavior, leading_object_type="Machine")
        writer.write_json("observed.behavior.ocel.json", behavior, pretty=False)
        query = Path(__file__).parents[2] / "queries" / "manufacturing_state.sql"
        writer.write_text("state_query.sql", query.read_text(encoding="utf-8"))
        state_rows, observed_transitions = align_state_truth(
            self.state_rows,
            self.transition_rows,
            document,
            unknown_reason="telemetry or mode data are incomplete",
            observed_transition_prefix="MFG-OBS-T-",
        )
        latent_rows = sorted(
            align_event_rows(self.latent_rows, observed_events, retain_unobserved=True),
            key=lambda row: (str(row["leading_object_id"]), str(row["event_time"])),
        )
        writer.write_csv("truth/state_at_event.csv", STATE_FIELDS, state_rows)
        state_episodes = self._episodes(state_rows, "reference_state")
        writer.write_csv("truth/state_episodes.csv", tuple(state_episodes[0]), state_episodes)
        writer.write_csv(
            "truth/transitions.csv", tuple(observed_transitions[0]), observed_transitions
        )
        writer.write_csv("truth/latent_regime_at_event.csv", LATENT_FIELDS, latent_rows)
        regime_episodes = self._episodes(latent_rows, "primary_regime")
        writer.write_csv(
            "truth/latent_regime_episodes.csv", tuple(regime_episodes[0]), regime_episodes
        )
        writer.write_csv("truth/injected_pattern_instances.csv", PATTERN_FIELDS, self.pattern_rows)
        writer.write_csv(
            "truth/conformance_violations.csv",
            VIOLATION_FIELDS,
            align_event_rows(self.violation_rows, observed_events, retain_unobserved=True),
        )
        prediction = self._prediction_samples()
        state_by_event = {str(row["event_id"]): row for row in state_rows}
        prediction = [
            {
                **row,
                "cutoff_time": observed_events[str(row["cutoff_event_id"])]["time"],
                "current_state": state_by_event[str(row["cutoff_event_id"])]["reference_state"],
            }
            for row in prediction
            if str(row["cutoff_event_id"]) in observed_events
        ]
        writer.write_csv("truth/prediction_samples.csv", tuple(prediction[0]), prediction)
        states_by_machine: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for row in state_rows:
            states_by_machine[str(row["leading_object_id"])].append(row)
        outcomes = []
        for machine in sorted(self.states):
            rows = states_by_machine[machine]
            outcomes.append(
                {
                    "leading_object_id": machine,
                    "final_state": rows[-1]["reference_state"],
                    "transition_count": sum(bool(row["is_transition"]) for row in rows),
                    "down_event_count": sum(row["reference_state"] == "Down" for row in rows),
                }
            )
        writer.write_csv("truth/outcomes_by_object.csv", tuple(outcomes[0]), outcomes)
        writer.write_csv(
            "truth/physical_state_at_event.csv",
            tuple(self.physical_rows[0]),
            align_event_rows(self.physical_rows, observed_events),
        )
        writer.write_csv(
            "truth/noisy_operational_state_at_event.csv",
            tuple(self.noisy_operational_rows[0]),
            align_event_rows(self.noisy_operational_rows, observed_events),
        )
        writer.write_json(
            "truth/causal_truth.json",
            manufacturing_causal_truth(
                self.seed_tree.stream("causal intervention"),
                profile=self.config.profile,
                pairs=max(24, self.config.entities.machines),
            ),
        )
        generate_analyst_tasks(output_dir, "manufacturing")
        state_counts = Counter(str(row["reference_state"]) for row in state_rows)
        transition_counts = Counter(
            f"{row['from_state']} -> {row['to_state']}" for row in observed_transitions
        )
        event_counts = Counter(str(event["type"]) for event in document["events"])
        counts = {
            "events": len(document["events"]),
            "objects": len(document["objects"]),
            "e2o": sum(len(event["relationships"]) for event in document["events"]),
            "o2o": sum(len(item.get("relationships", [])) for item in document["objects"]),
            "leading_objects": len(self.states),
            "event_types": len(self.builder.event_types),
            "object_types": len(self.builder.object_types),
        }
        writer.write_json(
            "expected/summary.json",
            {
                "counts": counts,
                "state_counts": dict(sorted(state_counts.items())),
                "transition_counts": dict(sorted(transition_counts.items())),
                "event_type_counts": dict(sorted(event_counts.items())),
                "pattern_instances": len(self.pattern_rows),
                "conformance_violations": len(self.violation_rows),
            },
        )
        writer.write_json(
            "expected/branch_coverage.json",
            {
                "states": {state: state_counts[state] for state in MANUFACTURING_STATES},
                "state_branches_covered": all(
                    state_counts[state] > 0 for state in MANUFACTURING_STATES
                ),
                "event_types_exercised": dict(sorted(event_counts.items())),
                "pattern_ids": [row["pattern_id"] for row in self.pattern_rows],
                "conformance_rule_ids": sorted({row["rule_id"] for row in self.violation_rows}),
            },
        )
        sequences: dict[str, list[str]] = {}
        for machine in sorted(self.states):
            sequence: list[str] = []
            for row in states_by_machine[machine]:
                state = str(row["reference_state"])
                if not sequence or sequence[-1] != state:
                    sequence.append(state)
            sequences[machine] = sequence
        writer.write_json(
            "expected/golden_assertions.json",
            {
                "observed_sha256": sha256_bytes(observed),
                "state_sequences": sequences,
                "transition_counts": dict(sorted(transition_counts.items())),
                "pattern_support": {
                    pattern: sum(row["pattern_id"] == pattern for row in self.pattern_rows)
                    for pattern in (f"MFG-P{index}" for index in range(1, 7))
                },
                "conformance_rule_counts": dict(
                    sorted(Counter(row["rule_id"] for row in self.violation_rows).items())
                ),
                "prediction_positive_count": sum(bool(row["label"]) for row in prediction),
            },
        )
        writer.write_manifest(
            scenario="manufacturing",
            profile=self.config.profile,
            seed=self.config.seed,
            config_sha256=config_sha256(load_yaml(config_path)),
            generator_commit=repository_commit(Path(__file__).parents[2]),
            start_time=self.start,
            end_time=self.end,
            counts=counts,
            rng_streams=self.seed_tree.metadata(),
            expected_counts={
                "states": len(state_rows),
                "transitions": len(observed_transitions),
                "patterns": len(self.pattern_rows),
                "violations": len(self.violation_rows),
            },
            perturbations=self.perturbation_manifest(),
            extra={"run_id": f"manufacturing-{self.config.profile}-{self.config.seed}"},
        )


def generate_manufacturing_golden(
    config: ManufacturingConfig, config_path: Path, output_dir: Path
) -> None:
    if config.profile != "golden":
        raise ValueError("golden manufacturing generator requires the golden profile")
    simulation = ManufacturingGoldenSimulation(config)
    simulation.simulate()
    simulation.write(output_dir, config_path)
