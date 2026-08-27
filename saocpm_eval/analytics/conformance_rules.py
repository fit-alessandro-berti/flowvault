"""Independent scenario conformance detectors over the observed OCEL."""

from __future__ import annotations

from bisect import bisect_right
from collections import defaultdict
from datetime import timedelta
from typing import Any

from saocpm_eval.analytics.conformance import Violation
from saocpm_eval.common.ocel_builder import parse_timestamp


def _attributes(item: dict[str, Any]) -> dict[str, Any]:
    return {attribute["name"]: attribute["value"] for attribute in item["attributes"]}


class _Index:
    def __init__(self, document: dict[str, Any], leading_type: str) -> None:
        self.objects = {item["id"]: item for item in document["objects"]}
        self.object_types = {identifier: item["type"] for identifier, item in self.objects.items()}
        self.events = sorted(
            document["events"], key=lambda event: (parse_timestamp(event["time"]), event["id"])
        )
        self.lifecycle: dict[str, list[dict[str, Any]]] = defaultdict(list)
        self.events_by_object: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for event in self.events:
            for relationship in event["relationships"]:
                self.events_by_object[relationship["objectId"]].append(event)
            leading = self.related(event, leading_type)
            if len(leading) == 1:
                self.lifecycle[leading[0]].append(event)

    def related(self, event: dict[str, Any], object_type: str) -> list[str]:
        return [
            relationship["objectId"]
            for relationship in event["relationships"]
            if self.object_types[relationship["objectId"]] == object_type
        ]

    def object_attribute(self, object_id: str, name: str) -> Any:
        values = [
            attribute["value"]
            for attribute in self.objects[object_id].get("attributes", [])
            if attribute["name"] == name
        ]
        return values[-1] if values else None


def _violation(rule: str, object_id: str, event: dict[str, Any]) -> Violation:
    return Violation(rule, object_id, parse_timestamp(event["time"]), event["id"])


def _inventory(index: _Index, profile: str) -> list[Violation]:
    critical_sla = timedelta(hours=12 if profile == "paper" else 8)
    planner_sla = timedelta(hours=72 if profile == "paper" else 24)
    result: list[Violation] = []
    for object_id, events in sorted(index.lifecycle.items()):
        previous_critical = False
        positions_by_type: dict[str, list[int]] = defaultdict(list)
        position_by_event: dict[str, int] = {}
        for event_position, lifecycle_event in enumerate(events):
            positions_by_type[lifecycle_event["type"]].append(event_position)
            position_by_event[lifecycle_event["id"]] = event_position
        planner_purchase_positions = [
            event_position
            for event_position in positions_by_type["Purchase Order Item Created"]
            if _attributes(events[event_position]).get("cause_code") == "PLANNER_ORDER"
        ]
        for position, event in enumerate(events):
            attributes = _attributes(event)
            event_time = parse_timestamp(event["time"])
            critical = bool(attributes.get("critical_understock", False))
            if critical and not previous_critical:
                proposal_positions = positions_by_type["Replenishment Proposal Created"]
                proposal_index = bisect_right(proposal_positions, position)
                proposal = (
                    events[proposal_positions[proposal_index]]
                    if proposal_index < len(proposal_positions)
                    and parse_timestamp(events[proposal_positions[proposal_index]]["time"])
                    <= event_time + critical_sla
                    else None
                )
                if proposal is None:
                    result.append(_violation("INV-C1", object_id, event))
            previous_critical = critical
            if event["type"] == "Replenishment Proposal Approved":
                conversions = []
                planner_index = bisect_right(planner_purchase_positions, position)
                if planner_index < len(planner_purchase_positions):
                    conversions.append(events[planner_purchase_positions[planner_index]])
                for proposal_id in index.related(event, "ReplenishmentProposal"):
                    conversions.extend(
                        candidate
                        for candidate in index.events_by_object[proposal_id]
                        if candidate["type"] == "Purchase Order Item Created"
                        and position_by_event.get(candidate["id"], -1) > position
                    )
                conversion = min(
                    conversions,
                    key=lambda candidate: (
                        parse_timestamp(candidate["time"]),
                        candidate["id"],
                    ),
                    default=None,
                )
                if conversion is None:
                    result.append(_violation("INV-C2", object_id, event))
                elif parse_timestamp(conversion["time"]) > event_time + planner_sla:
                    result.append(_violation("INV-C2", object_id, conversion))
            if event["type"] == "Goods Receipt":
                for purchase_order in index.related(event, "PurchaseOrderItem"):
                    creation = next(
                        (
                            candidate
                            for candidate in index.events_by_object[purchase_order]
                            if candidate["type"] == "Purchase Order Item Created"
                        ),
                        None,
                    )
                    if creation is None or parse_timestamp(creation["time"]) > event_time:
                        result.append(_violation("INV-C3", object_id, event))
                        break
                if attributes.get("cause_code") in {
                    "DUPLICATE_RECEIPT",
                    "OVER_DELIVERY",
                }:
                    result.append(_violation("INV-C6", object_id, event))
            if event["type"] == "Transfer Receive":
                transfer_orders = index.related(event, "TransferOrder")
                shipped = any(
                    candidate["type"] == "Transfer Ship"
                    and parse_timestamp(candidate["time"]) <= event_time
                    for transfer_order in transfer_orders
                    for candidate in index.events_by_object[transfer_order]
                )
                if not shipped:
                    result.append(_violation("INV-C4", object_id, event))
            if (
                event["type"] == "Purchase Order Item Created"
                and float(attributes.get("inventory_position_after", 0.0))
                > float(attributes.get("lower_threshold", 0.0))
                and attributes.get("cause_code") == "NO_EXCEPTION"
            ):
                result.append(_violation("INV-C5", object_id, event))
    return result


def _inspection_passed(index: _Index, event: dict[str, Any]) -> bool:
    return any(
        str(index.object_attribute(identifier, "result")).lower() == "passed"
        for identifier in index.related(event, "Inspection")
    )


def _manufacturing(index: _Index, profile: str) -> list[Violation]:
    critical_request_sla = timedelta(minutes=15)
    stable_minutes = 180 if profile == "paper" else 120
    down_slas = {
        "high": 60,
        "medium": 240 if profile == "paper" else 180,
        "low": 720 if profile == "paper" else 480,
    }
    result: list[Violation] = []
    for object_id, events in sorted(index.lifecycle.items()):
        criticality = str(index.object_attribute(object_id, "criticality") or "high").lower()
        down_sla = timedelta(minutes=down_slas.get(criticality, down_slas["high"]))
        positions_by_type: dict[str, list[int]] = defaultdict(list)
        for event_position, lifecycle_event in enumerate(events):
            positions_by_type[lifecycle_event["type"]].append(event_position)
        safe_restart_evidence = False
        for position, event in enumerate(events):
            attributes = _attributes(event)
            event_time = parse_timestamp(event["time"])
            if event["type"] == "Automatic Stop":
                safe_restart_evidence = False
            elif event["type"] == "Test Run Completed" or (
                event["type"] == "Inspection Performed" and _inspection_passed(index, event)
            ):
                safe_restart_evidence = True
            if event["type"] == "Critical Alarm Raised" and not bool(
                attributes.get("maintenance_open", False)
            ):
                request_positions = positions_by_type["Maintenance Request Created"]
                request_index = bisect_right(request_positions, position)
                request = (
                    events[request_positions[request_index]]
                    if request_index < len(request_positions)
                    and parse_timestamp(events[request_positions[request_index]]["time"])
                    <= event_time + critical_request_sla
                    else None
                )
                if request is None:
                    result.append(_violation("MFG-C1", object_id, event))
            if event["type"] == "Automatic Stop":
                start_positions = positions_by_type["Maintenance Started"]
                start_index = bisect_right(start_positions, position)
                maintenance_start = (
                    events[start_positions[start_index]]
                    if start_index < len(start_positions)
                    else None
                )
                if maintenance_start is None:
                    result.append(_violation("MFG-C2", object_id, event))
                elif parse_timestamp(maintenance_start["time"]) > event_time + down_sla:
                    result.append(_violation("MFG-C2", object_id, maintenance_start))
            if event["type"] == "Machine Restarted" and not safe_restart_evidence:
                result.append(_violation("MFG-C3", object_id, event))
            if (
                event["type"] == "Maintenance Completed"
                and int(attributes.get("stable_run_minutes", 0)) < stable_minutes
            ):
                result.append(_violation("MFG-C4", object_id, event))
            if event["type"] == "Quality Hold Released" and not _inspection_passed(index, event):
                result.append(_violation("MFG-C5", object_id, event))
            if event["type"] == "Component Replaced" and (
                not index.related(event, "WorkOrder")
                or not bool(attributes.get("maintenance_open", False))
            ):
                result.append(_violation("MFG-C6", object_id, event))
    return result


def detect_conformance(
    document: dict[str, Any], scenario: str, *, profile: str = "smoke"
) -> list[Violation]:
    """Detect all pre-registered rules without consulting injected truth sidecars."""

    if scenario == "inventory":
        return _inventory(_Index(document, "ItemLocation"), profile)
    if scenario == "manufacturing":
        return _manufacturing(_Index(document, "Machine"), profile)
    raise ValueError(f"unsupported conformance scenario {scenario!r}")
