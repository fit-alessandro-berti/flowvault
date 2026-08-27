"""Event-state agreement and tolerance-aware transition matching."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from datetime import timedelta

import numpy as np
from sklearn.metrics import f1_score, precision_recall_fscore_support

from saocpm_eval.analytics.episodes import StateObservation, StateTransition


@dataclass(frozen=True, slots=True)
class TransitionMatch:
    truth: StateTransition
    predicted: StateTransition
    absolute_error_seconds: float


@dataclass(frozen=True, slots=True)
class TransitionAgreement:
    precision: float
    recall: float
    matches: tuple[TransitionMatch, ...]
    median_absolute_error_seconds: float | None
    percentile_95_absolute_error_seconds: float | None


@dataclass(frozen=True, slots=True)
class StateAgreement:
    coverage: float
    accuracy: float
    macro_f1: float
    weighted_f1: float
    per_state: Mapping[str, Mapping[str, float]]
    unknown_exposure: float


def event_state_agreement(
    truth: Iterable[StateObservation],
    predicted_by_event: Mapping[str, str],
    *,
    unknown_state: str = "Unknown",
) -> StateAgreement:
    truth_rows = list(truth)
    if not truth_rows:
        raise ValueError("state agreement requires truth observations")
    truth_labels = [row.state for row in truth_rows]
    assigned = [row.event_id in predicted_by_event for row in truth_rows]
    predicted_labels = [predicted_by_event.get(row.event_id, "__MISSING__") for row in truth_rows]
    labels = sorted(set(truth_labels))
    precision, recall, f1, _ = precision_recall_fscore_support(
        truth_labels,
        predicted_labels,
        labels=labels,
        zero_division=0,
    )
    per_state = {
        label: {
            "precision": float(precision[index]),
            "recall": float(recall[index]),
            "f1": float(f1[index]),
        }
        for index, label in enumerate(labels)
    }
    return StateAgreement(
        coverage=sum(assigned) / len(assigned),
        accuracy=sum(
            left == right for left, right in zip(truth_labels, predicted_labels, strict=True)
        )
        / len(truth_labels),
        macro_f1=float(
            f1_score(
                truth_labels,
                predicted_labels,
                labels=labels,
                average="macro",
                zero_division=0,
            )
        ),
        weighted_f1=float(
            f1_score(
                truth_labels,
                predicted_labels,
                labels=labels,
                average="weighted",
                zero_division=0,
            )
        ),
        per_state=per_state,
        unknown_exposure=sum(label == unknown_state for label in truth_labels) / len(truth_labels),
    )


def match_transitions(
    truth: Iterable[StateTransition],
    predicted: Iterable[StateTransition],
    tolerance: timedelta,
) -> TransitionAgreement:
    if tolerance.total_seconds() < 0:
        raise ValueError("transition matching tolerance must be non-negative")
    truth_rows = list(truth)
    predicted_rows = list(predicted)
    matches: list[TransitionMatch] = []
    truth_groups: dict[tuple[str, str, str], list[StateTransition]] = defaultdict(list)
    predicted_groups: dict[tuple[str, str, str], list[StateTransition]] = defaultdict(list)
    for row in truth_rows:
        truth_groups[(row.object_id, row.from_state, row.to_state)].append(row)
    for row in predicted_rows:
        predicted_groups[(row.object_id, row.from_state, row.to_state)].append(row)
    tolerance_seconds = tolerance.total_seconds()
    for key, expected_group in truth_groups.items():
        expected_group.sort(key=lambda row: (row.time, row.event_id))
        candidates = sorted(predicted_groups.get(key, []), key=lambda row: (row.time, row.event_id))
        candidate_index = 0
        for expected in expected_group:
            earliest = expected.time - tolerance
            latest = expected.time + tolerance
            while candidate_index < len(candidates) and candidates[candidate_index].time < earliest:
                candidate_index += 1
            if candidate_index >= len(candidates) or candidates[candidate_index].time > latest:
                continue
            best = candidate_index
            while best + 1 < len(candidates) and candidates[best + 1].time <= latest:
                current_error = abs((candidates[best].time - expected.time).total_seconds())
                next_error = abs((candidates[best + 1].time - expected.time).total_seconds())
                if next_error >= current_error:
                    break
                best += 1
            selected = candidates[best]
            error = abs((selected.time - expected.time).total_seconds())
            if error <= tolerance_seconds:
                matches.append(TransitionMatch(expected, selected, error))
                candidate_index = best + 1
    errors = np.array([match.absolute_error_seconds for match in matches], dtype=float)
    return TransitionAgreement(
        precision=len(matches) / len(predicted_rows) if predicted_rows else 1.0,
        recall=len(matches) / len(truth_rows) if truth_rows else 1.0,
        matches=tuple(matches),
        median_absolute_error_seconds=float(np.median(errors)) if len(errors) else None,
        percentile_95_absolute_error_seconds=(
            float(np.percentile(errors, 95)) if len(errors) else None
        ),
    )
