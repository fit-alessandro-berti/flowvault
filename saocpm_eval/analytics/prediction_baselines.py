"""Reproducible baseline pipelines and required feature-set ablations."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import timedelta

import numpy as np
import pandas as pd
from sklearn.compose import ColumnTransformer
from sklearn.ensemble import HistGradientBoostingClassifier, HistGradientBoostingRegressor
from sklearn.impute import SimpleImputer
from sklearn.linear_model import LogisticRegression, Ridge
from sklearn.metrics import (
    average_precision_score,
    brier_score_loss,
    mean_absolute_error,
    roc_auc_score,
)
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import OneHotEncoder, StandardScaler

from saocpm_eval.analytics.prediction import expected_calibration_error

FEATURE_SETS: dict[str, tuple[str, ...]] = {
    "static-only": ("static.",),
    "raw-only": ("raw.",),
    "process-only": ("process.",),
    "state-only": ("state.", "automatic."),
    "object-context-only": ("context.",),
    "full": ("static.", "raw.", "process.", "state.", "automatic.", "context."),
}

FORBIDDEN_FEATURE_TERMS = ("label", "future", "outcome", "time_to_event", "reference_state")


@dataclass(frozen=True, slots=True)
class DataSplit:
    name: str
    train_index: tuple[int, ...]
    test_index: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class BaselineScore:
    model: str
    feature_set: str
    split: str
    sample_count: int
    positive_count: int
    auprc: float
    auroc: float | None
    brier: float
    ece: float
    recall_at_alert_budget: float
    median_warning_lead_time_minutes: float | None
    false_alerts_per_object_week: float


@dataclass(frozen=True, slots=True)
class RegressionScore:
    model: str
    feature_set: str
    split: str
    sample_count: int
    mae_minutes: float
    concordance: float


def feature_columns(frame: pd.DataFrame, feature_set: str) -> list[str]:
    prefixes = FEATURE_SETS.get(feature_set)
    if prefixes is None:
        raise ValueError(f"unknown feature set {feature_set!r}")
    columns = [column for column in frame.columns if column.startswith(prefixes)]
    leaked = [
        column
        for column in columns
        if any(term in column.lower() for term in FORBIDDEN_FEATURE_TERMS)
    ]
    if leaked:
        raise ValueError(f"future or label leakage in feature columns: {leaked}")
    if not columns:
        raise ValueError(f"feature set {feature_set!r} has no available columns")
    return sorted(columns)


def temporal_split(
    frame: pd.DataFrame,
    *,
    prediction_horizon: timedelta,
    test_fraction: float = 0.25,
) -> DataSplit:
    if not 0 < test_fraction < 1:
        raise ValueError("test fraction must be in (0, 1)")
    split_time = pd.Timestamp(frame["cutoff_time"].quantile(1 - test_fraction))
    train = frame.index[frame["cutoff_time"] + prediction_horizon <= split_time].tolist()
    test = frame.index[frame["feature_start"] >= split_time].tolist()
    if not train or not test:
        raise ValueError("temporal split is empty after horizon purging")
    return DataSplit("temporal-holdout", tuple(train), tuple(test))


def grouped_split(
    frame: pd.DataFrame,
    *,
    test_fraction: float = 0.25,
    seed: int = 20260826,
) -> DataSplit:
    groups = sorted(frame["split_group"].astype(str).unique())
    if len(groups) < 2:
        raise ValueError("grouped split requires at least two groups")
    rng = np.random.default_rng(seed)
    shuffled = list(rng.permutation(groups))
    test_count = max(1, round(len(groups) * test_fraction))
    test_groups = set(shuffled[:test_count])
    test = frame.index[frame["split_group"].astype(str).isin(test_groups)].tolist()
    train = frame.index[~frame["split_group"].astype(str).isin(test_groups)].tolist()
    return DataSplit("group-holdout", tuple(train), tuple(test))


def assert_no_window_overlap(frame: pd.DataFrame, split: DataSplit) -> None:
    train = frame.loc[list(split.train_index)]
    test = frame.loc[list(split.test_index)]
    for group in set(train["split_group"]).intersection(test["split_group"]):
        train_intervals = sorted(
            train.loc[train["split_group"] == group, ["feature_start", "cutoff_time"]].itertuples(
                index=False, name=None
            )
        )
        test_intervals = sorted(
            test.loc[test["split_group"] == group, ["feature_start", "cutoff_time"]].itertuples(
                index=False, name=None
            )
        )
        train_index = 0
        test_index = 0
        while train_index < len(train_intervals) and test_index < len(test_intervals):
            train_start, train_end = train_intervals[train_index]
            test_start, test_end = test_intervals[test_index]
            if train_end < test_start:
                train_index += 1
            elif test_end < train_start:
                test_index += 1
            else:
                raise ValueError(f"overlapping train/test windows for group {group}")


def _preprocessor(frame: pd.DataFrame, columns: Sequence[str]) -> ColumnTransformer:
    categorical = [column for column in columns if not pd.api.types.is_numeric_dtype(frame[column])]
    numerical = [column for column in columns if column not in categorical]
    numeric_pipeline = Pipeline(
        [("impute", SimpleImputer(strategy="median")), ("scale", StandardScaler())]
    )
    categorical_pipeline = Pipeline(
        [
            ("impute", SimpleImputer(strategy="most_frequent")),
            ("encode", OneHotEncoder(handle_unknown="ignore", sparse_output=False)),
        ]
    )
    return ColumnTransformer(
        [
            ("numeric", numeric_pipeline, numerical),
            ("categorical", categorical_pipeline, categorical),
        ],
        remainder="drop",
    )


def _coerce_feature_types(frame: pd.DataFrame, columns: Sequence[str]) -> pd.DataFrame:
    result = frame.loc[:, list(columns)].copy()
    for column in columns:
        if not pd.api.types.is_numeric_dtype(frame[column]):
            result[column] = result[column].map(
                lambda value: "__MISSING__" if pd.isna(value) else str(value)
            )
    return result


def _models(frame: pd.DataFrame, columns: Sequence[str], seed: int) -> dict[str, Pipeline]:
    return {
        "logistic-regression": Pipeline(
            [
                ("features", _preprocessor(frame, columns)),
                (
                    "model",
                    LogisticRegression(
                        max_iter=2000,
                        class_weight="balanced",
                        random_state=seed,
                    ),
                ),
            ]
        ),
        "hist-gradient-boosting": Pipeline(
            [
                ("features", _preprocessor(frame, columns)),
                (
                    "model",
                    HistGradientBoostingClassifier(
                        learning_rate=0.08,
                        max_iter=150,
                        max_leaf_nodes=15,
                        random_state=seed,
                    ),
                ),
            ]
        ),
    }


def _regression_models(
    frame: pd.DataFrame, columns: Sequence[str], seed: int
) -> dict[str, Pipeline]:
    return {
        "ridge-regression": Pipeline(
            [
                ("features", _preprocessor(frame, columns)),
                ("model", Ridge(alpha=1.0)),
            ]
        ),
        "hist-gradient-boosting-regression": Pipeline(
            [
                ("features", _preprocessor(frame, columns)),
                (
                    "model",
                    HistGradientBoostingRegressor(
                        learning_rate=0.08,
                        max_iter=150,
                        max_leaf_nodes=15,
                        random_state=seed,
                    ),
                ),
            ]
        ),
    }


def concordance_index(
    observed: Sequence[float], predicted: Sequence[float], *, max_pairs: int = 100_000
) -> float:
    """Return deterministic pairwise concordance for uncensored durations."""

    if len(observed) != len(predicted):
        raise ValueError("concordance inputs must have equal lengths")
    if len(set(observed)) < 2:
        return 1.0
    total_pairs = len(observed) * (len(observed) - 1) // 2
    if total_pairs <= max_pairs:
        pairs = [
            (left, right)
            for left in range(len(observed))
            for right in range(left + 1, len(observed))
            if observed[left] != observed[right]
        ]
    else:
        rng = np.random.default_rng(20260826)
        pairs = []
        while len(pairs) < max_pairs:
            left = int(rng.integers(0, len(observed)))
            right = int(rng.integers(0, len(observed)))
            if left != right and observed[left] != observed[right]:
                pairs.append((left, right))
    if not pairs:
        return 1.0
    concordant = 0.0
    for left, right in pairs:
        observed_order = observed[left] < observed[right]
        if predicted[left] == predicted[right]:
            concordant += 0.5
        elif (predicted[left] < predicted[right]) == observed_order:
            concordant += 1.0
    return concordant / len(pairs)


def evaluate_baselines(
    frame: pd.DataFrame,
    split: DataSplit,
    *,
    feature_sets: Sequence[str] = tuple(FEATURE_SETS),
    seed: int = 20260826,
    alert_budget_fraction: float = 0.1,
) -> list[BaselineScore]:
    assert_no_window_overlap(frame, split)
    train = frame.loc[list(split.train_index)]
    test = frame.loc[list(split.test_index)]
    y_train = train["label"].astype(int)
    y_test = test["label"].astype(int)
    if y_train.nunique() < 2:
        raise ValueError("baseline training split must contain both label classes")
    results = []
    for feature_set in feature_sets:
        columns = feature_columns(frame, feature_set)
        train_features = _coerce_feature_types(train, columns)
        test_features = _coerce_feature_types(test, columns)
        for model_name, pipeline in _models(frame, columns, seed).items():
            pipeline.fit(train_features, y_train)
            probabilities = pipeline.predict_proba(test_features)[:, 1]
            alert_count = max(1, round(len(test) * alert_budget_fraction))
            alerted = np.argsort(-probabilities)[:alert_count]
            true_alerts = int(y_test.iloc[alerted].sum())
            positives = int(y_test.sum())
            lead_times = test.iloc[alerted].loc[
                y_test.iloc[alerted].astype(bool), "time_to_event_minutes"
            ]
            false_alerts = alert_count - true_alerts
            exposure_weeks = max(
                1 / 7,
                (test["cutoff_time"].max() - test["cutoff_time"].min()).total_seconds()
                / (7 * 86400),
            )
            object_count = max(1, test["split_group"].nunique())
            results.append(
                BaselineScore(
                    model=model_name,
                    feature_set=feature_set,
                    split=split.name,
                    sample_count=len(test),
                    positive_count=positives,
                    auprc=(
                        float(average_precision_score(y_test, probabilities))
                        if y_test.nunique() == 2
                        else (1.0 if positives else 0.0)
                    ),
                    auroc=(
                        float(roc_auc_score(y_test, probabilities))
                        if y_test.nunique() == 2
                        else None
                    ),
                    brier=float(brier_score_loss(y_test, probabilities)),
                    ece=expected_calibration_error(y_test.tolist(), probabilities.tolist()),
                    recall_at_alert_budget=true_alerts / positives if positives else 1.0,
                    median_warning_lead_time_minutes=(
                        float(np.nanmedian(lead_times)) if len(lead_times) else None
                    ),
                    false_alerts_per_object_week=false_alerts / (object_count * exposure_weeks),
                )
            )
    return results


def evaluate_regression_baselines(
    frame: pd.DataFrame,
    split: DataSplit,
    *,
    feature_sets: Sequence[str] = tuple(FEATURE_SETS),
    seed: int = 20260826,
) -> list[RegressionScore]:
    """Fit reproducible uncensored time-to-event baselines for every ablation."""

    assert_no_window_overlap(frame, split)
    train = frame.loc[list(split.train_index)].dropna(subset=["time_to_event_minutes"])
    test = frame.loc[list(split.test_index)].dropna(subset=["time_to_event_minutes"])
    if len(train) < 2 or len(test) < 2:
        raise ValueError("regression split requires at least two uncensored train/test rows")
    y_train = train["time_to_event_minutes"].astype(float)
    y_test = test["time_to_event_minutes"].astype(float)
    result = []
    for feature_set in feature_sets:
        columns = feature_columns(frame, feature_set)
        train_features = _coerce_feature_types(train, columns)
        test_features = _coerce_feature_types(test, columns)
        for model_name, pipeline in _regression_models(frame, columns, seed).items():
            pipeline.fit(train_features, y_train)
            predictions = pipeline.predict(test_features)
            result.append(
                RegressionScore(
                    model=model_name,
                    feature_set=feature_set,
                    split=split.name,
                    sample_count=len(test),
                    mae_minutes=float(mean_absolute_error(y_test, predictions)),
                    concordance=concordance_index(y_test.tolist(), predictions.tolist()),
                )
            )
    return result
