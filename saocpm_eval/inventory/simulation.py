"""Deterministic inventory golden simulation and output-contract generation."""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

from saocpm_eval.analyst_tasks import generate_analyst_tasks
from saocpm_eval.causal_truth import inventory_causal_truth
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
from saocpm_eval.inventory.config import InventoryConfig
from saocpm_eval.inventory.entities import InventoryState
from saocpm_eval.inventory.state_reference import INVENTORY_STATES, reference_state

INVENTORY_EVENT_ATTRIBUTES: dict[str, OcelType] = {
    "quantity": "float",
    "on_hand_before": "float",
    "on_hand_after": "float",
    "reserved_after": "float",
    "backorder_after": "float",
    "on_order_after": "float",
    "inventory_position_after": "float",
    "lower_threshold": "float",
    "upper_threshold": "float",
    "confirmed_demand_horizon": "float",
    "inbound_horizon": "float",
    "critical_understock": "boolean",
    "data_complete": "boolean",
    "passive_observation": "boolean",
    "cause_code": "string",
    "policy_version": "string",
}

INVENTORY_EVENT_TYPES = (
    "Initialize Inventory",
    "Sales Order Item Created",
    "Reservation Created",
    "Goods Issue",
    "Backorder Registered",
    "Demand Cancelled",
    "Replenishment Proposal Created",
    "Replenishment Proposal Approved",
    "Purchase Order Item Created",
    "Purchase Order Item Changed",
    "Supplier Confirmation Received",
    "Delivery Delayed",
    "Expedite Requested",
    "Goods Receipt",
    "Receipt Rejected",
    "Transfer Requested",
    "Transfer Ship",
    "Transfer Receive",
    "Cycle Count Performed",
    "Inventory Adjustment",
    "Policy Threshold Updated",
    "Data Gap Started",
    "Data Gap Ended",
    "Simulation End Snapshot",
)

STATE_TRUTH_FIELDS = (
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
class ItemMetadata:
    material_id: str
    location_id: str


def _object_attributes(
    time: datetime, values: Mapping[str, str | int | float | bool]
) -> list[ObjectAttribute]:
    return [ObjectAttribute.create(name, time, value) for name, value in values.items()]


class InventoryGoldenSimulation:
    """Scripted golden profile with independent truth and exact branch coverage."""

    def __init__(self, config: InventoryConfig) -> None:
        if config.entities.item_locations < 8:
            raise ValueError("inventory simulation requires at least eight item-locations")
        self.config = config
        self.start = config.start_time.astimezone(UTC)
        self.end = self.start + timedelta(days=config.horizon_days)
        self.builder = OcelBuilder(leading_object_type="ItemLocation")
        self.seed_tree = SeedTree(config.seed)
        self.event_ids = DeterministicIds("INV-E-", 6)
        self.transition_ids = DeterministicIds("INV-T-", 5)
        self.object_ids = {
            "SalesOrderItem": DeterministicIds("SOI-", 4),
            "ReplenishmentProposal": DeterministicIds("RP-", 4),
            "PurchaseOrderItem": DeterministicIds("POI-", 4),
            "Delivery": DeterministicIds("DEL-", 4),
            "TransferOrder": DeterministicIds("TO-", 4),
        }
        self.states: dict[str, InventoryState] = {}
        self.metadata: dict[str, ItemMetadata] = {}
        self.last_dynamic: dict[str, dict[str, str | float | bool]] = {}
        self.previous_reference: dict[str, str | None] = {}
        self.state_started_at: dict[str, datetime] = {}
        self.previous_regime: dict[str, str | None] = {}
        self.regime_started_at: dict[str, datetime] = {}
        self.state_rows: list[dict[str, Any]] = []
        self.transition_rows: list[dict[str, Any]] = []
        self.latent_rows: list[dict[str, Any]] = []
        self.pattern_rows: list[dict[str, Any]] = []
        self.violation_rows: list[dict[str, Any]] = []
        self.events_by_object: dict[str, list[str]] = defaultdict(list)
        self.pattern_event_ids: dict[str, list[str]] = defaultdict(list)
        self._declare_types()
        self._create_core_objects()

    def at(self, day: float, *, seconds: int = 0) -> datetime:
        return self.start + timedelta(days=day, seconds=seconds)

    def _declare_types(self) -> None:
        self.builder.declare_object_type(
            "ItemLocation",
            {
                "material_class": "string",
                "location_class": "string",
                "on_hand": "float",
                "reserved": "float",
                "backorder": "float",
                "on_order": "float",
                "inventory_position": "float",
                "lower_threshold": "float",
                "upper_threshold": "float",
                "demand_estimate": "float",
                "lead_time_estimate": "float",
                "policy_version": "string",
                "data_complete": "boolean",
            },
        )
        self.builder.declare_object_type(
            "Material",
            {
                "product_class": "string",
                "unit_cost": "float",
                "shelf_life_days": "integer",
                "lot_size": "integer",
                "criticality": "string",
            },
        )
        self.builder.declare_object_type(
            "Location",
            {
                "region": "string",
                "capacity_class": "string",
                "service_level_target": "float",
                "planner_team": "string",
            },
        )
        self.builder.declare_object_type(
            "Supplier",
            {
                "reliability": "float",
                "mean_lead_time_days": "float",
                "lead_time_cv": "float",
                "fill_rate": "float",
            },
        )
        self.builder.declare_object_type(
            "SalesOrderItem",
            {
                "requested_quantity": "float",
                "due_time": "time",
                "priority": "string",
                "customer_class": "string",
            },
        )
        self.builder.declare_object_type(
            "ReplenishmentProposal",
            {
                "suggested_quantity": "float",
                "reason": "string",
                "creation_time": "time",
                "status": "string",
            },
        )
        self.builder.declare_object_type(
            "PurchaseOrderItem",
            {
                "ordered_quantity": "float",
                "confirmed_quantity": "float",
                "planned_receipt_time": "time",
                "actual_receipt_time": "time",
                "status": "string",
            },
        )
        self.builder.declare_object_type(
            "Delivery",
            {"carrier": "string", "shipment_status": "string", "delay_code": "string"},
        )
        self.builder.declare_object_type(
            "TransferOrder",
            {
                "quantity": "float",
                "source_item_location": "string",
                "target_item_location": "string",
                "status": "string",
            },
        )
        self.builder.declare_object_type(
            "Planner",
            {"team": "string", "workload_band": "string", "experience_band": "string"},
        )
        for name in INVENTORY_EVENT_TYPES:
            self.builder.declare_event_type(name, INVENTORY_EVENT_ATTRIBUTES)

    def _create_core_objects(self) -> None:
        entity_rng = self.seed_tree.stream("entity parameters")
        material_ids: list[str] = []
        demand_classes = tuple(self.config.demand.class_mix)
        for index in range(self.config.entities.materials):
            identifier = f"MAT-{index + 1:04d}"
            material_ids.append(identifier)
            self.builder.add_object(
                OcelObject(
                    id=identifier,
                    type="Material",
                    attributes=_object_attributes(
                        self.start,
                        {
                            "product_class": demand_classes[index % len(demand_classes)],
                            "unit_cost": round(float(entity_rng.uniform(5.0, 250.0)), 2),
                            "shelf_life_days": 365 + index,
                            "lot_size": 10,
                            "criticality": ("high", "medium", "low")[index % 3],
                        },
                    ),
                )
            )
        location_ids: list[str] = []
        for index in range(self.config.entities.locations):
            identifier = f"LOC-{index + 1:03d}"
            location_ids.append(identifier)
            self.builder.add_object(
                OcelObject(
                    id=identifier,
                    type="Location",
                    attributes=_object_attributes(
                        self.start,
                        {
                            "region": ("West", "Central", "East")[index % 3],
                            "capacity_class": ("compact", "standard", "large")[index % 3],
                            "service_level_target": 0.95 + 0.01 * (index % 3),
                            "planner_team": f"TEAM-{index % self.config.entities.planners + 1}",
                        },
                    ),
                )
            )
        for index in range(self.config.entities.suppliers):
            self.builder.add_object(
                OcelObject(
                    id=f"SUP-{index + 1:03d}",
                    type="Supplier",
                    attributes=_object_attributes(
                        self.start,
                        {
                            "reliability": round(float(entity_rng.uniform(0.75, 0.99)), 4),
                            "mean_lead_time_days": float(2 + index),
                            "lead_time_cv": 0.1 + index * 0.05,
                            "fill_rate": 0.9 + 0.02 * (index % 3),
                        },
                    ),
                )
            )
        for index in range(self.config.entities.planners):
            self.builder.add_object(
                OcelObject(
                    id=f"PLN-{index + 1:03d}",
                    type="Planner",
                    attributes=_object_attributes(
                        self.start,
                        {
                            "team": f"TEAM-{index + 1}",
                            "workload_band": ("low", "medium", "high")[index % 3],
                            "experience_band": ("senior", "mid", "junior")[index % 3],
                        },
                    ),
                )
            )
        initial_levels = (50.0, 10.0, 40.0, 10.0, 50.0, 100.0, 50.0, 50.0)
        for index in range(self.config.entities.item_locations):
            identifier = f"IL-{index + 1:04d}"
            material_id = material_ids[index % len(material_ids)]
            location_id = location_ids[index % len(location_ids)]
            on_hand = initial_levels[index] if index < len(initial_levels) else 50.0
            state = InventoryState(on_hand=on_hand, lower_threshold=20.0, upper_threshold=80.0)
            self.states[identifier] = state
            self.metadata[identifier] = ItemMetadata(material_id, location_id)
            dynamic = state.dynamic_attributes()
            self.last_dynamic[identifier] = dict(dynamic)
            self.previous_reference[identifier] = None
            self.previous_regime[identifier] = None
            attributes: dict[str, str | float | bool] = {
                "material_class": demand_classes[index % len(demand_classes)],
                "location_class": ("compact", "standard", "large")[index % 3],
                **dynamic,
            }
            self.builder.add_object(
                OcelObject(
                    id=identifier,
                    type="ItemLocation",
                    attributes=_object_attributes(self.start, attributes),
                    relationships=[
                        Relationship(material_id, "material"),
                        Relationship(location_id, "location"),
                    ],
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
                attributes=_object_attributes(time, attributes),
                relationships=list(relationships),
            )
        )
        return identifier

    def _apply_effect(
        self,
        state: InventoryState,
        event_type: str,
        quantity: float,
        updates: Mapping[str, str | float | bool],
    ) -> None:
        if event_type in {"Goods Receipt", "Transfer Receive"}:
            state.on_hand += quantity
        elif event_type in {"Goods Issue", "Transfer Ship"}:
            state.on_hand -= quantity
        elif event_type == "Inventory Adjustment":
            state.on_hand += quantity
        if event_type == "Purchase Order Item Created":
            state.on_order += quantity
        elif event_type == "Goods Receipt":
            state.on_order = max(0.0, state.on_order - quantity)
            state.backorder = max(0.0, state.backorder - quantity)
        for name, value in updates.items():
            if not hasattr(state, name):
                raise ValueError(f"unknown inventory state field {name!r}")
            setattr(state, name, value)
        if (
            not self.config.stock.allow_negative_on_hand
            and state.on_hand < -self.config.stock.numeric_tolerance
        ):
            raise ValueError("golden script produced negative on-hand stock")
        state.on_hand = max(0.0, state.on_hand)
        for name in ("reserved", "backorder", "on_order"):
            if float(getattr(state, name)) < -self.config.stock.numeric_tolerance:
                raise ValueError(f"golden script produced negative {name}")

    def _update_object_history(self, object_id: str, time: datetime) -> None:
        state = self.states[object_id]
        current = state.dynamic_attributes()
        target = self.builder.objects[object_id]
        previous = self.last_dynamic[object_id]
        for name, value in current.items():
            if previous.get(name) != value:
                target.attributes.append(ObjectAttribute.create(name, time, value))
        self.last_dynamic[object_id] = dict(current)

    def emit(
        self,
        object_id: str,
        event_type: str,
        time: datetime,
        *,
        quantity: float = 0.0,
        updates: Mapping[str, str | float | bool] | None = None,
        context: Sequence[tuple[str, str]] = (),
        cause_code: str,
        regime: str,
        regime_factors: Sequence[str] = (),
        passive: bool = False,
        pattern_id: str | None = None,
    ) -> str:
        state = self.states[object_id]
        on_hand_before = state.on_hand
        self._apply_effect(state, event_type, quantity, updates or {})
        self._update_object_history(object_id, time)
        metadata = self.metadata[object_id]
        relationships = [
            Relationship(object_id, "inventory perspective"),
            Relationship(metadata.material_id, "material"),
            Relationship(metadata.location_id, "location"),
        ]
        relationships.extend(
            Relationship(identifier, qualifier) for identifier, qualifier in context
        )
        deduplicated: list[Relationship] = []
        seen_relationships: set[tuple[str, str]] = set()
        for relationship in relationships:
            key = (relationship.object_id, relationship.qualifier)
            if key not in seen_relationships:
                seen_relationships.add(key)
                deduplicated.append(relationship)
        event_id = self.event_ids.next()
        values: dict[str, str | float | bool] = {
            "quantity": float(quantity),
            "on_hand_before": float(on_hand_before),
            "on_hand_after": float(state.on_hand),
            "reserved_after": float(state.reserved),
            "backorder_after": float(state.backorder),
            "on_order_after": float(state.on_order),
            "inventory_position_after": float(state.inventory_position),
            "lower_threshold": float(state.lower_threshold),
            "upper_threshold": float(state.upper_threshold),
            "confirmed_demand_horizon": float(state.confirmed_demand_horizon),
            "inbound_horizon": float(state.inbound_horizon),
            "critical_understock": state.critical_understock,
            "data_complete": state.data_complete,
            "passive_observation": passive,
            "cause_code": cause_code,
            "policy_version": state.policy_version,
        }
        self.builder.add_event(
            OcelEvent.create(
                event_id,
                event_type,
                time,
                attributes=[EventAttribute(name, value) for name, value in values.items()],
                relationships=deduplicated,
            )
        )
        self.events_by_object[object_id].append(event_id)
        if pattern_id:
            self.pattern_event_ids[pattern_id].append(event_id)
        reference = reference_state(state)
        before = self.previous_reference[object_id]
        is_transition = before is not None and before != reference.name
        transition_id = self.transition_ids.next() if is_transition else None
        event_time = format_timestamp(time)
        self.state_rows.append(
            {
                "scenario": "inventory",
                "leading_object_type": "ItemLocation",
                "leading_object_id": object_id,
                "event_id": event_id,
                "event_time": event_time,
                "reference_state": reference.name,
                "state_reason": reference.reason,
                "policy_or_rule_version": state.policy_version,
                "data_complete": state.data_complete,
                "state_before": before,
                "state_after": reference.name,
                "is_transition": is_transition,
                "transition_id": transition_id,
            }
        )
        if before is None:
            self.state_started_at[object_id] = time
        elif is_transition:
            started_at = self.state_started_at[object_id]
            self.transition_rows.append(
                {
                    "transition_id": transition_id,
                    "leading_object_id": object_id,
                    "event_id": event_id,
                    "event_time": event_time,
                    "from_state": before,
                    "to_state": reference.name,
                    "from_state_started_at": format_timestamp(started_at),
                    "duration_minutes": (time - started_at).total_seconds() / 60.0,
                }
            )
            self.state_started_at[object_id] = time
        self.previous_reference[object_id] = reference.name
        previous_regime = self.previous_regime[object_id]
        transition_window = previous_regime is not None and previous_regime != regime
        if previous_regime != regime:
            self.regime_started_at[object_id] = time
        self.previous_regime[object_id] = regime
        factors = list(dict.fromkeys((regime, *regime_factors)))
        self.latent_rows.append(
            {
                "leading_object_id": object_id,
                "event_id": event_id,
                "event_time": event_time,
                "primary_regime": regime,
                "regime_factors_json": factors,
                "regime_started_at": format_timestamp(self.regime_started_at[object_id]),
                "transition_window": transition_window,
            }
        )
        return event_id

    def add_violation(
        self,
        rule_id: str,
        object_id: str,
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
                "violation_id": f"INV-V-{len(self.violation_rows) + 1:03d}",
                "leading_object_id": object_id,
                "event_id": event_id,
                "event_time": format_timestamp(event_time),
                "related_object_id": related_object_id,
                "expected_deadline": format_timestamp(deadline) if deadline else "",
                "details_json": dict(details),
            }
        )

    def add_pattern(
        self,
        pattern_id: str,
        leading_object_id: str,
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
        events = self.pattern_event_ids[event_key or pattern_id]
        if not events:
            raise ValueError(f"pattern {pattern_id} has no recorded events")
        self.pattern_rows.append(
            {
                "pattern_id": pattern_id,
                "instance_id": f"{pattern_id}-I{instance_number:03d}",
                "family": family,
                "leading_object_id": leading_object_id,
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

    def _initialize_all(self) -> None:
        for index, object_id in enumerate(sorted(self.states)):
            regime = "Stable Low Movement" if index == 1 else "Nominal Replenishment"
            self.emit(
                object_id,
                "Initialize Inventory",
                self.start,
                quantity=self.states[object_id].on_hand,
                cause_code="INITIALIZATION",
                regime=regime,
            )

    def _script_demand_surge(self) -> None:
        object_id = "IL-0001"
        sales_time = self.at(1, seconds=10)
        sales_id = self.new_context(
            "SalesOrderItem",
            sales_time,
            {
                "requested_quantity": 45.0,
                "due_time": format_timestamp(self.at(2)),
                "priority": "urgent",
                "customer_class": "A",
            },
            [Relationship(object_id, "consumes-from")],
        )
        self.emit(
            object_id,
            "Sales Order Item Created",
            sales_time,
            updates={"confirmed_demand_horizon": 0.0},
            context=[(sales_id, "sales order item")],
            cause_code="DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id="INV-P1",
        )
        self.emit(
            object_id,
            "Reservation Created",
            self.at(1, seconds=20),
            quantity=45.0,
            updates={"reserved": 45.0},
            context=[(sales_id, "sales order item")],
            cause_code="DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id="INV-P1",
        )
        self.emit(
            object_id,
            "Goods Issue",
            self.at(1, seconds=30),
            quantity=35.0,
            updates={"reserved": 0.0, "confirmed_demand_horizon": 4.0},
            context=[(sales_id, "sales order item")],
            cause_code="DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id="INV-P1",
        )
        self.emit(
            object_id,
            "Backorder Registered",
            self.at(1, seconds=40),
            quantity=30.0,
            updates={"backorder": 30.0, "confirmed_demand_horizon": 30.0},
            context=[(sales_id, "sales order item")],
            cause_code="DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id="INV-P1",
        )
        proposal_time = self.at(1, seconds=50)
        proposal_id = self.new_context(
            "ReplenishmentProposal",
            proposal_time,
            {
                "suggested_quantity": 70.0,
                "reason": "shortage",
                "creation_time": format_timestamp(proposal_time),
                "status": "approved",
            },
            [Relationship(object_id, "replenishes")],
        )
        self.emit(
            object_id,
            "Replenishment Proposal Created",
            proposal_time,
            quantity=70.0,
            context=[(proposal_id, "proposal"), ("PLN-001", "planner")],
            cause_code="POLICY_RESPONSE",
            regime="Demand Surge Without Inbound",
            pattern_id="INV-P1",
        )
        self.add_pattern(
            "INV-P1",
            object_id,
            family="inter",
            from_state="Normal",
            to_state="Critical Understock",
            sequence=(
                "Sales Order Item Created",
                "Reservation Created",
                "Goods Issue",
                "Backorder Registered",
                "Replenishment Proposal Created",
            ),
            object_types=("ItemLocation", "SalesOrderItem", "ReplenishmentProposal", "Planner"),
        )
        approved_time = self.at(1, seconds=60)
        self.emit(
            object_id,
            "Replenishment Proposal Approved",
            approved_time,
            quantity=70.0,
            context=[(proposal_id, "proposal"), ("PLN-001", "planner")],
            cause_code="PLANNER_APPROVAL",
            regime="Demand Surge Without Inbound",
        )
        po_time = self.at(3, seconds=60)
        po_id = self.new_context(
            "PurchaseOrderItem",
            po_time,
            {
                "ordered_quantity": 70.0,
                "confirmed_quantity": 70.0,
                "planned_receipt_time": format_timestamp(self.at(5)),
                "actual_receipt_time": format_timestamp(self.at(5)),
                "status": "received",
            },
            [Relationship("SUP-001", "supplier"), Relationship(object_id, "replenishes")],
        )
        po_event = self.emit(
            object_id,
            "Purchase Order Item Created",
            po_time,
            quantity=70.0,
            updates={"inbound_horizon": 70.0},
            context=[(po_id, "purchase order item"), ("SUP-001", "supplier")],
            cause_code="PLANNER_ORDER",
            regime="Replenishment In Transit",
        )
        self.add_violation(
            "INV-C2",
            object_id,
            po_event,
            po_time,
            related_object_id=proposal_id,
            deadline=approved_time
            + timedelta(hours=self.config.policy.planner_approval_delay_hours[1]),
            details={"reason": "approved proposal converted after planner SLA"},
        )
        delivery_id = self.new_context(
            "Delivery",
            self.at(5),
            {"carrier": "CARRIER-A", "shipment_status": "received", "delay_code": "none"},
        )
        self.emit(
            object_id,
            "Goods Receipt",
            self.at(5),
            quantity=70.0,
            updates={"confirmed_demand_horizon": 0.0, "inbound_horizon": 0.0},
            context=[
                (po_id, "purchase order item"),
                (delivery_id, "delivery"),
                ("SUP-001", "supplier"),
            ],
            cause_code="RECEIPT",
            regime="Receipt-Driven Excess",
        )
        self.emit(
            object_id,
            "Transfer Requested",
            self.at(6),
            cause_code="EXCESS_MITIGATION",
            regime="Receipt-Driven Excess",
        )
        self.emit(
            object_id,
            "Transfer Ship",
            self.at(7),
            quantity=40.0,
            cause_code="EXCESS_MITIGATION",
            regime="Transfer Recovery",
        )

    def _script_supplier_delay(self) -> None:
        object_id = "IL-0002"
        po_time = self.at(1, seconds=100)
        po_id = self.new_context(
            "PurchaseOrderItem",
            po_time,
            {
                "ordered_quantity": 40.0,
                "confirmed_quantity": 40.0,
                "planned_receipt_time": format_timestamp(self.at(3)),
                "actual_receipt_time": format_timestamp(self.at(8)),
                "status": "received late",
            },
            [Relationship("SUP-002", "supplier"), Relationship(object_id, "replenishes")],
        )
        delivery_id = self.new_context(
            "Delivery",
            po_time,
            {"carrier": "CARRIER-B", "shipment_status": "delayed", "delay_code": "capacity"},
        )
        sequence = (
            ("Purchase Order Item Created", po_time, "ORDER_CREATED"),
            ("Supplier Confirmation Received", self.at(1, seconds=110), "CONFIRMED"),
            ("Delivery Delayed", self.at(3, seconds=100), "SUPPLIER_DELAY"),
            ("Expedite Requested", self.at(4, seconds=100), "EXPEDITE"),
            ("Goods Receipt", self.at(8, seconds=100), "RECEIPT"),
        )
        for event_type, time, cause in sequence:
            updates: dict[str, str | float | bool] = {}
            quantity = 0.0
            if event_type == "Purchase Order Item Created":
                quantity = 40.0
            elif event_type == "Goods Receipt":
                quantity = 40.0
                updates["confirmed_demand_horizon"] = 0.0
            self.emit(
                object_id,
                event_type,
                time,
                quantity=quantity,
                updates=updates,
                context=[
                    (po_id, "purchase order item"),
                    (delivery_id, "delivery"),
                    ("SUP-002", "supplier"),
                    ("PLN-002", "planner"),
                ],
                cause_code=cause,
                regime=(
                    "Supplier Delay" if event_type != "Goods Receipt" else "Nominal Replenishment"
                ),
                pattern_id="INV-P2",
            )
        self.add_pattern(
            "INV-P2",
            object_id,
            family="intra",
            from_state="Understock",
            to_state="Normal",
            sequence=tuple(item[0] for item in sequence),
            object_types=("ItemLocation", "PurchaseOrderItem", "Delivery", "Supplier", "Planner"),
        )

    def _script_receipt_and_transfer(self) -> None:
        source = "IL-0003"
        target = "IL-0004"
        po_time = self.at(1.5)
        po_id = self.new_context(
            "PurchaseOrderItem",
            po_time,
            {
                "ordered_quantity": 20.0,
                "confirmed_quantity": 20.0,
                "planned_receipt_time": format_timestamp(self.at(2)),
                "actual_receipt_time": format_timestamp(self.at(2)),
                "status": "received",
            },
            [Relationship("SUP-003", "supplier"), Relationship(source, "replenishes")],
        )
        self.emit(
            source,
            "Purchase Order Item Created",
            po_time,
            quantity=20.0,
            context=[(po_id, "purchase order item"), ("SUP-003", "supplier")],
            cause_code="ORDER_CREATED",
            regime="Nominal Replenishment",
        )
        delivery_id = self.new_context(
            "Delivery",
            self.at(2),
            {"carrier": "CARRIER-C", "shipment_status": "received", "delay_code": "none"},
        )
        self.emit(
            source,
            "Goods Receipt",
            self.at(2),
            quantity=20.0,
            context=[(po_id, "purchase order item"), (delivery_id, "delivery")],
            cause_code="RECEIPT",
            regime="Nominal Replenishment",
        )
        duplicate_time = self.at(3, seconds=10)
        duplicate_event = self.emit(
            source,
            "Goods Receipt",
            duplicate_time,
            quantity=50.0,
            context=[(po_id, "purchase order item"), (delivery_id, "delivery")],
            cause_code="DUPLICATE_RECEIPT",
            regime="Receipt-Driven Excess",
            pattern_id="INV-P3",
        )
        self.add_violation(
            "INV-C6",
            source,
            duplicate_event,
            duplicate_time,
            related_object_id=po_id,
            details={"reason": "duplicate over-delivery caused receipt-driven excess"},
        )
        self.emit(
            source,
            "Policy Threshold Updated",
            self.at(3, seconds=20),
            updates={"upper_threshold": 90.0, "policy_version": "P2"},
            context=[("PLN-003", "planner")],
            cause_code="CAPACITY_ALERT",
            regime="Receipt-Driven Excess",
            pattern_id="INV-P3",
        )
        transfer_time = self.at(3, seconds=30)
        transfer_id = self.new_context(
            "TransferOrder",
            transfer_time,
            {
                "quantity": 50.0,
                "source_item_location": source,
                "target_item_location": target,
                "status": "received",
            },
            [Relationship(source, "source"), Relationship(target, "target")],
        )
        request_event = self.emit(
            source,
            "Transfer Requested",
            transfer_time,
            quantity=50.0,
            context=[(transfer_id, "transfer order"), ("PLN-003", "planner")],
            cause_code="EXCESS_MITIGATION",
            regime="Receipt-Driven Excess",
            pattern_id="INV-P3",
        )
        self.pattern_event_ids["INV-P4"].append(request_event)
        ship_event = self.emit(
            source,
            "Transfer Ship",
            self.at(4, seconds=10),
            quantity=50.0,
            context=[(transfer_id, "transfer order")],
            cause_code="TRANSFER_RECOVERY",
            regime="Transfer Recovery",
        )
        self.pattern_event_ids["INV-P4"].append(ship_event)
        receive_event = self.emit(
            target,
            "Transfer Receive",
            self.at(4, seconds=20),
            quantity=30.0,
            context=[(transfer_id, "transfer order")],
            cause_code="TRANSFER_RECOVERY",
            regime="Transfer Recovery",
        )
        self.pattern_event_ids["INV-P4"].append(receive_event)
        self.add_pattern(
            "INV-P3",
            source,
            family="inter",
            from_state="Normal",
            to_state="Overstock",
            sequence=("Goods Receipt", "Policy Threshold Updated", "Transfer Requested"),
            object_types=(
                "ItemLocation",
                "PurchaseOrderItem",
                "Delivery",
                "TransferOrder",
                "Planner",
            ),
        )
        self.add_pattern(
            "INV-P4",
            target,
            family="inter",
            from_state="Understock",
            to_state="Normal",
            sequence=("Transfer Requested", "Transfer Ship", "Transfer Receive"),
            object_types=("ItemLocation", "TransferOrder", "Location"),
        )

    def _script_policy_failure(self) -> None:
        object_id = "IL-0005"
        issue = self.emit(
            object_id,
            "Goods Issue",
            self.at(1, seconds=200),
            quantity=35.0,
            updates={"confirmed_demand_horizon": 4.0},
            cause_code="DEMAND",
            regime="Demand Surge Without Inbound",
            pattern_id="INV-P5",
        )
        critical_time = self.at(1, seconds=210)
        critical_event = self.emit(
            object_id,
            "Backorder Registered",
            critical_time,
            quantity=20.0,
            updates={"backorder": 20.0, "confirmed_demand_horizon": 25.0},
            cause_code="POLICY_FAILURE",
            regime="Demand Surge Without Inbound",
            pattern_id="INV-P5",
        )
        recovery = self.emit(
            object_id,
            "Inventory Adjustment",
            self.at(3, seconds=210),
            quantity=30.0,
            updates={"confirmed_demand_horizon": 0.0, "backorder": 0.0},
            cause_code="LATE_MANUAL_RECOVERY",
            regime="Count Discrepancy",
            pattern_id="INV-P5",
        )
        self.add_violation(
            "INV-C1",
            object_id,
            critical_event,
            critical_time,
            deadline=critical_time + timedelta(hours=self.config.policy.critical_action_sla_hours),
            details={"reason": "no replenishment proposal followed Critical Understock"},
        )
        self.add_pattern(
            "INV-P5",
            object_id,
            family="inter",
            from_state="Normal",
            to_state="Normal",
            sequence=("Goods Issue", "Backorder Registered", "Inventory Adjustment"),
            object_types=("ItemLocation",),
        )
        del issue, recovery

    def _script_count_discrepancy(self) -> None:
        object_id = "IL-0006"
        self.emit(
            object_id,
            "Cycle Count Performed",
            self.at(2, seconds=300),
            quantity=50.0,
            context=[("PLN-001", "auditor")],
            cause_code="COUNT_DISCREPANCY",
            regime="Count Discrepancy",
            pattern_id="INV-P6",
        )
        self.emit(
            object_id,
            "Inventory Adjustment",
            self.at(2, seconds=310),
            quantity=-50.0,
            context=[("PLN-001", "auditor")],
            cause_code="COUNT_CORRECTION",
            regime="Count Discrepancy",
            pattern_id="INV-P6",
        )
        self.add_pattern(
            "INV-P6",
            object_id,
            family="intra",
            from_state="Overstock",
            to_state="Normal",
            sequence=("Cycle Count Performed", "Inventory Adjustment"),
            object_types=("ItemLocation", "Planner"),
        )

    def _script_remaining_violations(self) -> None:
        object_id = "IL-0007"
        proposal_time = self.at(1, seconds=400)
        proposal_id = self.new_context(
            "ReplenishmentProposal",
            proposal_time,
            {
                "suggested_quantity": 20.0,
                "reason": "manual forecast override",
                "creation_time": format_timestamp(proposal_time),
                "status": "approved",
            },
            [Relationship(object_id, "replenishes")],
        )
        self.emit(
            object_id,
            "Replenishment Proposal Created",
            proposal_time,
            quantity=20.0,
            context=[(proposal_id, "proposal"), ("PLN-002", "planner")],
            cause_code="MANUAL_OVERRIDE",
            regime="Forecast or Policy Bias",
        )
        approved_time = self.at(1, seconds=410)
        approved_event = self.emit(
            object_id,
            "Replenishment Proposal Approved",
            approved_time,
            quantity=20.0,
            context=[(proposal_id, "proposal"), ("PLN-002", "planner")],
            cause_code="MANUAL_OVERRIDE",
            regime="Forecast or Policy Bias",
        )
        self.add_violation(
            "INV-C2",
            object_id,
            approved_event,
            approved_time,
            related_object_id=proposal_id,
            deadline=approved_time
            + timedelta(hours=self.config.policy.planner_approval_delay_hours[1]),
            details={"reason": "approved proposal was never converted to a purchase order"},
        )
        high_stock_po_time = self.at(2, seconds=400)
        high_stock_po = self.new_context(
            "PurchaseOrderItem",
            high_stock_po_time,
            {
                "ordered_quantity": 20.0,
                "confirmed_quantity": 20.0,
                "planned_receipt_time": format_timestamp(self.at(6)),
                "actual_receipt_time": format_timestamp(self.at(6)),
                "status": "open",
            },
            [Relationship("SUP-001", "supplier"), Relationship(object_id, "replenishes")],
        )
        high_stock_event = self.emit(
            object_id,
            "Purchase Order Item Created",
            high_stock_po_time,
            quantity=20.0,
            context=[(high_stock_po, "purchase order item"), ("SUP-001", "supplier")],
            cause_code="NO_EXCEPTION",
            regime="Forecast or Policy Bias",
        )
        self.add_violation(
            "INV-C5",
            object_id,
            high_stock_event,
            high_stock_po_time,
            related_object_id=high_stock_po,
            details={"reason": "order created above reorder point without exception"},
        )
        future_po_time = self.at(5, seconds=400)
        future_po = self.new_context(
            "PurchaseOrderItem",
            future_po_time,
            {
                "ordered_quantity": 10.0,
                "confirmed_quantity": 10.0,
                "planned_receipt_time": format_timestamp(self.at(4)),
                "actual_receipt_time": format_timestamp(self.at(4)),
                "status": "received before creation",
            },
            [Relationship("SUP-002", "supplier"), Relationship(object_id, "replenishes")],
        )
        premature_time = self.at(4, seconds=400)
        premature_receipt = self.emit(
            object_id,
            "Goods Receipt",
            premature_time,
            quantity=10.0,
            context=[(future_po, "purchase order item")],
            cause_code="PREMATURE_RECEIPT",
            regime="Nominal Replenishment",
        )
        self.add_violation(
            "INV-C3",
            object_id,
            premature_receipt,
            premature_time,
            related_object_id=future_po,
            details={"reason": "Goods Receipt precedes Purchase Order Item Created"},
        )
        self.emit(
            object_id,
            "Purchase Order Item Created",
            future_po_time,
            quantity=10.0,
            context=[(future_po, "purchase order item")],
            cause_code="LATE_ORDER_RECORD",
            regime="Nominal Replenishment",
        )
        target = "IL-0008"
        bad_transfer_time = self.at(6, seconds=400)
        bad_transfer = self.new_context(
            "TransferOrder",
            bad_transfer_time,
            {
                "quantity": 10.0,
                "source_item_location": object_id,
                "target_item_location": target,
                "status": "sequence violation",
            },
            [Relationship(object_id, "source"), Relationship(target, "target")],
        )
        bad_receive = self.emit(
            target,
            "Transfer Receive",
            bad_transfer_time,
            quantity=10.0,
            context=[(bad_transfer, "transfer order")],
            cause_code="RECEIVE_BEFORE_SHIP",
            regime="Transfer Recovery",
        )
        self.add_violation(
            "INV-C4",
            target,
            bad_receive,
            bad_transfer_time,
            related_object_id=bad_transfer,
            details={"reason": "Transfer Receive precedes Transfer Ship"},
        )
        self.emit(
            object_id,
            "Transfer Ship",
            self.at(7, seconds=400),
            quantity=10.0,
            context=[(bad_transfer, "transfer order")],
            cause_code="LATE_TRANSFER_SHIP",
            regime="Transfer Recovery",
        )

    def _script_data_gap(self) -> None:
        object_id = "IL-0008"
        self.emit(
            object_id,
            "Data Gap Started",
            self.at(2, seconds=500),
            updates={"data_complete": False},
            cause_code="DATA_GAP",
            regime="Data Gap",
        )
        self.emit(
            object_id,
            "Data Gap Ended",
            self.at(3, seconds=500),
            updates={"data_complete": True},
            cause_code="DATA_RESTORED",
            regime="Nominal Replenishment",
        )

    def _finalize_lifecycles(self) -> None:
        for index, object_id in enumerate(sorted(self.states)):
            self.emit(
                object_id,
                "Simulation End Snapshot",
                self.end + timedelta(seconds=index),
                cause_code="HORIZON_END",
                regime=self.previous_regime[object_id] or "Nominal Replenishment",
                passive=True,
            )

    def simulate(self) -> None:
        self._initialize_all()
        self._script_demand_surge()
        self._script_supplier_delay()
        self._script_receipt_and_transfer()
        self._script_policy_failure()
        self._script_count_discrepancy()
        self._script_data_gap()
        self._script_remaining_violations()
        self._finalize_lifecycles()
        self.builder.validate()

    def _episodes(
        self, rows: Sequence[Mapping[str, Any]], label_field: str
    ) -> list[dict[str, Any]]:
        grouped: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
        for row in rows:
            grouped[str(row["leading_object_id"])].append(row)
        episodes: list[dict[str, Any]] = []
        for object_id, object_rows in sorted(grouped.items()):
            ordered = sorted(object_rows, key=lambda row: parse_timestamp(str(row["event_time"])))
            start_index = 0
            episode_number = 1
            for index in range(1, len(ordered) + 1):
                boundary = index == len(ordered) or (
                    ordered[index][label_field] != ordered[start_index][label_field]
                )
                if not boundary:
                    continue
                start_row = ordered[start_index]
                last_inside = ordered[index - 1]
                if index < len(ordered):
                    end_row = ordered[index]
                    right_censored = False
                else:
                    end_row = last_inside
                    right_censored = True
                start_time = parse_timestamp(str(start_row["event_time"]))
                end_time = parse_timestamp(str(end_row["event_time"]))
                episodes.append(
                    {
                        "leading_object_id": object_id,
                        "episode_id": f"{object_id}-EP-{episode_number:03d}",
                        "label": start_row[label_field],
                        "start_event_id": start_row["event_id"],
                        "end_event_id": end_row["event_id"],
                        "start_time": str(start_row["event_time"]),
                        "end_time": str(end_row["event_time"]),
                        "duration_minutes": (end_time - start_time).total_seconds() / 60.0,
                        "event_count": index - start_index,
                        "right_censored": right_censored,
                    }
                )
                episode_number += 1
                start_index = index
        return episodes

    def _prediction_samples(self) -> list[dict[str, Any]]:
        grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for row in self.state_rows:
            grouped[str(row["leading_object_id"])].append(row)
        samples: list[dict[str, Any]] = []
        for object_id, rows in grouped.items():
            ordered = sorted(rows, key=lambda row: parse_timestamp(str(row["event_time"])))
            times = [parse_timestamp(str(row["event_time"])) for row in ordered]
            next_shortage: list[int | None] = [None] * len(ordered)
            next_critical: list[int | None] = [None] * len(ordered)
            next_normal: list[int | None] = [None] * len(ordered)
            shortage_index: int | None = None
            critical_index: int | None = None
            normal_index: int | None = None
            for reverse_index in range(len(ordered) - 1, -1, -1):
                next_shortage[reverse_index] = shortage_index
                next_critical[reverse_index] = critical_index
                next_normal[reverse_index] = normal_index
                state = ordered[reverse_index]["reference_state"]
                if state in {"Understock", "Critical Understock"}:
                    shortage_index = reverse_index
                if state == "Critical Understock":
                    critical_index = reverse_index
                if state == "Normal":
                    normal_index = reverse_index
            for index, row in enumerate(ordered):
                cutoff = times[index]
                if row["reference_state"] in {"Normal", "Understock"}:
                    for task, future_index in (
                        ("Understock within 7 days", next_shortage[index]),
                        ("Critical Understock within 7 days", next_critical[index]),
                    ):
                        within_horizon = (
                            future_index is not None
                            and times[future_index] <= cutoff + timedelta(days=7)
                        )
                        samples.append(
                            {
                                "leading_object_id": object_id,
                                "cutoff_event_id": row["event_id"],
                                "cutoff_time": row["event_time"],
                                "current_state": row["reference_state"],
                                "label_name": task,
                                "horizon_minutes": 10080,
                                "label": within_horizon,
                                "time_to_event_minutes": (
                                    (times[future_index] - cutoff).total_seconds() / 60.0
                                    if within_horizon and future_index is not None
                                    else ""
                                ),
                                "split_group": object_id,
                            }
                        )
                shortage = row["reference_state"] in {
                    "Understock",
                    "Critical Understock",
                }
                previous_shortage = index > 0 and ordered[index - 1]["reference_state"] in {
                    "Understock",
                    "Critical Understock",
                }
                if not shortage or previous_shortage:
                    continue
                future_normal_index = next_normal[index]
                recovery_minutes = (
                    (times[future_normal_index] - cutoff).total_seconds() / 60.0
                    if future_normal_index is not None
                    else ""
                )
                common = {
                    "leading_object_id": object_id,
                    "cutoff_event_id": row["event_id"],
                    "cutoff_time": row["event_time"],
                    "current_state": row["reference_state"],
                    "time_to_event_minutes": recovery_minutes,
                    "split_group": object_id,
                }
                samples.extend(
                    (
                        {
                            **common,
                            "label_name": "Recovery to Normal within 3 days",
                            "horizon_minutes": 4320,
                            "label": future_normal_index is not None
                            and float(recovery_minutes) <= 4320,
                        },
                        {
                            **common,
                            "label_name": "Time to stable Normal recovery",
                            "horizon_minutes": int((self.end - cutoff).total_seconds() / 60),
                            "label": future_normal_index is not None,
                        },
                    )
                )
        return samples

    def observed_document(self) -> dict[str, Any]:
        return self.builder.to_dict()

    def perturbation_manifest(self) -> list[dict[str, object]]:
        return []

    def _behavior_document(self, document: dict[str, Any]) -> dict[str, Any]:
        document = {
            **document,
            "events": list(document["events"]),
        }
        document["events"] = [
            event
            for event in document["events"]
            if not next(
                attribute["value"]
                for attribute in event["attributes"]
                if attribute["name"] == "passive_observation"
            )
        ]
        OcelBuilder.from_dict(document, leading_object_type="ItemLocation")
        return document

    def write(self, output_dir: Path, config_path: Path) -> None:
        writer = RunWriter(output_dir)
        writer.prepare()
        observed_document = self.observed_document()
        observed = canonical_json_bytes(observed_document)
        observed_events = observed_event_index(observed_document)
        writer.write_bytes("observed.ocel.json", observed)
        writer.write_json(
            "observed.behavior.ocel.json",
            self._behavior_document(observed_document),
            pretty=False,
        )
        query_path = Path(__file__).parents[2] / "queries" / "inventory_state.sql"
        writer.write_text("state_query.sql", query_path.read_text(encoding="utf-8"))
        ordered_states, observed_transitions = align_state_truth(
            self.state_rows,
            self.transition_rows,
            observed_document,
            unknown_reason="required stock or threshold data are incomplete",
            observed_transition_prefix="INV-OBS-T-",
        )
        ordered_latent = sorted(
            align_event_rows(self.latent_rows, observed_events, retain_unobserved=True),
            key=lambda row: (str(row["leading_object_id"]), str(row["event_time"])),
        )
        writer.write_csv("truth/state_at_event.csv", STATE_TRUTH_FIELDS, ordered_states)
        state_episodes = self._episodes(ordered_states, "reference_state")
        writer.write_csv(
            "truth/state_episodes.csv",
            tuple(state_episodes[0]),
            state_episodes,
        )
        writer.write_csv(
            "truth/transitions.csv", tuple(observed_transitions[0]), observed_transitions
        )
        writer.write_csv("truth/latent_regime_at_event.csv", LATENT_FIELDS, ordered_latent)
        regime_episodes = self._episodes(ordered_latent, "primary_regime")
        writer.write_csv(
            "truth/latent_regime_episodes.csv",
            tuple(regime_episodes[0]),
            regime_episodes,
        )
        writer.write_csv("truth/injected_pattern_instances.csv", PATTERN_FIELDS, self.pattern_rows)
        writer.write_csv(
            "truth/conformance_violations.csv",
            VIOLATION_FIELDS,
            align_event_rows(self.violation_rows, observed_events, retain_unobserved=True),
        )
        prediction_samples = self._prediction_samples()
        state_by_event = {str(row["event_id"]): row for row in ordered_states}
        prediction_samples = [
            {
                **row,
                "cutoff_time": observed_events[str(row["cutoff_event_id"])]["time"],
                "current_state": state_by_event[str(row["cutoff_event_id"])]["reference_state"],
            }
            for row in prediction_samples
            if str(row["cutoff_event_id"]) in observed_events
        ]
        writer.write_csv(
            "truth/prediction_samples.csv",
            tuple(prediction_samples[0]),
            prediction_samples,
        )
        state_counts = Counter(str(row["reference_state"]) for row in ordered_states)
        states_by_object: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for row in ordered_states:
            states_by_object[str(row["leading_object_id"])].append(row)
        outcomes = []
        for object_id in sorted(self.states):
            object_rows = states_by_object[object_id]
            outcomes.append(
                {
                    "leading_object_id": object_id,
                    "final_state": object_rows[-1]["reference_state"],
                    "transition_count": sum(bool(row["is_transition"]) for row in object_rows),
                    "understock_event_count": sum(
                        row["reference_state"] in {"Understock", "Critical Understock"}
                        for row in object_rows
                    ),
                }
            )
        writer.write_csv("truth/outcomes_by_object.csv", tuple(outcomes[0]), outcomes)
        writer.write_json(
            "truth/causal_truth.json",
            inventory_causal_truth(
                self.seed_tree.stream("causal intervention"),
                profile=self.config.profile,
                pairs=max(24, self.config.entities.item_locations),
            ),
        )
        generate_analyst_tasks(output_dir, "inventory")
        event_types = Counter(str(event["type"]) for event in observed_document["events"])
        transition_counts = Counter(
            f"{row['from_state']} -> {row['to_state']}" for row in observed_transitions
        )
        counts = {
            "events": len(observed_document["events"]),
            "objects": len(observed_document["objects"]),
            "e2o": sum(len(event["relationships"]) for event in observed_document["events"]),
            "o2o": sum(len(item.get("relationships", [])) for item in observed_document["objects"]),
            "leading_objects": len(self.states),
            "event_types": len(self.builder.event_types),
            "object_types": len(self.builder.object_types),
        }
        summary = {
            "counts": counts,
            "state_counts": dict(sorted(state_counts.items())),
            "transition_counts": dict(sorted(transition_counts.items())),
            "event_type_counts": dict(sorted(event_types.items())),
            "pattern_instances": len(self.pattern_rows),
            "conformance_violations": len(self.violation_rows),
        }
        writer.write_json("expected/summary.json", summary)
        writer.write_json(
            "expected/branch_coverage.json",
            {
                "states": {state: state_counts[state] for state in INVENTORY_STATES},
                "state_branches_covered": all(
                    state_counts[state] > 0 for state in INVENTORY_STATES
                ),
                "event_types_exercised": dict(sorted(event_types.items())),
                "pattern_ids": [row["pattern_id"] for row in self.pattern_rows],
                "conformance_rule_ids": sorted({row["rule_id"] for row in self.violation_rows}),
            },
        )
        compressed_sequences: dict[str, list[str]] = {}
        for object_id in sorted(self.states):
            sequence: list[str] = []
            for row in states_by_object[object_id]:
                state = str(row["reference_state"])
                if not sequence or sequence[-1] != state:
                    sequence.append(state)
            compressed_sequences[object_id] = sequence
        writer.write_json(
            "expected/golden_assertions.json",
            {
                "observed_sha256": sha256_bytes(observed),
                "state_sequences": compressed_sequences,
                "transition_counts": dict(sorted(transition_counts.items())),
                "pattern_support": {
                    pattern_id: sum(row["pattern_id"] == pattern_id for row in self.pattern_rows)
                    for pattern_id in (f"INV-P{index}" for index in range(1, 7))
                },
                "conformance_rule_counts": dict(
                    sorted(Counter(row["rule_id"] for row in self.violation_rows).items())
                ),
                "prediction_positive_count": sum(bool(row["label"]) for row in prediction_samples),
            },
        )
        raw_config = load_yaml(config_path)
        writer.write_manifest(
            scenario="inventory",
            profile=self.config.profile,
            seed=self.config.seed,
            config_sha256=config_sha256(raw_config),
            generator_commit=repository_commit(Path(__file__).parents[2]),
            start_time=self.start,
            end_time=self.end,
            counts=counts,
            rng_streams=self.seed_tree.metadata(),
            expected_counts={
                "states": len(ordered_states),
                "transitions": len(observed_transitions),
                "patterns": len(self.pattern_rows),
                "violations": len(self.violation_rows),
            },
            perturbations=self.perturbation_manifest(),
            extra={"run_id": f"inventory-{self.config.profile}-{self.config.seed}"},
        )


def generate_inventory_golden(config: InventoryConfig, config_path: Path, output_dir: Path) -> None:
    simulation = InventoryGoldenSimulation(config)
    simulation.simulate()
    simulation.write(output_dir, config_path)
