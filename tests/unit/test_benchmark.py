from saocpm_eval.analytics.benchmark import _counts, _derived_counts
from saocpm_eval.config import ScaleProfile


def test_benchmark_counts_distinguish_objects_from_leading_objects() -> None:
    profile = ScaleProfile(
        id="inventory-test",
        scenario="inventory",
        target_events=100,
        leading_objects=10,
    )
    counts = _counts(
        {
            "events": 100,
            "objects": 17,
            "e2o_relationships": 120,
            "o2o_relationships": 8,
        },
        profile,
    )
    assert counts["leading_object_count"] == 10
    assert counts["average_lifecycle_length"] == 10
    assert counts["e2o_density"] == 1.2
    assert counts["event_attribute_value_count"] == 600


def test_benchmark_derives_state_transition_feature_and_pattern_dimensions() -> None:
    assert _derived_counts("apply_state_query", {"assigned_events": 12})["state_count"] == 12
    assert (
        _derived_counts(
            "state_transition_kpis",
            {"transitions": [{"count": 3}, {"count": 4}]},
        )["transition_frequency"]
        == 7
    )
    detection = _derived_counts(
        "state_detection_and_assignments", {"feature_count": 9, "window_count": 42}
    )
    assert detection["feature_count"] == 9
    assert detection["window_count"] == 42
    patterns = _derived_counts("state_patterns", {"intra": [{}, {}], "inter": [{}]})
    assert patterns["pattern_count"] == 3
