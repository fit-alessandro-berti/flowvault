"""Leakage-safe outcome labels, temporal purging, calibration, and alert scoring."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from datetime import datetime, timedelta

import numpy as np

from saocpm_eval.analytics.episodes import StateEpisode, StateObservation


@dataclass(frozen=True, slots=True)
class DecisionPoint:
    object_id: str
    event_id: str
    feature_start: datetime
    cutoff_time: datetime
    current_state: str


@dataclass(frozen=True, slots=True)
class LabeledDecision:
    point: DecisionPoint
    label: bool
    time_to_event_seconds: float | None


@dataclass(frozen=True, slots=True)
class AlertScore:
    true_alerts: int
    false_alerts: int
    detected_episodes: int
    total_episodes: int
    event_sensitivity: float
    median_lead_time_seconds: float | None


def label_future_state(
    points: Iterable[DecisionPoint],
    observations: Iterable[StateObservation],
    *,
    target_states: frozenset[str],
    horizon: timedelta,
) -> list[LabeledDecision]:
    if horizon.total_seconds() <= 0:
        raise ValueError("prediction horizon must be positive")
    grouped: dict[str, list[StateObservation]] = defaultdict(list)
    for observation in observations:
        grouped[observation.object_id].append(observation)
    for rows in grouped.values():
        rows.sort(key=lambda row: row.time)
    labeled = []
    for point in points:
        future = next(
            (
                row
                for row in grouped.get(point.object_id, [])
                if point.cutoff_time < row.time <= point.cutoff_time + horizon
                and row.state in target_states
            ),
            None,
        )
        labeled.append(
            LabeledDecision(
                point=point,
                label=future is not None,
                time_to_event_seconds=(
                    (future.time - point.cutoff_time).total_seconds() if future else None
                ),
            )
        )
    return labeled


def purge_temporal_split(
    points: Iterable[DecisionPoint],
    split_time: datetime,
    prediction_horizon: timedelta,
) -> tuple[list[DecisionPoint], list[DecisionPoint]]:
    """Drop training labels crossing the split and test windows starting before it."""

    train = [point for point in points if point.cutoff_time + prediction_horizon <= split_time]
    test = [point for point in points if point.feature_start >= split_time]
    return train, test


def expected_calibration_error(
    labels: Sequence[int | bool], probabilities: Sequence[float], bins: int = 10
) -> float:
    if len(labels) != len(probabilities) or not labels:
        raise ValueError("ECE requires equal non-empty label and probability sequences")
    if bins < 1 or any(not 0 <= probability <= 1 for probability in probabilities):
        raise ValueError("invalid ECE bins or probabilities")
    labels_array = np.asarray(labels, dtype=float)
    probabilities_array = np.asarray(probabilities, dtype=float)
    edges = np.linspace(0, 1, bins + 1)
    error = 0.0
    for index in range(bins):
        selected = (probabilities_array >= edges[index]) & (
            probabilities_array <= edges[index + 1]
            if index == bins - 1
            else probabilities_array < edges[index + 1]
        )
        if not np.any(selected):
            continue
        error += float(np.mean(selected)) * abs(
            float(np.mean(labels_array[selected])) - float(np.mean(probabilities_array[selected]))
        )
    return error


def score_episode_alerts(
    alert_times: Iterable[tuple[str, datetime]],
    target_episodes: Iterable[StateEpisode],
    horizon: timedelta,
) -> AlertScore:
    alerts = sorted(alert_times, key=lambda item: item[1])
    episodes = list(target_episodes)
    matched_episodes: set[int] = set()
    lead_times: list[float] = []
    false_alerts = 0
    for object_id, alert_time in alerts:
        match = next(
            (
                (index, episode)
                for index, episode in enumerate(episodes)
                if index not in matched_episodes
                and episode.object_id == object_id
                and alert_time < episode.start_time <= alert_time + horizon
            ),
            None,
        )
        if match is None:
            false_alerts += 1
            continue
        index, episode = match
        matched_episodes.add(index)
        lead_times.append((episode.start_time - alert_time).total_seconds())
    return AlertScore(
        true_alerts=len(matched_episodes),
        false_alerts=false_alerts,
        detected_episodes=len(matched_episodes),
        total_episodes=len(episodes),
        event_sensitivity=len(matched_episodes) / len(episodes) if episodes else 1.0,
        median_lead_time_seconds=float(np.median(lead_times)) if lead_times else None,
    )
