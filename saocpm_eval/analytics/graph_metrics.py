"""State-conditioned edge heterogeneity measures."""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Iterable
from dataclasses import dataclass

import numpy as np


@dataclass(frozen=True, slots=True)
class EdgeObservation:
    activity: str
    next_activity: str
    state: str
    waiting_seconds: float


@dataclass(frozen=True, slots=True)
class GraphHeterogeneity:
    state_conditioned_frequency: dict[tuple[str, str, str], int]
    state_conditioned_median_waiting_seconds: dict[tuple[str, str, str], float]
    edge_state_entropy: float
    weighted_jensen_shannon_divergence: float
    conditional_mutual_information: float


def _distribution(counter: Counter[str], support: list[str]) -> np.ndarray:
    total = sum(counter.values())
    return np.array([counter[item] / total if total else 0.0 for item in support])


def _kl(left: np.ndarray, right: np.ndarray) -> float:
    selected = left > 0
    return float(np.sum(left[selected] * np.log2(left[selected] / right[selected])))


def graph_heterogeneity(rows: Iterable[EdgeObservation]) -> GraphHeterogeneity:
    observations = list(rows)
    if not observations:
        raise ValueError("graph heterogeneity requires edge observations")
    frequencies = Counter((row.activity, row.next_activity, row.state) for row in observations)
    waits: dict[tuple[str, str, str], list[float]] = defaultdict(list)
    by_activity: dict[str, list[EdgeObservation]] = defaultdict(list)
    for row in observations:
        waits[(row.activity, row.next_activity, row.state)].append(row.waiting_seconds)
        by_activity[row.activity].append(row)
    median_waits = {key: float(np.median(values)) for key, values in waits.items()}
    entropy_total = 0.0
    js_total = 0.0
    mi_total = 0.0
    for activity, activity_rows in by_activity.items():
        del activity
        activity_weight = len(activity_rows) / len(observations)
        next_values = sorted({row.next_activity for row in activity_rows})
        overall = _distribution(Counter(row.next_activity for row in activity_rows), next_values)
        states = sorted({row.state for row in activity_rows})
        for state in states:
            state_rows = [row for row in activity_rows if row.state == state]
            state_weight = len(state_rows) / len(activity_rows)
            conditional = _distribution(
                Counter(row.next_activity for row in state_rows), next_values
            )
            midpoint = 0.5 * (conditional + overall)
            js_total += (
                activity_weight
                * state_weight
                * 0.5
                * (_kl(conditional, midpoint) + _kl(overall, midpoint))
            )
            mi_total += activity_weight * state_weight * _kl(conditional, overall)
        for next_activity in next_values:
            edge_rows = [row for row in activity_rows if row.next_activity == next_activity]
            probabilities = np.array(
                [sum(row.state == state for row in edge_rows) / len(edge_rows) for state in states]
            )
            nonzero = probabilities[probabilities > 0]
            entropy = float(-np.sum(nonzero * np.log2(nonzero)))
            entropy_total += len(edge_rows) / len(observations) * entropy
    return GraphHeterogeneity(
        state_conditioned_frequency=dict(frequencies),
        state_conditioned_median_waiting_seconds=median_waits,
        edge_state_entropy=entropy_total,
        weighted_jensen_shannon_divergence=js_total,
        conditional_mutual_information=mi_total,
    )
