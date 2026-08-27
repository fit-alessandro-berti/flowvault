"""Typed subprocess adapter for the headless FLOWVAULT Rust CLI."""

from __future__ import annotations

import json
import os
import resource
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

REPOSITORY_ROOT = Path(__file__).parents[2]
RUST_ROOT = REPOSITORY_ROOT / "rust"


@dataclass(frozen=True, slots=True)
class CliResult:
    value: Any
    metrics: dict[str, Any]


def _source_is_newer(binary: Path) -> bool:
    if not binary.is_file():
        return True
    modified = binary.stat().st_mtime
    sources = (
        RUST_ROOT / "ocel_cli" / "src",
        RUST_ROOT / "ocel_core" / "src",
    )
    return any(
        path.stat().st_mtime > modified for source in sources for path in source.rglob("*.rs")
    )


def find_cli() -> Path:
    override = os.environ.get("FLOWVAULT_OCEL_CLI")
    if override:
        binary = Path(override).expanduser().resolve()
        if not binary.is_file():
            raise ValueError(f"FLOWVAULT_OCEL_CLI does not exist: {binary}")
        return binary
    profile = os.environ.get("FLOWVAULT_RUST_PROFILE", "release")
    if profile not in {"debug", "release"}:
        raise ValueError("FLOWVAULT_RUST_PROFILE must be 'debug' or 'release'")
    binary = RUST_ROOT / "target" / profile / "ocel_cli"
    if _source_is_newer(binary):
        environment = os.environ.copy()
        environment["CARGO_HOME"] = os.environ.get(
            "FLOWVAULT_CARGO_HOME", str(REPOSITORY_ROOT / ".cargo-home")
        )
        try:
            command = ["cargo", "build", "-p", "ocel_cli"]
            if profile == "release":
                command.append("--release")
            subprocess.run(
                command,
                cwd=RUST_ROOT,
                env=environment,
                check=True,
                capture_output=True,
                text=True,
            )
        except (OSError, subprocess.CalledProcessError) as exc:
            detail = (
                exc.stderr.strip() if isinstance(exc, subprocess.CalledProcessError) else str(exc)
            )
            raise ValueError(f"cannot build FLOWVAULT headless CLI: {detail}") from exc
    if not binary.is_file():
        raise ValueError(f"FLOWVAULT headless CLI was not produced at {binary}")
    return binary


def run_json(
    command: str,
    *,
    input_path: Path,
    query_path: Path | None = None,
    request: dict[str, Any] | None = None,
    object_type: str | None = None,
) -> CliResult:
    """Run one deterministic CLI operation and parse its JSON and metrics."""

    with tempfile.TemporaryDirectory(prefix="flowvault-cli-") as temporary:
        metrics_path = Path(temporary) / "metrics.json"
        arguments = [
            str(find_cli()),
            command,
            "--input",
            str(input_path),
            "--metrics",
            str(metrics_path),
        ]
        if query_path is not None:
            arguments.extend(("--query", str(query_path)))
        if request is not None:
            arguments.extend(
                ("--request", json.dumps(request, sort_keys=True, separators=(",", ":")))
            )
        if object_type is not None:
            arguments.extend(("--object-type", object_type))
        try:
            usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
            completed = subprocess.run(
                arguments,
                check=True,
                capture_output=True,
                text=True,
            )
            value = json.loads(completed.stdout)
            metrics = json.loads(metrics_path.read_text(encoding="utf-8"))
            usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)
            metrics["cpu_time_ms"] = (
                usage_after.ru_utime
                + usage_after.ru_stime
                - usage_before.ru_utime
                - usage_before.ru_stime
            ) * 1000
        except (OSError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
            detail = (
                exc.stderr.strip() if isinstance(exc, subprocess.CalledProcessError) else str(exc)
            )
            raise ValueError(f"FLOWVAULT CLI {command!r} failed: {detail}") from exc
    return CliResult(value=value, metrics=metrics)


def run_evaluation_bundle(
    *, input_path: Path, query_path: Path, request: dict[str, Any]
) -> CliResult:
    """Run all full-log evaluation operations after a single OCEL import."""

    with tempfile.TemporaryDirectory(prefix="flowvault-bundle-") as temporary:
        root = Path(temporary)
        bundle_dir = root / "parts"
        metrics_path = root / "metrics.json"
        arguments = [
            str(find_cli()),
            "evaluation-bundle",
            "--input",
            str(input_path),
            "--query",
            str(query_path),
            "--request",
            json.dumps(request, sort_keys=True, separators=(",", ":")),
            "--bundle-dir",
            str(bundle_dir),
            "--metrics",
            str(metrics_path),
        ]
        try:
            usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
            completed = subprocess.run(arguments, check=True, capture_output=True, text=True)
            manifest = json.loads(completed.stdout)
            files = dict(manifest["files"])
            value = {}
            for name, filename in files.items():
                with (bundle_dir / filename).open(encoding="utf-8") as source:
                    value[name] = json.load(source)
            metrics = json.loads(metrics_path.read_text(encoding="utf-8"))
            usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)
            metrics["cpu_time_ms"] = (
                usage_after.ru_utime
                + usage_after.ru_stime
                - usage_before.ru_utime
                - usage_before.ru_stime
            ) * 1000
        except (OSError, KeyError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
            detail = (
                exc.stderr.strip() if isinstance(exc, subprocess.CalledProcessError) else str(exc)
            )
            raise ValueError(f"FLOWVAULT evaluation bundle failed: {detail}") from exc
    return CliResult(value=value, metrics=metrics)
