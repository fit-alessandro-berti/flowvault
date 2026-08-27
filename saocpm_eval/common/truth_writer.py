"""Deterministic writers for truth sidecars and run manifests."""

from __future__ import annotations

import csv
import io
import json
import subprocess
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path, PurePosixPath
from typing import Any

from saocpm_eval.common.hashing import canonical_json_bytes, sha256_file


def repository_commit(repository: Path | None = None) -> str:
    """Return the checked-out commit without making dirty state nondeterministic."""

    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repository,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return "unknown"
    return result.stdout.strip() or "unknown"


def _safe_relative_path(relative_path: str | Path) -> Path:
    candidate = PurePosixPath(str(relative_path))
    if candidate.is_absolute() or ".." in candidate.parts or not candidate.parts:
        raise ValueError(f"unsafe run-relative path {relative_path!r}")
    return Path(*candidate.parts)


def _csv_value(value: Any) -> str | int | float:
    if value is None:
        return ""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, datetime):
        if value.tzinfo is None or value.utcoffset() is None:
            raise ValueError("CSV timestamps must include a UTC offset")
        return value.isoformat().replace("+00:00", "Z")
    if isinstance(value, (dict, list, tuple)):
        return canonical_json_bytes(value).decode("utf-8").rstrip("\n")
    if isinstance(value, (str, int, float)):
        return value
    raise TypeError(f"unsupported CSV value type {type(value).__name__}")


@dataclass(slots=True)
class RunWriter:
    """Write an immutable-layout evaluation run under one root directory."""

    root: Path

    def prepare(self) -> None:
        if self.root.exists() and any(self.root.iterdir()):
            raise ValueError(f"output directory is not empty: {self.root}")
        self.root.mkdir(parents=True, exist_ok=True)
        (self.root / "truth").mkdir(exist_ok=True)
        (self.root / "expected").mkdir(exist_ok=True)
        (self.root / "analytics").mkdir(exist_ok=True)

    def path(self, relative_path: str | Path) -> Path:
        return self.root / _safe_relative_path(relative_path)

    def write_bytes(self, relative_path: str | Path, content: bytes) -> Path:
        target = self.path(relative_path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)
        return target

    def write_text(self, relative_path: str | Path, content: str) -> Path:
        normalized = content if content.endswith("\n") else f"{content}\n"
        return self.write_bytes(relative_path, normalized.encode("utf-8"))

    def write_json(self, relative_path: str | Path, value: Any, *, pretty: bool = True) -> Path:
        return self.write_bytes(relative_path, canonical_json_bytes(value, pretty=pretty))

    def write_csv(
        self,
        relative_path: str | Path,
        fieldnames: Sequence[str],
        rows: Iterable[Mapping[str, Any]],
    ) -> Path:
        if not fieldnames or len(fieldnames) != len(set(fieldnames)):
            raise ValueError("CSV fieldnames must be non-empty and unique")
        buffer = io.StringIO(newline="")
        writer = csv.DictWriter(
            buffer,
            fieldnames=list(fieldnames),
            extrasaction="raise",
            lineterminator="\n",
        )
        writer.writeheader()
        for row in rows:
            missing = set(fieldnames).difference(row)
            if missing:
                raise ValueError(f"CSV row is missing fields: {sorted(missing)}")
            writer.writerow({name: _csv_value(row[name]) for name in fieldnames})
        return self.write_text(relative_path, buffer.getvalue())

    def file_inventory(self) -> dict[str, dict[str, int | str]]:
        files: dict[str, dict[str, int | str]] = {}
        for path in sorted(item for item in self.root.rglob("*") if item.is_file()):
            relative = path.relative_to(self.root).as_posix()
            if relative == "manifest.json":
                continue
            files[relative] = {"sha256": sha256_file(path), "bytes": path.stat().st_size}
        return files

    def write_manifest(
        self,
        *,
        scenario: str,
        profile: str,
        seed: int,
        config_sha256: str,
        generator_commit: str,
        start_time: datetime | str,
        end_time: datetime | str,
        counts: Mapping[str, int],
        rng_streams: Mapping[str, object],
        expected_counts: Mapping[str, int] | None = None,
        perturbations: Sequence[Mapping[str, object]] = (),
        extra: Mapping[str, object] | None = None,
    ) -> Path:
        manifest: dict[str, object] = {
            "scenario": scenario,
            "profile": profile,
            "seed": seed,
            "config_sha256": config_sha256,
            "generator_commit": generator_commit,
            "start_time": start_time,
            "end_time": end_time,
            "counts": dict(counts),
            "files": self.file_inventory(),
            "rng_streams": dict(rng_streams),
            "expected_counts": dict(expected_counts or {}),
            "perturbations": list(perturbations),
        }
        if extra:
            overlap = set(manifest).intersection(extra)
            if overlap:
                raise ValueError(
                    f"manifest extra fields overwrite required fields: {sorted(overlap)}"
                )
            manifest.update(extra)
        return self.write_json("manifest.json", manifest)


def read_manifest(path: Path) -> dict[str, Any]:
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot read manifest {path}: {exc}") from exc
    if not isinstance(manifest, dict):
        raise ValueError(f"manifest {path} must be a JSON object")
    return manifest
