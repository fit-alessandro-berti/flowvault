from datetime import UTC, datetime, timedelta

import pandas as pd
import pytest

from saocpm_eval.analytics.prediction_baselines import (
    DataSplit,
    assert_no_window_overlap,
    concordance_index,
    evaluate_baselines,
    evaluate_regression_baselines,
    feature_columns,
    grouped_split,
)


def synthetic_frame() -> pd.DataFrame:
    base = datetime(2025, 1, 1, tzinfo=UTC)
    rows = []
    for group in range(8):
        for index in range(20):
            cutoff = base + timedelta(days=index, hours=group)
            signal = (index + group) % 5
            rows.append(
                {
                    "object_id": f"O-{group}",
                    "split_group": f"O-{group}",
                    "feature_start": cutoff - timedelta(hours=1),
                    "cutoff_time": cutoff,
                    "label": signal >= 3,
                    "time_to_event_minutes": 60.0 if signal >= 3 else float("nan"),
                    "static.class": "A" if group % 2 else "B",
                    "raw.signal": float(signal),
                    "process.count": index % 4,
                    "state.current": "Normal" if signal < 3 else "Degraded",
                    "state.dwell": index,
                    "context.count": group % 3,
                }
            )
    return pd.DataFrame(rows)


def test_group_split_is_disjoint_and_reproducible() -> None:
    frame = synthetic_frame()
    first = grouped_split(frame, seed=9)
    second = grouped_split(frame, seed=9)
    assert first == second
    train_groups = set(frame.loc[list(first.train_index), "split_group"])
    test_groups = set(frame.loc[list(first.test_index), "split_group"])
    assert train_groups.isdisjoint(test_groups)
    assert_no_window_overlap(frame, first)


def test_overlap_guard_rejects_overlapping_windows() -> None:
    frame = synthetic_frame().iloc[:2].copy()
    frame.loc[1, "cutoff_time"] = frame.loc[0, "cutoff_time"] + timedelta(minutes=30)
    frame.loc[1, "feature_start"] = frame.loc[0, "cutoff_time"] - timedelta(minutes=30)
    split = DataSplit("bad", (0,), (1,))
    with pytest.raises(ValueError, match="overlapping"):
        assert_no_window_overlap(frame, split)


def test_feature_sets_reject_leakage() -> None:
    frame = synthetic_frame()
    frame["raw.future_label"] = 1
    with pytest.raises(ValueError, match="leakage"):
        feature_columns(frame, "raw-only")


def test_baselines_are_reproducible_for_all_required_feature_sets() -> None:
    frame = synthetic_frame()
    frame["static.class"] = frame["static.class"].astype(object)
    frame.loc[0, "static.class"] = 7
    split = grouped_split(frame, test_fraction=0.25, seed=11)
    first = evaluate_baselines(frame, split, seed=21)
    second = evaluate_baselines(frame, split, seed=21)
    assert first == second
    assert len(first) == 12
    assert {row.feature_set for row in first} == {
        "static-only",
        "raw-only",
        "process-only",
        "state-only",
        "object-context-only",
        "full",
    }
    assert all(0 <= row.auprc <= 1 for row in first)


def test_time_to_event_baselines_report_mae_and_concordance() -> None:
    frame = synthetic_frame()
    split = grouped_split(frame, test_fraction=0.25, seed=11)
    scores = evaluate_regression_baselines(frame, split, seed=21)
    assert len(scores) == 12
    assert all(row.mae_minutes >= 0 for row in scores)
    assert all(0 <= row.concordance <= 1 for row in scores)
    assert concordance_index([1.0, 2.0, 3.0], [1.5, 2.5, 4.0]) == 1.0
