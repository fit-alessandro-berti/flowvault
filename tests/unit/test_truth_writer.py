from datetime import UTC, datetime
from pathlib import Path

import jsonschema
import pytest

from saocpm_eval.common.hashing import config_sha256, sha256_file
from saocpm_eval.common.rng import SeedTree
from saocpm_eval.common.truth_writer import RunWriter, read_manifest

REPOSITORY_ROOT = Path(__file__).parents[2]


def write_reproducible_run(root: Path) -> None:
    tree = SeedTree(20260826)
    value = int(tree.stream("entity parameters").integers(1, 100))
    writer = RunWriter(root)
    writer.prepare()
    writer.write_json("expected/summary.json", {"sample": value})
    writer.write_csv(
        "truth/state_at_event.csv",
        ("event_id", "data_complete", "factors_json"),
        [
            {
                "event_id": "E-001",
                "data_complete": True,
                "factors_json": {"primary": "Nominal"},
            }
        ],
    )
    writer.write_manifest(
        scenario="inventory",
        profile="golden",
        seed=20260826,
        config_sha256=config_sha256({"scenario": "inventory", "seed": 20260826}),
        generator_commit="test-commit",
        start_time=datetime(2025, 1, 1, tzinfo=UTC),
        end_time=datetime(2025, 1, 2, tzinfo=UTC),
        counts={"events": 1, "objects": 1, "e2o": 1, "o2o": 0, "leading_objects": 1},
        rng_streams=tree.metadata(),
        expected_counts={"states": 1},
    )


def test_same_seed_and_config_are_byte_reproducible(tmp_path: Path) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    write_reproducible_run(first)
    write_reproducible_run(second)
    first_files = sorted(item.relative_to(first) for item in first.rglob("*") if item.is_file())
    second_files = sorted(item.relative_to(second) for item in second.rglob("*") if item.is_file())
    assert first_files == second_files
    for relative in first_files:
        assert (first / relative).read_bytes() == (second / relative).read_bytes()


def test_manifest_contains_hashes_and_sizes(tmp_path: Path) -> None:
    root = tmp_path / "run"
    write_reproducible_run(root)
    manifest = read_manifest(root / "manifest.json")
    summary = manifest["files"]["expected/summary.json"]
    assert summary["sha256"] == sha256_file(root / "expected/summary.json")
    assert summary["bytes"] == (root / "expected/summary.json").stat().st_size
    schema = read_manifest(REPOSITORY_ROOT / "schemas" / "manifest.schema.json")
    jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker()).validate(
        manifest
    )


def test_writer_refuses_nonempty_output_and_unsafe_paths(tmp_path: Path) -> None:
    root = tmp_path / "run"
    root.mkdir()
    (root / "existing").write_text("user data", encoding="utf-8")
    with pytest.raises(ValueError, match="not empty"):
        RunWriter(root).prepare()
    with pytest.raises(ValueError, match="unsafe"):
        RunWriter(tmp_path).write_text("../outside", "bad")
