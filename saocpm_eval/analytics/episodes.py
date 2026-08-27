"""Independent state-episode extraction and temporal overlap metrics."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from datetime import datetime
from itertools import pairwise


@dataclass(frozen=True, slots=True)
class StateObservation:
    object_id: str
    event_id: str
    time: datetime
    state: str


@dataclass(frozen=True, slots=True)
class StateEpisode:
    object_id: str
    state: str
    start_time: datetime
    end_time: datetime
    start_event_id: str
    end_event_id: str
    event_count: int
    right_censored: bool

    @property
    def duration_seconds(self) -> float:
        return max(0.0, (self.end_time - self.start_time).total_seconds())


@dataclass(frozen=True, slots=True)
class StateTransition:
    object_id: str
    from_state: str
    to_state: str
    time: datetime
    event_id: str
    from_state_started_at: datetime


def extract_episodes(
    observations: Iterable[StateObservation],
    horizons: Mapping[str, datetime] | None = None,
) -> list[StateEpisode]:
    grouped: dict[str, list[StateObservation]] = defaultdict(list)
    for observation in observations:
        grouped[observation.object_id].append(observation)
    episodes: list[StateEpisode] = []
    for object_id, rows in sorted(grouped.items()):
        ordered = sorted(rows, key=lambda row: (row.time, row.event_id))
        if len({(row.time, row.event_id) for row in ordered}) != len(ordered):
            raise ValueError(f"duplicate state observation for {object_id}")
        start = 0
        for index in range(1, len(ordered) + 1):
            boundary = index == len(ordered) or ordered[index].state != ordered[start].state
            if not boundary:
                continue
            start_row = ordered[start]
            last_inside = ordered[index - 1]
            if index < len(ordered):
                end_time = ordered[index].time
                end_event = ordered[index].event_id
                right_censored = False
            else:
                end_time = (horizons or {}).get(object_id, last_inside.time)
                if end_time < last_inside.time:
                    raise ValueError(f"horizon precedes the final event for {object_id}")
                end_event = last_inside.event_id
                right_censored = True
            episodes.append(
                StateEpisode(
                    object_id=object_id,
                    state=start_row.state,
                    start_time=start_row.time,
                    end_time=end_time,
                    start_event_id=start_row.event_id,
                    end_event_id=end_event,
                    event_count=index - start,
                    right_censored=right_censored,
                )
            )
            start = index
    return episodes


def transitions_from_episodes(episodes: Iterable[StateEpisode]) -> list[StateTransition]:
    grouped: dict[str, list[StateEpisode]] = defaultdict(list)
    for episode in episodes:
        grouped[episode.object_id].append(episode)
    transitions: list[StateTransition] = []
    for object_id, rows in sorted(grouped.items()):
        ordered = sorted(rows, key=lambda row: row.start_time)
        for left, right in pairwise(ordered):
            if left.state == right.state:
                raise ValueError("adjacent extracted episodes must have different states")
            transitions.append(
                StateTransition(
                    object_id=object_id,
                    from_state=left.state,
                    to_state=right.state,
                    time=right.start_time,
                    event_id=right.start_event_id,
                    from_state_started_at=left.start_time,
                )
            )
    return transitions


def episode_temporal_iou(truth: Iterable[StateEpisode], predicted: Iterable[StateEpisode]) -> float:
    truth_grouped: dict[tuple[str, str], list[StateEpisode]] = defaultdict(list)
    predicted_grouped: dict[tuple[str, str], list[StateEpisode]] = defaultdict(list)
    for episode in truth:
        truth_grouped[(episode.object_id, episode.state)].append(episode)
    for episode in predicted:
        predicted_grouped[(episode.object_id, episode.state)].append(episode)
    intersection = 0.0
    truth_duration = 0.0
    predicted_duration = 0.0
    keys = set(truth_grouped).union(predicted_grouped)
    for key in keys:
        left = sorted(truth_grouped[key], key=lambda item: item.start_time)
        right = sorted(predicted_grouped[key], key=lambda item: item.start_time)
        truth_duration += sum(item.duration_seconds for item in left)
        predicted_duration += sum(item.duration_seconds for item in right)
        left_index = 0
        right_index = 0
        while left_index < len(left) and right_index < len(right):
            truth_episode = left[left_index]
            predicted_episode = right[right_index]
            start = max(truth_episode.start_time, predicted_episode.start_time)
            end = min(truth_episode.end_time, predicted_episode.end_time)
            intersection += max(0.0, (end - start).total_seconds())
            if truth_episode.end_time <= predicted_episode.end_time:
                left_index += 1
            else:
                right_index += 1
    union = truth_duration + predicted_duration - intersection
    return intersection / union if union else 1.0


def chattering_rate(episodes: Iterable[StateEpisode], minimum_seconds: float) -> float:
    rows = list(episodes)
    if minimum_seconds < 0:
        raise ValueError("minimum episode duration must be non-negative")
    return (
        sum(episode.duration_seconds < minimum_seconds for episode in rows) / len(rows)
        if rows
        else 0.0
    )
