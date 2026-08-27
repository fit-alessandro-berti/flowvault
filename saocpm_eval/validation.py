"""Generated-run structural, checksum, and semantic validation."""

import csv
import json
from pathlib import Path
from typing import Any

import jsonschema

from saocpm_eval.common.hashing import sha256_file
from saocpm_eval.common.ocel_builder import OcelBuilder
from saocpm_eval.common.truth_writer import read_manifest
from saocpm_eval.completion import (
    atomic_write_json,
    file_snapshot,
    implementation_fingerprint,
    read_json_record,
    snapshot_matches,
)

REPOSITORY_ROOT = Path(__file__).parents[1]
REQUIRED_RUN_FILES = (
    "observed.ocel.json",
    "observed.behavior.ocel.json",
    "state_query.sql",
    "manifest.json",
    "truth/state_at_event.csv",
    "truth/state_episodes.csv",
    "truth/transitions.csv",
    "truth/latent_regime_at_event.csv",
    "truth/latent_regime_episodes.csv",
    "truth/injected_pattern_instances.csv",
    "truth/conformance_violations.csv",
    "truth/prediction_samples.csv",
    "truth/outcomes_by_object.csv",
    "truth/causal_truth.json",
    "expected/summary.json",
    "expected/branch_coverage.json",
    "expected/golden_assertions.json",
    "tasks/answer_key.json",
)


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"JSON root in {path} must be an object")
    return value


def _read_csv(path: Path) -> list[dict[str, str]]:
    try:
        with path.open(encoding="utf-8", newline="") as source:
            return list(csv.DictReader(source))
    except OSError as exc:
        raise ValueError(f"cannot read CSV {path}: {exc}") from exc


def _validate_manifest(
    run_dir: Path, manifest: dict[str, Any], *, verify_checksums: bool = True
) -> None:
    schema = _read_json(REPOSITORY_ROOT / "schemas" / "manifest.schema.json")
    try:
        jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker()).validate(
            manifest
        )
    except jsonschema.ValidationError as exc:
        raise ValueError(f"manifest schema validation failed: {exc.message}") from exc
    inventory = manifest["files"]
    for relative_path, expected in inventory.items():
        path = run_dir / relative_path
        if not path.is_file():
            raise ValueError(f"manifest file is missing: {relative_path}")
        if path.stat().st_size != expected["bytes"]:
            raise ValueError(f"manifest size mismatch: {relative_path}")
        if verify_checksums and sha256_file(path) != expected["sha256"]:
            raise ValueError(f"manifest checksum mismatch: {relative_path}")


def _inventory_reference_from_event(attributes: dict[str, Any]) -> str:
    if not bool(attributes["data_complete"]):
        return "Unknown"
    usable = max(0.0, float(attributes["on_hand_after"]) - float(attributes["reserved_after"]))
    critical = float(attributes["confirmed_demand_horizon"]) > (
        usable + float(attributes["inbound_horizon"])
    )
    if bool(attributes["critical_understock"]) != critical:
        raise ValueError("observed critical_understock is inconsistent with independent inputs")
    if critical:
        return "Critical Understock"
    if float(attributes["on_hand_after"]) < float(attributes["lower_threshold"]):
        return "Understock"
    if float(attributes["on_hand_after"]) > float(attributes["upper_threshold"]):
        return "Overstock"
    return "Normal"


def _validate_inventory(run_dir: Path, manifest: dict[str, Any]) -> None:
    from saocpm_eval.inventory.validation import validate_inventory_document

    builder = OcelBuilder.read_json(
        run_dir / "observed.ocel.json", leading_object_type="ItemLocation"
    )
    document = builder.to_dict()
    validate_inventory_document(document)
    forbidden = {
        "reference_state",
        "latent_regime",
        "future_outcome",
        "injected_pattern_id",
        "causal_treatment_outcome",
    }
    for event in document["events"]:
        names = {attribute["name"] for attribute in event["attributes"]}
        leaked = names.intersection(forbidden)
        if leaked:
            raise ValueError(
                f"observed event {event['id']} leaks truth attributes: {sorted(leaked)}"
            )
    truth_rows = _read_csv(run_dir / "truth" / "state_at_event.csv")
    truth_by_event = {row["event_id"]: row for row in truth_rows}
    compared = 0
    for event in document["events"]:
        leading = [
            relationship["objectId"]
            for relationship in event["relationships"]
            if builder.objects[relationship["objectId"]].type == "ItemLocation"
        ]
        if not leading:
            continue
        row = truth_by_event.get(event["id"])
        if row is None:
            raise ValueError(f"missing state truth for event {event['id']}")
        attributes = {attribute["name"]: attribute["value"] for attribute in event["attributes"]}
        expected = _inventory_reference_from_event(attributes)
        if row["reference_state"] != expected:
            raise ValueError(
                f"state truth mismatch at {event['id']}: {row['reference_state']} != {expected}"
            )
        compared += 1
    if compared != len(truth_rows):
        raise ValueError(
            f"state truth row count mismatch: compared {compared}, found {len(truth_rows)}"
        )
    expected_counts = manifest.get("expected_counts", {})
    if expected_counts.get("states") != compared:
        raise ValueError("manifest expected state count does not match truth")


def _manufacturing_reference_from_event(attributes: dict[str, Any]) -> str:
    if not bool(attributes["data_complete"]):
        return "Unknown"
    if bool(attributes["down_active"]) or attributes["mode"] == "DOWN":
        return "Down"
    if bool(attributes["quality_hold_active"]):
        return "Quality Hold"
    if bool(attributes["recovery_active"]):
        return "Recovery"
    if attributes["mode"] == "SETUP":
        return "Setup"
    if bool(attributes["degraded_latched"]):
        return "Degraded"
    if attributes["mode"] == "RUNNING":
        return "Running"
    return "Idle"


def _validate_manufacturing(run_dir: Path, manifest: dict[str, Any]) -> None:
    builder = OcelBuilder.read_json(run_dir / "observed.ocel.json", leading_object_type="Machine")
    document = builder.to_dict()
    forbidden = {
        "reference_state",
        "latent_regime",
        "bearing_wear",
        "thermal_wear",
        "calibration_drift",
        "true_fault_family",
        "future_outcome",
        "injected_pattern_id",
    }
    initialization: set[str] = set()
    snapshots: set[str] = set()
    truth_rows = _read_csv(run_dir / "truth" / "state_at_event.csv")
    truth_by_event = {row["event_id"]: row for row in truth_rows}
    compared = 0
    for event in document["events"]:
        leading = [
            relationship["objectId"]
            for relationship in event["relationships"]
            if builder.objects[relationship["objectId"]].type == "Machine"
        ]
        if not leading:
            continue
        machine = leading[0]
        if event["type"] == "Initialize Machine":
            initialization.add(machine)
        elif event["type"] == "Simulation End Snapshot":
            snapshots.add(machine)
        attributes = {attribute["name"]: attribute["value"] for attribute in event["attributes"]}
        leaked = set(attributes).intersection(forbidden)
        if leaked:
            raise ValueError(f"observed manufacturing event leaks truth: {sorted(leaked)}")
        if bool(attributes["data_complete"]):
            health = float(attributes["health_index"])
            load = float(attributes["load_fraction"])
            if not 0 <= health <= 1:
                raise ValueError(f"health index out of bounds at {event['id']}")
            if not 0 <= load <= 1.5:
                raise ValueError(f"load fraction out of bounds at {event['id']}")
            if float(attributes["vibration_rms"]) < 0 or float(attributes["power_kw"]) < 0:
                raise ValueError(f"negative sensor value at {event['id']}")
        row = truth_by_event.get(event["id"])
        if row is None:
            raise ValueError(f"missing manufacturing state truth for event {event['id']}")
        expected = _manufacturing_reference_from_event(attributes)
        if row["reference_state"] != expected:
            raise ValueError(
                f"manufacturing state truth mismatch at {event['id']}: "
                f"{row['reference_state']} != {expected}"
            )
        compared += 1
    machines = {
        identifier for identifier, item in builder.objects.items() if item.type == "Machine"
    }
    if initialization != machines:
        raise ValueError("not every machine has exactly one observed initialization")
    if snapshots != machines:
        raise ValueError("not every machine has an end snapshot")
    if compared != len(truth_rows):
        raise ValueError("manufacturing state truth row count does not match observed events")
    physical_rows = _read_csv(run_dir / "truth" / "physical_state_at_event.csv")
    if len(physical_rows) != compared:
        raise ValueError("physical truth row count does not match observed machine events")
    physical_by_event = {row["event_id"]: row for row in physical_rows}
    if len(physical_by_event) != len(physical_rows):
        raise ValueError("physical truth contains duplicate event IDs")
    for row in physical_rows:
        for name in ("bearing_wear", "thermal_wear", "calibration_drift", "true_health_index"):
            if not 0 <= float(row[name]) <= 1:
                raise ValueError(f"physical truth {name} is out of bounds")
        expected_health = 1.0 - (
            0.5 * float(row["bearing_wear"])
            + 0.35 * float(row["thermal_wear"])
            + 0.15 * float(row["calibration_drift"])
        )
        expected_health = min(1.0, max(0.0, expected_health))
        if abs(float(row["true_health_index"]) - expected_health) > 1e-9:
            raise ValueError(f"physical truth health equation mismatch at {row['event_id']}")

    component_families: dict[str, str] = {}
    for object_id, item in builder.objects.items():
        if item.type != "Component":
            continue
        values = [
            attribute.value for attribute in item.attributes if attribute.name == "component_family"
        ]
        if values:
            component_families[object_id] = str(values[-1])
    reset_values = {
        "bearing": ("bearing_wear", 0.02),
        "thermal": ("thermal_wear", 0.02),
        "calibration": ("calibration_drift", 0.0),
    }
    for event in document["events"]:
        if event["type"] != "Component Replaced":
            continue
        component_ids = sorted(
            {
                relationship["objectId"]
                for relationship in event["relationships"]
                if relationship["objectId"] in component_families
            }
        )
        if len(component_ids) != 1:
            raise ValueError(f"replacement {event['id']} must identify one component")
        if component_families.get(component_ids[0]) not in reset_values:
            raise ValueError(f"replacement {event['id']} has an unknown component family")
        physical = physical_by_event[event["id"]]
        reset_observed = any(
            abs(float(physical[field]) - reset_target) <= 1e-9
            for field, reset_target in reset_values.values()
        )
        if not reset_observed:
            raise ValueError(f"replacement effect mismatch at {event['id']}")
    if manifest.get("expected_counts", {}).get("states") != compared:
        raise ValueError("manifest expected manufacturing state count does not match truth")


def preflight_run(run_dir: Path) -> dict[str, Any]:
    """Cheap analysis preflight using the manifest schema, inventory, and file sizes."""

    for relative_path in REQUIRED_RUN_FILES:
        if not (run_dir / relative_path).is_file():
            raise ValueError(f"required run file is missing: {relative_path}")
    manifest = read_manifest(run_dir / "manifest.json")
    _validate_manifest(run_dir, manifest, verify_checksums=False)
    if manifest.get("scenario") not in {"inventory", "manufacturing"}:
        raise ValueError(f"unsupported run scenario {manifest.get('scenario')!r}")
    return manifest


def validate_run(run_dir: Path, *, force: bool = False) -> None:
    manifest = preflight_run(run_dir)
    input_paths = ("manifest.json", *manifest["files"])
    implementation_sha256 = implementation_fingerprint(
        (
            Path(__file__),
            REPOSITORY_ROOT / "saocpm_eval" / "common" / "ocel_builder.py",
            REPOSITORY_ROOT / "saocpm_eval" / "inventory" / "validation.py",
        )
    )
    completion_path = run_dir / "analytics" / "validation_manifest.json"
    prior = read_json_record(completion_path)
    if (
        not force
        and prior is not None
        and prior.get("complete") is True
        and prior.get("implementation_sha256") == implementation_sha256
        and isinstance(prior.get("input_snapshot"), dict)
        and snapshot_matches(run_dir, prior["input_snapshot"])
    ):
        return
    _validate_manifest(run_dir, manifest)
    scenario = manifest.get("scenario")
    if scenario == "inventory":
        _validate_inventory(run_dir, manifest)
    elif scenario == "manufacturing":
        _validate_manufacturing(run_dir, manifest)
    else:
        raise ValueError(f"unsupported run scenario {scenario!r}")
    atomic_write_json(
        completion_path,
        {
            "complete": True,
            "scenario": scenario,
            "run_id": manifest.get("run_id"),
            "implementation_sha256": implementation_sha256,
            "input_snapshot": file_snapshot(run_dir, input_paths),
        },
    )
