"""Injected-pattern retrieval, similarity, support, and occurrence scoring."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class PatternRecord:
    pattern_id: str
    family: str
    sequence: tuple[str, ...]
    object_types: frozenset[str]
    support: int
    occurrences: frozenset[tuple[str, str, str]] = frozenset()


@dataclass(frozen=True, slots=True)
class PatternScore:
    pattern_id: str
    rank: int | None
    top_k_hit: bool
    support_truth: int
    support_detected: int | None
    support_absolute_error: int | None
    support_relative_error: float | None
    sequence_similarity: float
    context_jaccard: float
    occurrence_precision: float | None
    occurrence_recall: float | None


def normalized_edit_similarity(left: tuple[str, ...], right: tuple[str, ...]) -> float:
    if not left and not right:
        return 1.0
    previous = list(range(len(right) + 1))
    for left_index, left_value in enumerate(left, start=1):
        current = [left_index]
        for right_index, right_value in enumerate(right, start=1):
            current.append(
                min(
                    current[-1] + 1,
                    previous[right_index] + 1,
                    previous[right_index - 1] + (left_value != right_value),
                )
            )
        previous = current
    return 1.0 - previous[-1] / max(len(left), len(right))


def jaccard(left: frozenset[str], right: frozenset[str]) -> float:
    union = left.union(right)
    return len(left.intersection(right)) / len(union) if union else 1.0


def score_pattern(
    truth: PatternRecord,
    detected: list[PatternRecord],
    *,
    top_k: int = 10,
) -> PatternScore:
    candidates = [row for row in detected if row.family == truth.family]
    if not candidates:
        return PatternScore(
            pattern_id=truth.pattern_id,
            rank=None,
            top_k_hit=False,
            support_truth=truth.support,
            support_detected=None,
            support_absolute_error=None,
            support_relative_error=None,
            sequence_similarity=0.0,
            context_jaccard=0.0,
            occurrence_precision=None,
            occurrence_recall=None,
        )
    similarities = [
        (
            0.7 * normalized_edit_similarity(truth.sequence, row.sequence)
            + 0.3 * jaccard(truth.object_types, row.object_types),
            index,
            row,
        )
        for index, row in enumerate(candidates, start=1)
    ]
    _, rank, best = max(similarities, key=lambda item: (item[0], -item[1]))
    sequence_similarity = normalized_edit_similarity(truth.sequence, best.sequence)
    context_similarity = jaccard(truth.object_types, best.object_types)
    intersection = len(truth.occurrences.intersection(best.occurrences))
    occurrence_precision = intersection / len(best.occurrences) if best.occurrences else None
    occurrence_recall = intersection / len(truth.occurrences) if truth.occurrences else None
    support_error = best.support - truth.support
    return PatternScore(
        pattern_id=truth.pattern_id,
        rank=rank,
        top_k_hit=rank <= top_k,
        support_truth=truth.support,
        support_detected=best.support,
        support_absolute_error=abs(support_error),
        support_relative_error=(abs(support_error) / truth.support if truth.support else None),
        sequence_similarity=sequence_similarity,
        context_jaccard=context_similarity,
        occurrence_precision=occurrence_precision,
        occurrence_recall=occurrence_recall,
    )
