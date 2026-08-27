"""Stochastic inventory smoke and paper profile simulation."""

from __future__ import annotations

import heapq
import math
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, ClassVar

from saocpm_eval.common.ocel_builder import Relationship, format_timestamp
from saocpm_eval.common.perturbations import (
    delete_context_relationships,
    jitter_event_timestamps,
    mask_event_attributes,
)
from saocpm_eval.inventory.config import InventoryConfig
from saocpm_eval.inventory.demand import daily_demand
from saocpm_eval.inventory.replenishment import (
    periodic_order_quantity,
    sample_supplier_lead_time_days,
)
from saocpm_eval.inventory.simulation import InventoryGoldenSimulation


class InventoryStochasticSimulation(InventoryGoldenSimulation):
    """Golden coverage plus heterogeneous stochastic lifecycles and forced support."""

    PATTERN_TO_FORCED: ClassVar[dict[str, str]] = {
        "INV-P1": "demand_surge_without_inbound",
        "INV-P2": "supplier_delay",
        "INV-P3": "receipt_driven_excess",
        "INV-P4": "transfer_recovery",
        "INV-P5": "policy_failure",
        "INV-P6": "count_discrepancy",
    }

    def __init__(self, config: InventoryConfig) -> None:
        if config.profile not in {"smoke", "paper"}:
            raise ValueError("stochastic inventory simulation requires smoke or paper profile")
        super().__init__(config)
        self.forced_cursor = self.at(10)
        self.forced_rng = self.seed_tree.stream("forced mechanisms")

    def _next_slot(self) -> datetime:
        current = self.forced_cursor
        self.forced_cursor += timedelta(hours=6)
        if self.forced_cursor >= self.end - timedelta(days=2):
            raise ValueError("configured forced episodes do not fit inside the simulation horizon")
        return current

    def _object_for_instance(self, instance: int, offset: int = 0) -> str:
        count = self.config.entities.item_locations
        return f"IL-{((instance + offset - 1) % count) + 1:04d}"

    def _reset(
        self,
        object_id: str,
        time: datetime,
        on_hand: float,
        *,
        lower: float = 20.0,
        upper: float = 80.0,
    ) -> None:
        state = self.states[object_id]
        self.emit(
            object_id,
            "Inventory Adjustment",
            time,
            quantity=on_hand - state.on_hand,
            updates={
                "reserved": 0.0,
                "backorder": 0.0,
                "on_order": 0.0,
                "confirmed_demand_horizon": 0.0,
                "inbound_horizon": 0.0,
                "lower_threshold": lower,
                "upper_threshold": upper,
                "data_complete": True,
            },
            cause_code="FORCED_EPISODE_RESET",
            regime="Nominal Replenishment",
        )

    def _noise(self, object_id: str, time: datetime, regime: str, enabled: bool) -> None:
        if enabled:
            self.emit(
                object_id,
                "Cycle Count Performed",
                time,
                cause_code="PATTERN_NOISE",
                regime=regime,
            )

    def _pattern_flags(self, instance: int) -> tuple[bool, float]:
        guaranteed = instance <= self.config.patterns.exact_behavior_instances_per_pattern
        noisy = not guaranteed and (
            float(self.forced_rng.random()) < self.config.patterns.noise_event_probability
        )
        return noisy, 1.0 if noisy else 0.0

    def _inject_p1(self, instance: int) -> None:
        base = self._next_slot()
        object_id = self._object_for_instance(instance)
        key = f"INV-P1#{instance:03d}"
        noisy, noise_level = self._pattern_flags(instance)
        self._reset(object_id, base, 50.0)
        sales_time = base + timedelta(seconds=10)
        sales_id = self.new_context(
            "SalesOrderItem",
            sales_time,
            {
                "requested_quantity": 45.0,
                "due_time": format_timestamp(base + timedelta(days=1)),
                "priority": "urgent",
                "customer_class": "A",
            },
            [Relationship(object_id, "consumes-from")],
        )
        context = [(sales_id, "sales order item")]
        self.emit(
            object_id,
            "Sales Order Item Created",
            sales_time,
            context=context,
            cause_code="FORCED_DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id=key,
        )
        self.emit(
            object_id,
            "Reservation Created",
            base + timedelta(seconds=20),
            quantity=45.0,
            updates={"reserved": 45.0},
            context=context,
            cause_code="FORCED_DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id=key,
        )
        self._noise(
            object_id,
            base + timedelta(seconds=25),
            "Demand Surge Without Inbound",
            noisy,
        )
        self.emit(
            object_id,
            "Goods Issue",
            base + timedelta(seconds=30),
            quantity=35.0,
            updates={"reserved": 0.0, "confirmed_demand_horizon": 4.0},
            context=context,
            cause_code="FORCED_DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id=key,
        )
        self.emit(
            object_id,
            "Backorder Registered",
            base + timedelta(seconds=40),
            quantity=30.0,
            updates={"backorder": 30.0, "confirmed_demand_horizon": 30.0},
            context=context,
            cause_code="FORCED_DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id=key,
        )
        proposal_id = self.new_context(
            "ReplenishmentProposal",
            base + timedelta(seconds=50),
            {
                "suggested_quantity": 70.0,
                "reason": "forced shortage",
                "creation_time": format_timestamp(base + timedelta(seconds=50)),
                "status": "created",
            },
            [Relationship(object_id, "replenishes")],
        )
        self.emit(
            object_id,
            "Replenishment Proposal Created",
            base + timedelta(seconds=50),
            quantity=70.0,
            context=[(proposal_id, "proposal"), ("PLN-001", "planner")],
            cause_code="FORCED_DEMAND_SURGE",
            regime="Demand Surge Without Inbound",
            pattern_id=key,
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
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_p2(self, instance: int) -> None:
        base = self._next_slot()
        object_id = self._object_for_instance(instance, 7)
        key = f"INV-P2#{instance:03d}"
        noisy, noise_level = self._pattern_flags(instance)
        self._reset(object_id, base, 10.0)
        po_id = self.new_context(
            "PurchaseOrderItem",
            base + timedelta(seconds=10),
            {
                "ordered_quantity": 40.0,
                "confirmed_quantity": 40.0,
                "planned_receipt_time": format_timestamp(base + timedelta(hours=1)),
                "actual_receipt_time": format_timestamp(base + timedelta(hours=4)),
                "status": "received late",
            },
            [Relationship("SUP-001", "supplier"), Relationship(object_id, "replenishes")],
        )
        delivery_id = self.new_context(
            "Delivery",
            base + timedelta(seconds=10),
            {"carrier": "FORCED", "shipment_status": "delayed", "delay_code": "forced"},
        )
        context = [
            (po_id, "purchase order item"),
            (delivery_id, "delivery"),
            ("SUP-001", "supplier"),
            ("PLN-001", "planner"),
        ]
        events = (
            ("Purchase Order Item Created", timedelta(seconds=10), 40.0),
            ("Supplier Confirmation Received", timedelta(seconds=20), 0.0),
            ("Delivery Delayed", timedelta(hours=1), 0.0),
            ("Expedite Requested", timedelta(hours=2), 0.0),
            ("Goods Receipt", timedelta(hours=4), 40.0),
        )
        for index, (event_type, offset, quantity) in enumerate(events):
            if index == 2:
                self._noise(object_id, base + timedelta(minutes=30), "Supplier Delay", noisy)
            self.emit(
                object_id,
                event_type,
                base + offset,
                quantity=quantity,
                context=context,
                cause_code="FORCED_SUPPLIER_DELAY",
                regime=(
                    "Supplier Delay" if event_type != "Goods Receipt" else "Nominal Replenishment"
                ),
                pattern_id=key,
            )
        self.add_pattern(
            "INV-P2",
            object_id,
            family="intra",
            from_state="Understock",
            to_state="Normal",
            sequence=tuple(item[0] for item in events),
            object_types=("ItemLocation", "PurchaseOrderItem", "Delivery", "Supplier", "Planner"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_p3(self, instance: int) -> None:
        base = self._next_slot()
        object_id = self._object_for_instance(instance, 14)
        key = f"INV-P3#{instance:03d}"
        noisy, noise_level = self._pattern_flags(instance)
        self._reset(object_id, base, 40.0)
        po_id = self.new_context(
            "PurchaseOrderItem",
            base + timedelta(seconds=1),
            {
                "ordered_quantity": 20.0,
                "confirmed_quantity": 20.0,
                "planned_receipt_time": format_timestamp(base + timedelta(seconds=10)),
                "actual_receipt_time": format_timestamp(base + timedelta(seconds=10)),
                "status": "over-delivered",
            },
            [Relationship("SUP-002", "supplier"), Relationship(object_id, "replenishes")],
        )
        self.emit(
            object_id,
            "Purchase Order Item Created",
            base + timedelta(seconds=1),
            quantity=20.0,
            context=[(po_id, "purchase order item")],
            cause_code="FORCED_PRECONDITION",
            regime="Nominal Replenishment",
        )
        self.emit(
            object_id,
            "Goods Receipt",
            base + timedelta(seconds=10),
            quantity=70.0,
            context=[(po_id, "purchase order item")],
            cause_code="FORCED_OVER_DELIVERY",
            regime="Receipt-Driven Excess",
            pattern_id=key,
        )
        self._noise(object_id, base + timedelta(seconds=15), "Receipt-Driven Excess", noisy)
        self.emit(
            object_id,
            "Policy Threshold Updated",
            base + timedelta(seconds=20),
            updates={"upper_threshold": 90.0},
            cause_code="CAPACITY_ALERT",
            regime="Receipt-Driven Excess",
            pattern_id=key,
        )
        self.emit(
            object_id,
            "Transfer Requested",
            base + timedelta(seconds=30),
            quantity=30.0,
            cause_code="EXCESS_MITIGATION",
            regime="Receipt-Driven Excess",
            pattern_id=key,
        )
        self.add_pattern(
            "INV-P3",
            object_id,
            family="inter",
            from_state="Normal",
            to_state="Overstock",
            sequence=("Goods Receipt", "Policy Threshold Updated", "Transfer Requested"),
            object_types=("ItemLocation", "PurchaseOrderItem", "TransferOrder", "Planner"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_p4(self, instance: int) -> None:
        base = self._next_slot()
        source = self._object_for_instance(instance, 21)
        target = self._object_for_instance(instance, 22)
        key = f"INV-P4#{instance:03d}"
        noisy, noise_level = self._pattern_flags(instance)
        self._reset(source, base, 70.0)
        self._reset(target, base + timedelta(seconds=1), 10.0)
        transfer_id = self.new_context(
            "TransferOrder",
            base + timedelta(seconds=10),
            {
                "quantity": 30.0,
                "source_item_location": source,
                "target_item_location": target,
                "status": "received",
            },
            [Relationship(source, "source"), Relationship(target, "target")],
        )
        request = self.emit(
            source,
            "Transfer Requested",
            base + timedelta(seconds=10),
            quantity=30.0,
            context=[(transfer_id, "transfer order")],
            cause_code="FORCED_TRANSFER",
            regime="Transfer Recovery",
        )
        self.pattern_event_ids[key].append(request)
        self._noise(source, base + timedelta(seconds=15), "Transfer Recovery", noisy)
        ship = self.emit(
            source,
            "Transfer Ship",
            base + timedelta(seconds=20),
            quantity=30.0,
            context=[(transfer_id, "transfer order")],
            cause_code="FORCED_TRANSFER",
            regime="Transfer Recovery",
        )
        self.pattern_event_ids[key].append(ship)
        receive = self.emit(
            target,
            "Transfer Receive",
            base + timedelta(seconds=30),
            quantity=30.0,
            context=[(transfer_id, "transfer order")],
            cause_code="FORCED_TRANSFER",
            regime="Transfer Recovery",
        )
        self.pattern_event_ids[key].append(receive)
        self.add_pattern(
            "INV-P4",
            target,
            family="inter",
            from_state="Understock",
            to_state="Normal",
            sequence=("Transfer Requested", "Transfer Ship", "Transfer Receive"),
            object_types=("ItemLocation", "TransferOrder", "Location"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_p5(self, instance: int) -> None:
        base = self._next_slot()
        object_id = self._object_for_instance(instance, 28)
        key = f"INV-P5#{instance:03d}"
        noisy, noise_level = self._pattern_flags(instance)
        self._reset(object_id, base, 50.0)
        issue = self.emit(
            object_id,
            "Goods Issue",
            base + timedelta(seconds=10),
            quantity=35.0,
            updates={"confirmed_demand_horizon": 4.0},
            cause_code="FORCED_POLICY_FAILURE",
            regime="Demand Surge Without Inbound",
            pattern_id=key,
        )
        self._noise(
            object_id,
            base + timedelta(seconds=15),
            "Demand Surge Without Inbound",
            noisy,
        )
        critical_time = base + timedelta(seconds=20)
        critical = self.emit(
            object_id,
            "Backorder Registered",
            critical_time,
            quantity=20.0,
            updates={"backorder": 20.0, "confirmed_demand_horizon": 25.0},
            cause_code="FORCED_POLICY_FAILURE",
            regime="Demand Surge Without Inbound",
            pattern_id=key,
        )
        self.add_violation(
            "INV-C1",
            object_id,
            critical,
            critical_time,
            deadline=critical_time + timedelta(hours=self.config.policy.critical_action_sla_hours),
            details={"reason": "forced episode has no timely replenishment action"},
        )
        self.add_pattern(
            "INV-P5",
            object_id,
            family="inter",
            from_state="Normal",
            to_state="Critical Understock",
            sequence=("Goods Issue", "Backorder Registered"),
            object_types=("ItemLocation",),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )
        del issue

    def _inject_p6(self, instance: int) -> None:
        base = self._next_slot()
        object_id = self._object_for_instance(instance, 35)
        key = f"INV-P6#{instance:03d}"
        noisy, noise_level = self._pattern_flags(instance)
        self._reset(object_id, base, 100.0)
        self.emit(
            object_id,
            "Cycle Count Performed",
            base + timedelta(seconds=10),
            quantity=50.0,
            cause_code="FORCED_COUNT_DISCREPANCY",
            regime="Count Discrepancy",
            pattern_id=key,
        )
        self._noise(object_id, base + timedelta(seconds=15), "Count Discrepancy", noisy)
        self.emit(
            object_id,
            "Inventory Adjustment",
            base + timedelta(seconds=20),
            quantity=-50.0,
            cause_code="FORCED_COUNT_CORRECTION",
            regime="Count Discrepancy",
            pattern_id=key,
        )
        self.add_pattern(
            "INV-P6",
            object_id,
            family="intra",
            from_state="Overstock",
            to_state="Normal",
            sequence=("Cycle Count Performed", "Inventory Adjustment"),
            object_types=("ItemLocation", "Planner"),
            event_key=key,
            instance_number=instance,
            noise_level=noise_level,
            exact=not noisy,
        )

    def _inject_forced_support(self) -> None:
        injectors = {
            "INV-P1": self._inject_p1,
            "INV-P2": self._inject_p2,
            "INV-P3": self._inject_p3,
            "INV-P4": self._inject_p4,
            "INV-P5": self._inject_p5,
            "INV-P6": self._inject_p6,
        }
        for pattern_id, forced_name in self.PATTERN_TO_FORCED.items():
            target = self.config.forced_episodes.get(forced_name, 0)
            for instance in range(2, target + 1):
                injectors[pattern_id](instance)
        for instance in range(2, self.config.forced_episodes.get("data_gap", 0) + 1):
            base = self._next_slot()
            object_id = self._object_for_instance(instance, 42)
            self.emit(
                object_id,
                "Data Gap Started",
                base + timedelta(seconds=10),
                updates={"data_complete": False},
                cause_code="FORCED_DATA_GAP",
                regime="Data Gap",
            )
            self.emit(
                object_id,
                "Data Gap Ended",
                base + timedelta(hours=1),
                updates={"data_complete": True},
                cause_code="FORCED_DATA_RESTORED",
                regime="Nominal Replenishment",
            )

    def _demand_quantity(self, demand_class: str, rate: float, day_index: int) -> float:
        return daily_demand(
            self.seed_tree.stream("exogenous demand or production schedule"),
            demand_class,
            rate,
            day_index,
        )

    def _simulate_background_for_object(
        self, object_id: str, start_time: datetime, object_index: int
    ) -> None:
        demand_rng = self.seed_tree.stream("exogenous demand or production schedule")
        response_rng = self.seed_tree.stream("supplier or maintenance response")
        demand_classes = tuple(self.config.demand.class_mix)
        demand_class = demand_classes[object_index % len(demand_classes)]
        low_rate, high_rate = self.config.demand.base_daily_rate
        if low_rate == high_rate:
            rate = low_rate
        else:
            rate = float(
                math.exp(demand_rng.uniform(math.log(max(low_rate, 1e-6)), math.log(high_rate)))
            )
        review_low, review_high = self.config.policy.review_interval_days
        review_interval = int(demand_rng.integers(review_low, review_high + 1))
        queue: list[tuple[datetime, int, str, dict[str, Any]]] = []
        sequence = 0

        def schedule(time: datetime, kind: str, payload: dict[str, Any] | None = None) -> None:
            nonlocal sequence
            sequence += 1
            heapq.heappush(queue, (time, sequence, kind, payload or {}))

        first_day = math.ceil((start_time - self.start).total_seconds() / 86400.0)
        last_day = self.config.horizon_days - 1
        for day in range(first_day, last_day):
            schedule(self.at(day) + timedelta(hours=8), "demand", {"day": day})
            if (day - first_day) % review_interval == 0:
                schedule(self.at(day) + timedelta(hours=18), "review")
        while queue:
            time, _, kind, payload = heapq.heappop(queue)
            if time >= self.end - timedelta(minutes=1):
                continue
            state = self.states[object_id]
            if kind == "demand":
                quantity = self._demand_quantity(demand_class, rate, int(payload["day"]))
                if quantity <= 0:
                    continue
                sales_id = self.new_context(
                    "SalesOrderItem",
                    time,
                    {
                        "requested_quantity": quantity,
                        "due_time": format_timestamp(time + timedelta(days=1)),
                        "priority": "standard",
                        "customer_class": "stochastic",
                    },
                    [Relationship(object_id, "consumes-from")],
                )
                context = [(sales_id, "sales order item")]
                self.emit(
                    object_id,
                    "Sales Order Item Created",
                    time,
                    quantity=quantity,
                    updates={"demand_estimate": rate},
                    context=context,
                    cause_code="STOCHASTIC_DEMAND",
                    regime="Nominal Replenishment",
                )
                available = min(state.on_hand, quantity)
                if available > 0:
                    self.emit(
                        object_id,
                        "Reservation Created",
                        time + timedelta(seconds=1),
                        quantity=available,
                        updates={"reserved": state.reserved + available},
                        context=context,
                        cause_code="STOCHASTIC_DEMAND",
                        regime="Nominal Replenishment",
                    )
                    self.emit(
                        object_id,
                        "Goods Issue",
                        time + timedelta(seconds=2),
                        quantity=available,
                        updates={"reserved": max(0.0, state.reserved - available)},
                        context=context,
                        cause_code="STOCHASTIC_DEMAND",
                        regime="Nominal Replenishment",
                    )
                shortage = quantity - available
                if shortage > 0:
                    self.emit(
                        object_id,
                        "Backorder Registered",
                        time + timedelta(seconds=3),
                        quantity=shortage,
                        updates={
                            "backorder": state.backorder + shortage,
                            "confirmed_demand_horizon": state.backorder + shortage,
                        },
                        context=context,
                        cause_code="STOCHASTIC_SHORTAGE",
                        regime="Demand Surge Without Inbound",
                    )
            elif kind == "review" and state.inventory_position <= state.lower_threshold:
                quantity = periodic_order_quantity(
                    state.inventory_position,
                    state.lower_threshold,
                    state.upper_threshold,
                    float(self.config.policy.minimum_order_quantity[0]),
                    float(self.config.policy.lot_size[0]),
                )
                proposal_id = self.new_context(
                    "ReplenishmentProposal",
                    time,
                    {
                        "suggested_quantity": quantity,
                        "reason": "periodic review",
                        "creation_time": format_timestamp(time),
                        "status": "created",
                    },
                    [Relationship(object_id, "replenishes")],
                )
                self.emit(
                    object_id,
                    "Replenishment Proposal Created",
                    time,
                    quantity=quantity,
                    context=[(proposal_id, "proposal"), ("PLN-001", "planner")],
                    cause_code="PERIODIC_REVIEW",
                    regime="Nominal Replenishment",
                )
                delay_low, delay_high = self.config.policy.planner_approval_delay_hours
                delay = float(response_rng.uniform(delay_low, delay_high))
                schedule(
                    time + timedelta(hours=delay),
                    "approve",
                    {"proposal_id": proposal_id, "quantity": quantity},
                )
            elif kind == "approve":
                quantity = float(payload["quantity"])
                proposal_id = str(payload["proposal_id"])
                self.emit(
                    object_id,
                    "Replenishment Proposal Approved",
                    time,
                    quantity=quantity,
                    context=[(proposal_id, "proposal"), ("PLN-001", "planner")],
                    cause_code="STOCHASTIC_APPROVAL",
                    regime="Nominal Replenishment",
                )
                lead_low, lead_high = self.config.supply.lead_time_days
                mean_lead = float(response_rng.uniform(lead_low, lead_high))
                cv_low, cv_high = self.config.supply.lead_time_cv
                cv = float(response_rng.uniform(cv_low, cv_high))
                lead_days = sample_supplier_lead_time_days(response_rng, mean_lead, cv)
                receipt_time = time + timedelta(days=lead_days)
                po_id = self.new_context(
                    "PurchaseOrderItem",
                    time + timedelta(seconds=1),
                    {
                        "ordered_quantity": quantity,
                        "confirmed_quantity": quantity,
                        "planned_receipt_time": format_timestamp(receipt_time),
                        "actual_receipt_time": format_timestamp(receipt_time),
                        "status": "open",
                    },
                    [Relationship("SUP-001", "supplier"), Relationship(object_id, "replenishes")],
                )
                timely = (
                    quantity
                    if lead_days <= self.config.policy.critical_demand_horizon_days
                    else 0.0
                )
                self.emit(
                    object_id,
                    "Purchase Order Item Created",
                    time + timedelta(seconds=1),
                    quantity=quantity,
                    updates={"inbound_horizon": state.inbound_horizon + timely},
                    context=[(po_id, "purchase order item"), ("SUP-001", "supplier")],
                    cause_code="STOCHASTIC_ORDER",
                    regime="Replenishment In Transit",
                )
                self.emit(
                    object_id,
                    "Supplier Confirmation Received",
                    time + timedelta(seconds=2),
                    context=[(po_id, "purchase order item"), ("SUP-001", "supplier")],
                    cause_code="STOCHASTIC_CONFIRMATION",
                    regime="Replenishment In Transit",
                )
                schedule(
                    receipt_time,
                    "receipt",
                    {"po_id": po_id, "quantity": quantity, "timely": timely},
                )
            elif kind == "receipt":
                quantity = float(payload["quantity"])
                fill_low, fill_high = self.config.supply.fill_rate
                accepted = quantity * float(response_rng.uniform(fill_low, fill_high))
                po_id = str(payload["po_id"])
                delivery_id = self.new_context(
                    "Delivery",
                    time,
                    {"carrier": "SIM", "shipment_status": "received", "delay_code": "none"},
                )
                context = [(po_id, "purchase order item"), (delivery_id, "delivery")]
                if float(response_rng.random()) < self.config.supply.rejection_probability:
                    self.emit(
                        object_id,
                        "Receipt Rejected",
                        time,
                        quantity=accepted,
                        context=context,
                        cause_code="QUALITY_REJECTION",
                        regime="Supplier Delay",
                    )
                else:
                    remaining_backorder = max(0.0, state.backorder - accepted)
                    self.emit(
                        object_id,
                        "Goods Receipt",
                        time,
                        quantity=accepted,
                        updates={
                            "confirmed_demand_horizon": remaining_backorder,
                            "inbound_horizon": max(
                                0.0, state.inbound_horizon - float(payload["timely"])
                            ),
                        },
                        context=context,
                        cause_code="STOCHASTIC_RECEIPT",
                        regime="Nominal Replenishment",
                    )

    def _simulate_background(self) -> None:
        background_start = self.forced_cursor + timedelta(days=1)
        for index, object_id in enumerate(sorted(self.states)):
            self._simulate_background_for_object(object_id, background_start, index)

    def simulate(self) -> None:
        self._initialize_all()
        self._script_demand_surge()
        self._script_supplier_delay()
        self._script_receipt_and_transfer()
        self._script_policy_failure()
        self._script_count_discrepancy()
        self._script_data_gap()
        self._script_remaining_violations()
        self._inject_forced_support()
        self._simulate_background()
        self._finalize_lifecycles()
        self.builder.validate()

    def observed_document(self) -> dict[str, Any]:
        document = super().observed_document()
        missingness = self.config.missingness
        if missingness.event_attribute_mcar:
            document = mask_event_attributes(
                document,
                missingness.event_attribute_mcar,
                self.seed_tree.stream("missingness and corruption"),
            )
        if missingness.relationship_mcar:
            document = delete_context_relationships(
                document,
                missingness.relationship_mcar,
                self.seed_tree.stream("missingness and corruption"),
                leading_object_type="ItemLocation",
            )
        if missingness.timestamp_jitter_minutes:
            document = jitter_event_timestamps(
                document,
                missingness.timestamp_jitter_minutes,
                self.seed_tree.stream("timestamp jitter"),
                leading_object_type="ItemLocation",
            )
        return document

    def perturbation_manifest(self) -> list[dict[str, object]]:
        missingness = self.config.missingness
        return [
            {
                "id": "profile-observation-noise",
                "event_attribute_mcar": missingness.event_attribute_mcar,
                "relationship_mcar": missingness.relationship_mcar,
                "timestamp_jitter_minutes": missingness.timestamp_jitter_minutes,
            }
        ]


def generate_inventory_stochastic(
    config: InventoryConfig, config_path: Path, output_dir: Path
) -> None:
    simulation = InventoryStochasticSimulation(config)
    simulation.simulate()
    simulation.write(output_dir, config_path)
