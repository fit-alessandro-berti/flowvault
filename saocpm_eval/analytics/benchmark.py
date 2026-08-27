"""Resumable geometric-scale performance benchmark runner."""

from __future__ import annotations

import csv
import json
import os
import platform
import subprocess
from pathlib import Path
from typing import Any

from saocpm_eval.analytics.flowvault_cli import CliResult, run_json
from saocpm_eval.common.hashing import canonical_json_bytes, sha256_file
from saocpm_eval.common.truth_writer import repository_commit
from saocpm_eval.completion import implementation_fingerprint
from saocpm_eval.config import ScaleMatrix, ScaleProfile
from saocpm_eval.scale import generate_scale_fixture

ROW_FIELDS = (
    "profile_id",
    "profile",
    "scenario",
    "repetition",
    "operation",
    "application_commit",
    "commit",
    "generator_commit",
    "config_sha256",
    "wall_time_ms",
    "cpu_time_ms",
    "peak_rss_bytes",
    "input_bytes",
    "output_bytes",
    "event_count",
    "events",
    "object_count",
    "objects",
    "leading_object_count",
    "e2o_count",
    "o2o_count",
    "average_lifecycle_length",
    "e2o_density",
    "o2o_density",
    "event_attribute_value_count",
    "object_attribute_update_count",
    "state_count",
    "transition_frequency",
    "feature_count",
    "features",
    "window_count",
    "windows",
    "pattern_count",
    "patterns",
)


def _jsonl_rows(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    rows = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ValueError(f"corrupt benchmark JSONL row {line_number}: {exc}") from exc
        if not isinstance(row, dict):
            raise ValueError(f"benchmark JSONL row {line_number} is not an object")
        rows.append(row)
    return rows


def _append_row(path: Path, row: dict[str, Any]) -> None:
    payload = canonical_json_bytes(row)
    with path.open("ab") as target:
        target.write(payload)
        target.flush()
        os.fsync(target.fileno())


def _write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=ROW_FIELDS, lineterminator="\n")
        writer.writeheader()
        writer.writerows({field: row.get(field, "") for field in ROW_FIELDS} for row in rows)


def _command_version(command: list[str]) -> str:
    try:
        return subprocess.run(command, check=True, capture_output=True, text=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def _metadata(matrix_path: Path) -> dict[str, Any]:
    repository_root = Path(__file__).parents[2]
    try:
        physical_memory_bytes = int(os.sysconf("SC_PAGE_SIZE")) * int(os.sysconf("SC_PHYS_PAGES"))
    except (OSError, TypeError, ValueError):
        physical_memory_bytes = 0
    return {
        "application_commit": repository_commit(repository_root),
        "generator_commit": repository_commit(repository_root),
        "implementation_sha256": implementation_fingerprint(
            (
                Path(__file__),
                Path(__file__).parent / "flowvault_cli.py",
                repository_root / "saocpm_eval" / "scale.py",
                repository_root / "rust" / "ocel_core" / "src",
                repository_root / "rust" / "ocel_cli" / "src",
            )
        ),
        "matrix_sha256": sha256_file(matrix_path),
        "operating_system": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "cpu_count": os.cpu_count(),
        "physical_memory_bytes": physical_memory_bytes,
        "python": platform.python_version(),
        "rustc": _command_version(["rustc", "--version"]),
        "cargo": _command_version(["cargo", "--version"]),
        "rust_profile": os.environ.get("FLOWVAULT_RUST_PROFILE", "release"),
        "wasm_build_mode": "native headless CLI",
        "browser": _command_version(["npx", "playwright", "--version"]),
        "browser_interaction_latency": "measured separately by Playwright E2E tests",
    }


def _ensure_fixture(profile: ScaleProfile, root: Path) -> Path:
    fixture = root / "runs" / profile.id
    profile_path = fixture / "scale_profile.json"
    expected = canonical_json_bytes(profile.model_dump(mode="json"), pretty=True)
    if profile_path.is_file():
        if profile_path.read_bytes() != expected:
            raise ValueError(f"scale fixture {profile.id!r} has incompatible parameters")
        if not (fixture / "observed.ocel.json").is_file():
            raise ValueError(f"scale fixture {profile.id!r} is incomplete")
        return fixture
    if fixture.exists() and any(fixture.iterdir()):
        raise ValueError(f"scale fixture {profile.id!r} is incomplete")
    generate_scale_fixture(profile, fixture)
    return fixture


def _request(profile: ScaleProfile) -> dict[str, Any]:
    return {
        "object_type": "ItemLocation" if profile.scenario == "inventory" else "Machine",
        "window_size": 3 if profile.scenario == "inventory" else 4,
        "som_width": 3,
        "som_height": 3,
        "epochs": 10,
        "max_training_windows": 50_000,
    }


def _operation(
    name: str,
    profile: ScaleProfile,
    fixture: Path,
) -> CliResult:
    input_path = fixture / "observed.ocel.json"
    query_path = fixture / "state_query.sql"
    leading_type = "ItemLocation" if profile.scenario == "inventory" else "Machine"
    detection = _request(profile)
    if name == "import":
        return run_json("summary", input_path=input_path)
    if name == "apply_state_query":
        return run_json("apply-state-query", input_path=input_path, query_path=query_path)
    if name == "state_transition_kpis":
        return run_json(
            "state-transition-kpis",
            input_path=input_path,
            query_path=query_path,
            request={"object_type": leading_type},
        )
    if name == "ocdfg":
        return run_json("ocdfg", input_path=input_path, object_type=leading_type)
    if name == "sa_ocdfg":
        return run_json("sa-ocdfg", input_path=input_path, query_path=query_path)
    if name == "state_patterns":
        return run_json(
            "state-patterns",
            input_path=input_path,
            query_path=query_path,
            request={
                "leading_object_type": leading_type,
                "family": "both",
                "min_support": 1,
                "include_occurrences": False,
            },
        )
    if name == "state_detection_and_assignments":
        return run_json("state-detection-assignments", input_path=input_path, request=detection)
    if name == "export_json":
        return run_json("export", input_path=input_path, query_path=query_path)
    raise ValueError(f"unsupported benchmark operation {name!r}")


def _counts(summary: dict[str, Any], profile: ScaleProfile) -> dict[str, int | float]:
    event_count = int(summary["events"])
    object_count = int(summary["objects"])
    leading_count = profile.leading_objects
    e2o_count = int(summary["e2o_relationships"])
    o2o_count = int(summary["o2o_relationships"])
    event_attribute_width = 6 if profile.scenario == "inventory" else 7
    return {
        "event_count": event_count,
        "object_count": object_count,
        "leading_object_count": leading_count,
        "e2o_count": e2o_count,
        "o2o_count": o2o_count,
        "average_lifecycle_length": event_count / leading_count,
        "e2o_density": e2o_count / event_count,
        "o2o_density": o2o_count / object_count,
        "event_attribute_value_count": event_count * event_attribute_width,
        "object_attribute_update_count": leading_count,
    }


def _derived_counts(operation: str, value: Any) -> dict[str, int | str]:
    result: dict[str, int | str] = {
        "state_count": "",
        "transition_frequency": "",
        "feature_count": "",
        "window_count": "",
        "pattern_count": "",
    }
    if operation == "state_detection_and_assignments":
        result["feature_count"] = int(value["feature_count"])
        result["window_count"] = int(value["window_count"])
    elif operation == "state_patterns":
        result["pattern_count"] = len(value["intra"]) + len(value["inter"])
    elif operation == "apply_state_query":
        result["state_count"] = int(value["assigned_events"])
    elif operation == "state_transition_kpis":
        result["transition_frequency"] = sum(int(row["count"]) for row in value["transitions"])
    return result


def run_benchmark(*, matrix: ScaleMatrix, matrix_path: Path, output_dir: Path) -> None:
    """Run every requested operation with append-only, interruption-safe checkpoints."""

    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "runs").mkdir(exist_ok=True)
    metadata_path = output_dir / "metadata.json"
    current_metadata = _metadata(matrix_path)
    if metadata_path.is_file():
        prior = json.loads(metadata_path.read_text(encoding="utf-8"))
        identity_fields = ("matrix_sha256", "implementation_sha256", "rust_profile")
        if any(prior.get(field) != current_metadata[field] for field in identity_fields):
            raise ValueError("benchmark output belongs to a different experiment definition")
        metadata = prior
    else:
        metadata = current_metadata
        metadata_path.write_bytes(canonical_json_bytes(metadata, pretty=True))

    results_path = output_dir / "results.jsonl"
    rows = _jsonl_rows(results_path)
    completed = {(row["profile_id"], int(row["repetition"]), row["operation"]) for row in rows}
    matrix_hash = sha256_file(matrix_path)
    for profile in matrix.profiles:
        expected = {
            (profile.id, repetition, operation)
            for operation in matrix.operations
            for repetition in range(1, matrix.repetitions + 1)
        }
        if expected.issubset(completed):
            continue
        fixture = _ensure_fixture(profile, output_dir)
        summary_path = fixture / "summary.json"
        if summary_path.is_file():
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
        else:
            summary = run_json("summary", input_path=fixture / "observed.ocel.json").value
            summary_path.write_bytes(canonical_json_bytes(summary, pretty=True))
        counts = _counts(summary, profile)
        for operation in matrix.operations:
            if not any(key[0] == profile.id and key[2] == operation for key in completed):
                for _ in range(matrix.warmup_repetitions):
                    _operation(operation, profile, fixture)
            for repetition in range(1, matrix.repetitions + 1):
                key = (profile.id, repetition, operation)
                if key in completed:
                    continue
                measured = _operation(operation, profile, fixture)
                counts = _counts(summary, profile)
                derived = _derived_counts(operation, measured.value)
                row = {
                    "profile_id": profile.id,
                    "profile": profile.id,
                    "scenario": profile.scenario,
                    "repetition": repetition,
                    "operation": operation,
                    "application_commit": metadata["application_commit"],
                    "commit": metadata["application_commit"],
                    "generator_commit": metadata["generator_commit"],
                    "config_sha256": matrix_hash,
                    **measured.metrics,
                    **counts,
                    **derived,
                    "events": counts["event_count"],
                    "objects": counts["object_count"],
                    "features": derived["feature_count"],
                    "windows": derived["window_count"],
                    "patterns": derived["pattern_count"],
                }
                row = {field: row.get(field, "") for field in ROW_FIELDS}
                _append_row(results_path, row)
                rows.append(row)
                completed.add(key)
                _write_csv(output_dir / "results.csv", rows)
                _write_csv(output_dir / "performance.csv", rows)
    if rows:
        _write_csv(output_dir / "results.csv", rows)
        _write_csv(output_dir / "performance.csv", rows)
