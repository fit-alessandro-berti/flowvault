"""Conformance violation matching against injected truth."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timedelta

import numpy as np


@dataclass(frozen=True, slots=True)
class Violation:
    rule_id: str
    object_id: str
    time: datetime
    event_id: str


@dataclass(frozen=True, slots=True)
class ConformanceScore:
    precision: float
    recall: float
    matched: int
    false_positives: int
    false_negatives: int
    median_timing_error_seconds: float | None


def score_violations(
    truth: Iterable[Violation],
    detected: Iterable[Violation],
    tolerance: timedelta,
) -> ConformanceScore:
    truth_rows = list(truth)
    detected_rows = list(detected)
    unused = set(range(len(detected_rows)))
    errors: list[float] = []
    for expected in truth_rows:
        candidates = [
            (abs((detected_rows[index].time - expected.time).total_seconds()), index)
            for index in unused
            if detected_rows[index].rule_id == expected.rule_id
            and detected_rows[index].object_id == expected.object_id
        ]
        if not candidates:
            continue
        error, index = min(candidates)
        if error <= tolerance.total_seconds():
            unused.remove(index)
            errors.append(error)
    matched = len(errors)
    return ConformanceScore(
        precision=matched / len(detected_rows) if detected_rows else 1.0,
        recall=matched / len(truth_rows) if truth_rows else 1.0,
        matched=matched,
        false_positives=len(detected_rows) - matched,
        false_negatives=len(truth_rows) - matched,
        median_timing_error_seconds=float(np.median(errors)) if errors else None,
    )
