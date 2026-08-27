from pathlib import Path

from saocpm_eval.analytics.runner import _completed_analysis_matches
from saocpm_eval.completion import (
    atomic_write_json,
    file_snapshot,
    size_inventory,
    snapshot_matches,
)


def test_analysis_completion_requires_matching_identity_and_outputs(tmp_path: Path) -> None:
    output = tmp_path / "analytics" / "scores.csv"
    output.parent.mkdir()
    output.write_text("metric,value\nscore,1\n", encoding="utf-8")
    identity = {"execution_fingerprint": "dataset-and-code-fingerprint"}
    atomic_write_json(
        tmp_path / "analytics" / "analysis_manifest.json",
        {
            **identity,
            "complete": True,
            "input_snapshot": file_snapshot(tmp_path, ("analytics/scores.csv",)),
            "output_inventory": size_inventory(tmp_path, ("analytics/scores.csv",)),
        },
    )

    assert _completed_analysis_matches(tmp_path, identity)
    assert not _completed_analysis_matches(tmp_path, {"execution_fingerprint": "changed"})
    output.write_text("metric,value\n", encoding="utf-8")
    assert not _completed_analysis_matches(tmp_path, identity)


def test_validation_snapshot_detects_an_input_change(tmp_path: Path) -> None:
    source = tmp_path / "observed.ocel.json"
    source.write_text("{}\n", encoding="utf-8")
    snapshot = file_snapshot(tmp_path, ("observed.ocel.json",))
    assert snapshot_matches(tmp_path, snapshot)
    source.write_text('{"changed":true}\n', encoding="utf-8")
    assert not snapshot_matches(tmp_path, snapshot)
