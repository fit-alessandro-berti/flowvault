from pathlib import Path

from saocpm_eval.analytics.features import build_prediction_features, prepare_prediction_context

REPOSITORY_ROOT = Path(__file__).parents[2]


def test_prediction_features_can_be_built_one_task_at_a_time() -> None:
    run = REPOSITORY_ROOT / "files" / "ocel2" / "evaluation" / "inventory_golden"
    task = "Understock within 7 days"
    frame = build_prediction_features(run, task=task)
    assert len(frame) == 40
    assert set(frame["task"]) == {task}
    assert frame["state.transition_count"].min() == 0
    assert frame["state.transition_count"].max() > 0
    assert frame["feature_start"].le(frame["cutoff_time"]).all()


def test_prediction_feature_sampling_is_bounded_stratified_and_deterministic() -> None:
    run = REPOSITORY_ROOT / "files" / "ocel2" / "evaluation" / "inventory_golden"
    task = "Understock within 7 days"
    context = prepare_prediction_context(run)
    first = build_prediction_features(run, task=task, context=context, max_samples=12)
    second = build_prediction_features(run, task=task, context=context, max_samples=12)
    assert len(first) == 12
    assert set(first["label"]) == {False, True}
    assert first["cutoff_event_id"].tolist() == second["cutoff_event_id"].tolist()
