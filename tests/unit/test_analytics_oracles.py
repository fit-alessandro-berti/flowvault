from datetime import UTC, datetime, timedelta

import pytest

from saocpm_eval.analytics.causal_checks import paired_effect
from saocpm_eval.analytics.conformance import Violation, score_violations
from saocpm_eval.analytics.episodes import (
    StateEpisode,
    StateObservation,
    chattering_rate,
    episode_temporal_iou,
    extract_episodes,
    transitions_from_episodes,
)
from saocpm_eval.analytics.graph_metrics import EdgeObservation, graph_heterogeneity
from saocpm_eval.analytics.pattern_evaluation import (
    PatternRecord,
    jaccard,
    normalized_edit_similarity,
    score_pattern,
)
from saocpm_eval.analytics.performance import measure
from saocpm_eval.analytics.prediction import (
    DecisionPoint,
    expected_calibration_error,
    label_future_state,
    purge_temporal_split,
    score_episode_alerts,
)
from saocpm_eval.analytics.som_evaluation import align_cells, nearby_transition_proportion
from saocpm_eval.analytics.state_agreement import event_state_agreement, match_transitions

BASE = datetime(2025, 1, 1, tzinfo=UTC)


def observation(minutes: int, state: str, event: str) -> StateObservation:
    return StateObservation("O-1", event, BASE + timedelta(minutes=minutes), state)


def test_episode_extraction_transitions_iou_and_chattering_are_hand_calculated() -> None:
    truth_observations = [
        observation(0, "A", "e0"),
        observation(1, "A", "e1"),
        observation(3, "B", "e3"),
        observation(5, "B", "e5"),
    ]
    truth = extract_episodes(truth_observations, {"O-1": BASE + timedelta(minutes=6)})
    assert [(row.state, row.duration_seconds) for row in truth] == [("A", 180), ("B", 180)]
    transitions = transitions_from_episodes(truth)
    assert len(transitions) == 1
    assert transitions[0].time == BASE + timedelta(minutes=3)

    predicted = extract_episodes(
        [observation(0, "A", "p0"), observation(2, "B", "p2")],
        {"O-1": BASE + timedelta(minutes=6)},
    )
    assert episode_temporal_iou(truth, predicted) == pytest.approx(5 / 7)
    assert chattering_rate(predicted, 150) == pytest.approx(0.5)


def test_state_and_transition_agreement_are_hand_calculated() -> None:
    truth = [observation(0, "A", "e0"), observation(1, "A", "e1"), observation(2, "B", "e2")]
    agreement = event_state_agreement(truth, {"e0": "A", "e1": "A", "e2": "A"})
    assert agreement.coverage == 1
    assert agreement.accuracy == pytest.approx(2 / 3)
    assert agreement.macro_f1 == pytest.approx(0.4)
    assert agreement.weighted_f1 == pytest.approx(8 / 15)

    truth_transition = transitions_from_episodes(
        extract_episodes(
            [observation(0, "A", "e0"), observation(10, "B", "e10")],
            {"O-1": BASE + timedelta(minutes=20)},
        )
    )
    predicted_transition = transitions_from_episodes(
        extract_episodes(
            [observation(0, "A", "p0"), observation(12, "B", "p12")],
            {"O-1": BASE + timedelta(minutes=20)},
        )
    )
    matched = match_transitions(truth_transition, predicted_transition, timedelta(minutes=3))
    assert matched.precision == matched.recall == 1
    assert matched.median_absolute_error_seconds == 120


def test_som_alignment_and_topology_are_exact_for_separable_cells() -> None:
    result = align_cells(["0,0", "0,0", "1,1", "1,1"], ["A", "A", "B", "B"], total_cell_count=3)
    assert result.purity == 1
    assert result.adjusted_rand_index == 1
    assert result.normalized_mutual_information == 1
    assert result.balanced_accuracy == 1
    assert result.mean_cell_entropy == 0
    assert result.empty_cell_rate == pytest.approx(1 / 3)
    assert nearby_transition_proportion(["O", "O", "O"], [(0, 0), (0, 1), (2, 2)]) == pytest.approx(
        0.5
    )


def test_graph_heterogeneity_recovers_perfect_state_separation() -> None:
    rows = [
        EdgeObservation("a", "x", "S1", 1),
        EdgeObservation("a", "x", "S1", 3),
        EdgeObservation("a", "y", "S2", 5),
        EdgeObservation("a", "y", "S2", 7),
    ]
    result = graph_heterogeneity(rows)
    assert result.state_conditioned_frequency[("a", "x", "S1")] == 2
    assert result.state_conditioned_median_waiting_seconds[("a", "y", "S2")] == 6
    assert result.edge_state_entropy == 0
    assert result.conditional_mutual_information == pytest.approx(1)
    assert result.weighted_jensen_shannon_divergence == pytest.approx(0.311278, rel=1e-5)


def test_pattern_scoring_uses_edit_context_support_and_occurrences() -> None:
    assert normalized_edit_similarity(("a", "b", "c"), ("a", "x", "c")) == pytest.approx(2 / 3)
    assert jaccard(frozenset({"A", "B"}), frozenset({"B", "C"})) == pytest.approx(1 / 3)
    truth = PatternRecord(
        "P1",
        "inter",
        ("a", "b"),
        frozenset({"Machine", "Alarm"}),
        2,
        frozenset({("M1", "e1", "e2"), ("M2", "e3", "e4")}),
    )
    detected = [
        PatternRecord("d1", "inter", ("x",), frozenset({"Other"}), 1),
        PatternRecord(
            "d2",
            "inter",
            ("a", "b"),
            frozenset({"Machine", "Alarm"}),
            3,
            frozenset({("M1", "e1", "e2"), ("M3", "e5", "e6")}),
        ),
    ]
    score = score_pattern(truth, detected, top_k=2)
    assert score.rank == 2
    assert score.top_k_hit is True
    assert score.support_absolute_error == 1
    assert score.support_relative_error == 0.5
    assert score.sequence_similarity == score.context_jaccard == 1
    assert score.occurrence_precision == score.occurrence_recall == 0.5


def test_conformance_matching_is_one_to_one_and_tolerance_aware() -> None:
    truth = [
        Violation("R1", "O1", BASE, "e1"),
        Violation("R2", "O2", BASE, "e2"),
    ]
    detected = [
        Violation("R1", "O1", BASE + timedelta(seconds=30), "d1"),
        Violation("R3", "O3", BASE, "d2"),
    ]
    score = score_violations(truth, detected, timedelta(minutes=1))
    assert score.precision == score.recall == 0.5
    assert score.median_timing_error_seconds == 30


def test_prediction_labeling_purging_calibration_and_alert_scoring() -> None:
    points = [
        DecisionPoint("O-1", "e0", BASE - timedelta(minutes=5), BASE, "Normal"),
        DecisionPoint(
            "O-1",
            "e1",
            BASE + timedelta(minutes=15),
            BASE + timedelta(minutes=20),
            "Normal",
        ),
    ]
    labels = label_future_state(
        points,
        [observation(10, "Down", "failure")],
        target_states=frozenset({"Down"}),
        horizon=timedelta(minutes=15),
    )
    assert [row.label for row in labels] == [True, False]
    train, test = purge_temporal_split(points, BASE + timedelta(minutes=10), timedelta(minutes=10))
    assert train == [points[0]]
    assert test == [points[1]]
    assert expected_calibration_error([False, True], [0.1, 0.9], bins=2) == pytest.approx(0.1)

    episodes = [
        StateEpisode(
            "O-1",
            "Down",
            BASE + timedelta(minutes=10),
            BASE + timedelta(minutes=20),
            "f1",
            "f2",
            2,
            False,
        ),
        StateEpisode(
            "O-1",
            "Down",
            BASE + timedelta(minutes=40),
            BASE + timedelta(minutes=50),
            "f3",
            "f4",
            2,
            False,
        ),
    ]
    alerts = score_episode_alerts(
        [("O-1", BASE), ("O-1", BASE + timedelta(minutes=1))],
        episodes,
        timedelta(minutes=15),
    )
    assert alerts.true_alerts == 1
    assert alerts.false_alerts == 1
    assert alerts.event_sensitivity == 0.5
    assert alerts.median_lead_time_seconds == 600


def test_performance_and_paired_causal_oracles_return_expected_values() -> None:
    measured = measure(lambda: sum(range(10)))
    assert measured.value == 45
    assert measured.wall_time_ms >= 0
    assert measured.cpu_time_ms >= 0
    assert measured.peak_rss_bytes > 0
    effect = paired_effect([3, 5, 7], [1, 2, 3], truth_effect=3)
    assert effect.average_treatment_effect == 3
    assert effect.sign_matches_truth is True
    assert effect.magnitude_error == 0
