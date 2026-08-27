"""Atomic, fingerprinted completion records for resumable evaluation modules."""

from __future__ import annotations

import json
import os
from collections.abc import Iterable, Mapping
from hashlib import sha256
from pathlib import Path
from typing import Any

from saocpm_eval.common.hashing import canonical_json_bytes


def atomic_write_json(path: Path, value: Any) -> None:
    """Atomically replace a JSON completion record after its module succeeds."""

    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    try:
        with temporary.open("wb") as target:
            target.write(canonical_json_bytes(value, pretty=True))
            target.flush()
            os.fsync(target.fileno())
        temporary.replace(path)
    finally:
        if temporary.exists():
            temporary.unlink()


def read_json_record(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def file_snapshot(root: Path, relative_paths: Iterable[str]) -> dict[str, dict[str, int]]:
    result = {}
    for relative_path in sorted(set(relative_paths)):
        path = root / relative_path
        if not path.is_file():
            continue
        stat = path.stat()
        result[relative_path] = {"bytes": stat.st_size, "mtime_ns": stat.st_mtime_ns}
    return result


def snapshot_matches(root: Path, snapshot: Mapping[str, Mapping[str, int]]) -> bool:
    return all(
        (root / relative_path).is_file()
        and (root / relative_path).stat().st_size == expected["bytes"]
        and (root / relative_path).stat().st_mtime_ns == expected["mtime_ns"]
        for relative_path, expected in snapshot.items()
    )


def size_inventory(root: Path, relative_paths: Iterable[str]) -> dict[str, int]:
    result = {}
    for relative_path in sorted(set(relative_paths)):
        path = root / relative_path
        if path.is_file():
            result[relative_path] = path.stat().st_size
    return result


def size_inventory_matches(root: Path, inventory: Mapping[str, int]) -> bool:
    return bool(inventory) and all(
        (root / relative_path).is_file()
        and (root / relative_path).stat().st_size == expected_size
        for relative_path, expected_size in inventory.items()
    )


def implementation_fingerprint(paths: Iterable[Path]) -> str:
    """Hash selected implementation files, including uncommitted source changes."""

    digest = sha256()
    files = sorted(
        {
            file.resolve()
            for path in paths
            for file in (path.rglob("*") if path.is_dir() else (path,))
            if file.is_file()
            and "__pycache__" not in file.parts
            and file.suffix in {".json", ".py", ".rs", ".toml", ".yaml", ".yml"}
        }
    )
    for path in files:
        digest.update(str(path).encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()
