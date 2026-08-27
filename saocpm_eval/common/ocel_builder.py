"""Typed construction and strict structural validation of OCEL 2.0 JSON."""

from __future__ import annotations

import json
import math
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import UTC, datetime
from itertools import pairwise
from pathlib import Path
from typing import Any, Literal

OcelType = Literal["string", "integer", "float", "boolean", "time"]
type OcelValue = str | int | float | bool
type JsonObject = dict[str, Any]


class OcelValidationError(ValueError):
    """Raised when an OCEL document violates the strict builder contract."""


def parse_timestamp(value: str) -> datetime:
    """Parse an ISO-8601 timestamp and require an explicit UTC offset."""

    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise OcelValidationError(f"invalid timestamp {value!r}") from exc
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise OcelValidationError(f"timestamp {value!r} has no UTC offset")
    return parsed.astimezone(UTC)


def format_timestamp(value: datetime | str) -> str:
    parsed = parse_timestamp(value) if isinstance(value, str) else value
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise OcelValidationError("timestamp has no UTC offset")
    utc_value = parsed.astimezone(UTC)
    text = utc_value.isoformat(timespec="seconds")
    return text.replace("+00:00", "Z")


@dataclass(frozen=True, slots=True)
class AttributeDeclaration:
    name: str
    type: OcelType

    def __post_init__(self) -> None:
        if not self.name:
            raise OcelValidationError("attribute declaration name must not be empty")
        if self.type not in {"string", "integer", "float", "boolean", "time"}:
            raise OcelValidationError(f"unsupported OCEL attribute type {self.type!r}")

    def to_dict(self) -> JsonObject:
        return {"name": self.name, "type": self.type}


@dataclass(frozen=True, slots=True)
class TypeDeclaration:
    name: str
    attributes: tuple[AttributeDeclaration, ...] = ()

    def __post_init__(self) -> None:
        if not self.name:
            raise OcelValidationError("type declaration name must not be empty")
        names = [attribute.name for attribute in self.attributes]
        if len(names) != len(set(names)):
            raise OcelValidationError(f"type {self.name!r} has duplicate attribute declarations")

    @classmethod
    def create(
        cls, name: str, attributes: dict[str, OcelType] | tuple[AttributeDeclaration, ...]
    ) -> TypeDeclaration:
        if isinstance(attributes, dict):
            return cls(
                name,
                tuple(AttributeDeclaration(key, value) for key, value in attributes.items()),
            )
        return cls(name, attributes)

    @property
    def attribute_types(self) -> dict[str, OcelType]:
        return {attribute.name: attribute.type for attribute in self.attributes}

    def to_dict(self) -> JsonObject:
        return {"name": self.name, "attributes": [item.to_dict() for item in self.attributes]}


@dataclass(frozen=True, slots=True)
class Relationship:
    object_id: str
    qualifier: str

    def __post_init__(self) -> None:
        if not self.object_id:
            raise OcelValidationError("relationship object ID must not be empty")
        if not self.qualifier:
            raise OcelValidationError("relationship qualifier must not be empty")

    def to_dict(self) -> JsonObject:
        return {"objectId": self.object_id, "qualifier": self.qualifier}


@dataclass(frozen=True, slots=True)
class ObjectAttribute:
    name: str
    time: str
    value: OcelValue

    @classmethod
    def create(cls, name: str, time: datetime | str, value: OcelValue) -> ObjectAttribute:
        return cls(name=name, time=format_timestamp(time), value=value)

    def to_dict(self) -> JsonObject:
        return {"name": self.name, "time": self.time, "value": self.value}


@dataclass(frozen=True, slots=True)
class EventAttribute:
    name: str
    value: OcelValue

    def to_dict(self) -> JsonObject:
        return {"name": self.name, "value": self.value}


@dataclass(slots=True)
class OcelObject:
    id: str
    type: str
    attributes: list[ObjectAttribute] = field(default_factory=list)
    relationships: list[Relationship] = field(default_factory=list)

    def to_dict(self) -> JsonObject:
        result: JsonObject = {
            "id": self.id,
            "type": self.type,
            "attributes": [
                item.to_dict()
                for item in sorted(
                    self.attributes,
                    key=lambda attribute: (parse_timestamp(attribute.time), attribute.name),
                )
            ],
        }
        if self.relationships:
            result["relationships"] = [item.to_dict() for item in self.relationships]
        return result


@dataclass(slots=True)
class OcelEvent:
    id: str
    type: str
    time: str
    attributes: list[EventAttribute] = field(default_factory=list)
    relationships: list[Relationship] = field(default_factory=list)

    @classmethod
    def create(
        cls,
        event_id: str,
        event_type: str,
        time: datetime | str,
        attributes: list[EventAttribute] | None = None,
        relationships: list[Relationship] | None = None,
    ) -> OcelEvent:
        return cls(
            id=event_id,
            type=event_type,
            time=format_timestamp(time),
            attributes=attributes or [],
            relationships=relationships or [],
        )

    def to_dict(self) -> JsonObject:
        return {
            "id": self.id,
            "type": self.type,
            "time": self.time,
            "attributes": [item.to_dict() for item in self.attributes],
            "relationships": [item.to_dict() for item in self.relationships],
        }


def _normalize_value(value: Any, expected: OcelType, context: str) -> OcelValue:
    if expected == "boolean":
        if type(value) is not bool:
            raise OcelValidationError(f"{context} must be boolean")
        return value
    if expected == "integer":
        if type(value) is not int:
            raise OcelValidationError(f"{context} must be integer")
        return value
    if expected == "float":
        if type(value) not in {float, int}:
            raise OcelValidationError(f"{context} must be float")
        normalized = float(value)
        if not math.isfinite(normalized):
            raise OcelValidationError(f"{context} must be finite")
        return normalized
    if expected == "string":
        if type(value) is not str:
            raise OcelValidationError(f"{context} must be string")
        return value
    if expected == "time":
        if type(value) is not str:
            raise OcelValidationError(f"{context} must be an ISO-8601 string")
        return format_timestamp(value)
    raise OcelValidationError(f"unsupported type {expected!r} for {context}")


class OcelBuilder:
    """Build one canonical OCEL document while enforcing evaluation invariants."""

    def __init__(self, leading_object_type: str | None = None) -> None:
        self.leading_object_type = leading_object_type
        self.object_types: dict[str, TypeDeclaration] = {}
        self.event_types: dict[str, TypeDeclaration] = {}
        self.objects: dict[str, OcelObject] = {}
        self.events: dict[str, OcelEvent] = {}

    def declare_object_type(
        self,
        name: str,
        attributes: dict[str, OcelType] | tuple[AttributeDeclaration, ...] = (),
    ) -> None:
        if name in self.object_types:
            raise OcelValidationError(f"duplicate object type {name!r}")
        self.object_types[name] = TypeDeclaration.create(name, attributes)

    def declare_event_type(
        self,
        name: str,
        attributes: dict[str, OcelType] | tuple[AttributeDeclaration, ...] = (),
    ) -> None:
        if name in self.event_types:
            raise OcelValidationError(f"duplicate event type {name!r}")
        self.event_types[name] = TypeDeclaration.create(name, attributes)

    def add_object(self, ocel_object: OcelObject) -> None:
        if ocel_object.id in self.objects:
            raise OcelValidationError(f"duplicate object ID {ocel_object.id!r}")
        self._validate_object(ocel_object, validate_relationships=False)
        self.objects[ocel_object.id] = ocel_object

    def add_event(self, event: OcelEvent) -> None:
        if event.id in self.events:
            raise OcelValidationError(f"duplicate event ID {event.id!r}")
        self._validate_event(event, validate_relationships=False)
        self.events[event.id] = event

    def _validate_attributes(
        self,
        attributes: list[ObjectAttribute] | list[EventAttribute],
        declaration: TypeDeclaration,
        context: str,
    ) -> None:
        declared = declaration.attribute_types
        seen: set[tuple[str, str | None]] = set()
        for attribute in attributes:
            timestamp = attribute.time if isinstance(attribute, ObjectAttribute) else None
            key = (attribute.name, timestamp)
            if key in seen:
                raise OcelValidationError(f"{context} has duplicate attribute {attribute.name!r}")
            seen.add(key)
            expected = declared.get(attribute.name)
            if expected is None:
                raise OcelValidationError(f"{context} uses undeclared attribute {attribute.name!r}")
            _normalize_value(attribute.value, expected, f"{context} attribute {attribute.name!r}")
            if timestamp is not None:
                parse_timestamp(timestamp)

    def _validate_relationships(self, relationships: list[Relationship], context: str) -> None:
        seen: set[tuple[str, str]] = set()
        for relationship in relationships:
            key = (relationship.object_id, relationship.qualifier)
            if key in seen:
                raise OcelValidationError(f"{context} has duplicate relationship {key!r}")
            seen.add(key)
            if relationship.object_id not in self.objects:
                raise OcelValidationError(
                    f"{context} references unknown object {relationship.object_id!r}"
                )

    def _validate_object(self, item: OcelObject, *, validate_relationships: bool) -> None:
        declaration = self.object_types.get(item.type)
        if declaration is None:
            raise OcelValidationError(f"object {item.id!r} has unknown type {item.type!r}")
        self._validate_attributes(item.attributes, declaration, f"object {item.id!r}")
        if validate_relationships:
            self._validate_relationships(item.relationships, f"object {item.id!r}")

    def _validate_event(self, item: OcelEvent, *, validate_relationships: bool) -> None:
        declaration = self.event_types.get(item.type)
        if declaration is None:
            raise OcelValidationError(f"event {item.id!r} has unknown type {item.type!r}")
        parse_timestamp(item.time)
        self._validate_attributes(item.attributes, declaration, f"event {item.id!r}")
        if validate_relationships:
            self._validate_relationships(item.relationships, f"event {item.id!r}")

    def validate(self) -> None:
        if (
            self.leading_object_type is not None
            and self.leading_object_type not in self.object_types
        ):
            raise OcelValidationError(
                f"leading object type {self.leading_object_type!r} is not declared"
            )
        for object_item in self.objects.values():
            self._validate_object(object_item, validate_relationships=True)
        lifecycle_times: dict[str, list[tuple[datetime, str]]] = defaultdict(list)
        for event_item in self.events.values():
            self._validate_event(event_item, validate_relationships=True)
            if self.leading_object_type is None:
                continue
            leading_ids = [
                relationship.object_id
                for relationship in event_item.relationships
                if self.objects[relationship.object_id].type == self.leading_object_type
            ]
            if len(leading_ids) > 1:
                raise OcelValidationError(
                    f"event {event_item.id!r} relates to multiple "
                    f"{self.leading_object_type!r} objects"
                )
            if leading_ids:
                lifecycle_times[leading_ids[0]].append(
                    (parse_timestamp(event_item.time), event_item.id)
                )
        for object_id, times in lifecycle_times.items():
            ordered = sorted(times)
            for previous, current in pairwise(ordered):
                if previous[0] == current[0]:
                    raise OcelValidationError(
                        f"leading object {object_id!r} has non-increasing event timestamps "
                        f"at {previous[1]!r} and {current[1]!r}"
                    )

    def to_dict(self) -> JsonObject:
        self.validate()
        return {
            "objectTypes": [item.to_dict() for item in self.object_types.values()],
            "eventTypes": [item.to_dict() for item in self.event_types.values()],
            "objects": [self.objects[key].to_dict() for key in sorted(self.objects)],
            "events": [
                item.to_dict()
                for item in sorted(
                    self.events.values(), key=lambda event: (parse_timestamp(event.time), event.id)
                )
            ],
        }

    def to_json_bytes(self) -> bytes:
        """Return byte-stable canonical JSON with a trailing newline."""

        return (
            json.dumps(
                self.to_dict(),
                ensure_ascii=False,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode()

    def write_json(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(self.to_json_bytes())

    @classmethod
    def from_dict(cls, document: JsonObject, leading_object_type: str | None = None) -> OcelBuilder:
        builder = cls(leading_object_type=leading_object_type)
        try:
            for raw_type in document["objectTypes"]:
                builder.declare_object_type(
                    raw_type["name"],
                    tuple(
                        AttributeDeclaration(attribute["name"], attribute["type"])
                        for attribute in raw_type.get("attributes", [])
                    ),
                )
            for raw_type in document["eventTypes"]:
                builder.declare_event_type(
                    raw_type["name"],
                    tuple(
                        AttributeDeclaration(attribute["name"], attribute["type"])
                        for attribute in raw_type.get("attributes", [])
                    ),
                )
            for raw in document["objects"]:
                builder.add_object(
                    OcelObject(
                        id=raw["id"],
                        type=raw["type"],
                        attributes=[
                            ObjectAttribute.create(
                                attribute["name"], attribute["time"], attribute["value"]
                            )
                            for attribute in raw.get("attributes", [])
                        ],
                        relationships=[
                            Relationship(item["objectId"], item["qualifier"])
                            for item in raw.get("relationships", [])
                        ],
                    )
                )
            for raw in document["events"]:
                builder.add_event(
                    OcelEvent.create(
                        event_id=raw["id"],
                        event_type=raw["type"],
                        time=raw["time"],
                        attributes=[
                            EventAttribute(attribute["name"], attribute["value"])
                            for attribute in raw.get("attributes", [])
                        ],
                        relationships=[
                            Relationship(item["objectId"], item["qualifier"])
                            for item in raw.get("relationships", [])
                        ],
                    )
                )
        except (KeyError, TypeError) as exc:
            raise OcelValidationError(f"malformed OCEL document: {exc}") from exc
        builder.validate()
        return builder

    @classmethod
    def read_json(cls, path: Path, leading_object_type: str | None = None) -> OcelBuilder:
        try:
            document = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise OcelValidationError(f"cannot read OCEL JSON {path}: {exc}") from exc
        if not isinstance(document, dict):
            raise OcelValidationError("OCEL root must be an object")
        return cls.from_dict(document, leading_object_type=leading_object_type)
