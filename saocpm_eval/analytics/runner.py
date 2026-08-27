"""End-to-end quantitative analysis over one generated evaluation run."""

from __future__ import annotations

import csv
import json
import re
from collections import Counter, defaultdict
from dataclasses import asdict
from datetime import datetime, timedelta
from hashlib import sha256
from itertools import pairwise
from pathlib import Path
from typing import Any

import pandas as pd

from saocpm_eval.analytics.causal_checks import paired_effect
from saocpm_eval.analytics.conformance import Violation, score_violations
from saocpm_eval.analytics.conformance_rules import detect_conformance
from saocpm_eval.analytics.domain_metrics import (
    cohort_metric_rows,
    conformance_breakdown_rows,
    data_quality_metrics,
    noisy_operational_agreement_rows,
    operational_metric_rows,
    task_dataset_summary,
    transition_kpi_oracle_rows,
)
from saocpm_eval.analytics.episodes import (
    StateObservation,
    StateTransition,
    chattering_rate,
    episode_temporal_iou,
    extract_episodes,
    transitions_from_episodes,
)
from saocpm_eval.analytics.features import build_prediction_features, prepare_prediction_context
from saocpm_eval.analytics.flowvault_cli import CliResult, run_evaluation_bundle, run_json
from saocpm_eval.analytics.graph_metrics import EdgeObservation, graph_heterogeneity
from saocpm_eval.analytics.pattern_evaluation import PatternRecord, score_pattern
from saocpm_eval.analytics.prediction_baselines import (
    DataSplit,
    evaluate_baselines,
    evaluate_regression_baselines,
    grouped_split,
    temporal_split,
)
from saocpm_eval.analytics.som_diagnostics import (
    bootstrap_stability_rows,
    cell_explanation_rows,
    period_stability_rows,
    transfer_rows,
    warning_rows,
)
from saocpm_eval.analytics.som_evaluation import align_cells, nearby_transition_proportion
from saocpm_eval.analytics.state_agreement import event_state_agreement, match_transitions
from saocpm_eval.common.hashing import canonical_json_bytes
from saocpm_eval.common.ocel_builder import parse_timestamp
from saocpm_eval.common.truth_writer import RunWriter, repository_commit
from saocpm_eval.completion import (
    atomic_write_json,
    file_snapshot,
    implementation_fingerprint,
    read_json_record,
    size_inventory,
    size_inventory_matches,
    snapshot_matches,
)
from saocpm_eval.validation import preflight_run

STATE_SUFFIX = re.compile(r"\s+\[[^]]+\]$")
ANALYSIS_PROTOCOL_VERSION = "scalable-modular-v1"

ANALYSIS_BUDGETS: dict[str, dict[str, int | None]] = {
    "golden": {
        "som_epochs": 40,
        "som_training_windows": None,
        "prediction_samples_per_task": None,
        "diagnostic_windows": None,
        "bootstrap_repetitions": 20,
    },
    "smoke": {
        "som_epochs": 10,
        "som_training_windows": 2_000,
        "prediction_samples_per_task": 1_000,
        "diagnostic_windows": 2_000,
        "bootstrap_repetitions": 3,
    },
    "paper": {
        "som_epochs": 10,
        "som_training_windows": 2_000,
        "prediction_samples_per_task": 1_000,
        "diagnostic_windows": 2_000,
        "bootstrap_repetitions": 3,
    },
}


def _analysis_identity(manifest: dict[str, Any]) -> dict[str, str]:
    repository_root = Path(__file__).parents[2]
    implementation_sha256 = implementation_fingerprint(
        (
            Path(__file__).parent,
            repository_root / "saocpm_eval" / "validation.py",
            repository_root / "rust" / "ocel_core" / "src",
            repository_root / "rust" / "ocel_cli" / "src",
        )
    )
    identity = {
        "analysis_protocol_version": ANALYSIS_PROTOCOL_VERSION,
        "implementation_sha256": implementation_sha256,
        "config_sha256": str(manifest["config_sha256"]),
        "input_sha256": str(manifest["files"]["observed.ocel.json"]["sha256"]),
        "generator_commit": str(manifest["generator_commit"]),
        "scenario": str(manifest["scenario"]),
        "profile": str(manifest["profile"]),
    }
    identity["execution_fingerprint"] = sha256(canonical_json_bytes(identity)).hexdigest()
    return identity


def _completed_analysis_matches(run_dir: Path, identity: dict[str, str]) -> bool:
    prior = read_json_record(run_dir / "analytics" / "analysis_manifest.json")
    return bool(
        prior
        and prior.get("complete") is True
        and prior.get("execution_fingerprint") == identity["execution_fingerprint"]
        and isinstance(prior.get("input_snapshot"), dict)
        and snapshot_matches(run_dir, prior["input_snapshot"])
        and isinstance(prior.get("output_inventory"), dict)
        and size_inventory_matches(run_dir, prior["output_inventory"])
    )


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source))


def _attribute_map(item: dict[str, Any]) -> dict[str, Any]:
    return {attribute["name"]: attribute["value"] for attribute in item["attributes"]}


def _truth_observations(run_dir: Path) -> list[StateObservation]:
    return [
        StateObservation(
            row["leading_object_id"],
            row["event_id"],
            parse_timestamp(row["event_time"]),
            row["reference_state"],
        )
        for row in _read_csv(run_dir / "truth" / "state_at_event.csv")
    ]


def _truth_transitions(run_dir: Path) -> list[StateTransition]:
    return [
        StateTransition(
            row["leading_object_id"],
            row["from_state"],
            row["to_state"],
            parse_timestamp(row["event_time"]),
            row["event_id"],
            parse_timestamp(row["from_state_started_at"]),
        )
        for row in _read_csv(run_dir / "truth" / "transitions.csv")
    ]


def _predicted_states(document: dict[str, Any]) -> dict[str, str]:
    result = {}
    for event in document["events"]:
        state = _attribute_map(event).get("state")
        if state is not None:
            result[event["id"]] = str(state)
    return result


def _write_state_outputs(
    writer: RunWriter,
    truth: list[StateObservation],
    predicted: dict[str, str],
    horizon: datetime,
    scenario: str,
    run_id: str,
) -> None:
    agreement = event_state_agreement(truth, predicted)
    rows: list[dict[str, Any]] = [
        {
            "scope": "overall",
            "scenario": scenario,
            "run_id": run_id,
            "state": "ALL",
            "coverage": agreement.coverage,
            "accuracy": agreement.accuracy,
            "macro_f1": agreement.macro_f1,
            "weighted_f1": agreement.weighted_f1,
            "precision": "",
            "recall": "",
            "f1": "",
            "unknown_exposure": agreement.unknown_exposure,
            "unknown_rate": agreement.unknown_exposure,
        }
    ]
    for state, metrics in sorted(agreement.per_state.items()):
        rows.append(
            {
                "scope": "state",
                "scenario": scenario,
                "run_id": run_id,
                "state": state,
                "coverage": "",
                "accuracy": "",
                "macro_f1": "",
                "weighted_f1": "",
                "precision": metrics["precision"],
                "recall": metrics["recall"],
                "f1": metrics["f1"],
                "unknown_exposure": "",
            }
        )
    latest_by_object: dict[str, datetime] = {}
    for row in truth:
        latest_by_object[row.object_id] = max(
            row.time, latest_by_object.get(row.object_id, row.time)
        )
    horizons = {object_id: max(horizon, latest) for object_id, latest in latest_by_object.items()}
    truth_episodes = extract_episodes(truth, horizons)
    predicted_observations = [
        StateObservation(row.object_id, row.event_id, row.time, predicted[row.event_id])
        for row in truth
        if row.event_id in predicted
    ]
    predicted_episodes = extract_episodes(predicted_observations, horizons)
    episode_iou = episode_temporal_iou(truth_episodes, predicted_episodes)
    writer.write_csv(
        "analytics/episode_scores.csv",
        ("temporal_iou", "predicted_episode_count", "chattering_rate_under_60_seconds"),
        [
            {
                "temporal_iou": episode_iou,
                "predicted_episode_count": len(predicted_episodes),
                "chattering_rate_under_60_seconds": chattering_rate(predicted_episodes, 60),
            }
        ],
    )
    transition_agreement = match_transitions(
        _truth_transitions(writer.root),
        transitions_from_episodes(predicted_episodes),
        timedelta(minutes=60),
    )
    rows[0].update(
        {
            "transition_precision": transition_agreement.precision,
            "transition_recall": transition_agreement.recall,
            "transition_time_mae_minutes": (
                transition_agreement.median_absolute_error_seconds / 60
                if transition_agreement.median_absolute_error_seconds is not None
                else ""
            ),
            "episode_iou": episode_iou,
        }
    )
    state_fields = tuple(sorted({field for row in rows for field in row}))
    writer.write_csv(
        "analytics/state_agreement.csv",
        state_fields,
        [{field: row.get(field, "") for field in state_fields} for row in rows],
    )
    writer.write_csv(
        "analytics/transition_agreement.csv",
        (
            "precision",
            "recall",
            "matched",
            "median_absolute_error_seconds",
            "percentile_95_absolute_error_seconds",
        ),
        [
            {
                "precision": transition_agreement.precision,
                "recall": transition_agreement.recall,
                "matched": len(transition_agreement.matches),
                "median_absolute_error_seconds": transition_agreement.median_absolute_error_seconds,
                "percentile_95_absolute_error_seconds": (
                    transition_agreement.percentile_95_absolute_error_seconds
                ),
            }
        ],
    )


def _transition_kpi_rows(
    result: dict[str, Any], scenario: str, run_id: str, object_type: str
) -> list[dict[str, Any]]:
    return [
        {
            "scenario": scenario,
            "run_id": run_id,
            "object_type": object_type,
            "category": category,
            **row,
            "median_duration_minutes": (
                float(row["median_duration_ms"]) / 60_000
                if row.get("median_duration_ms") is not None
                else ""
            ),
            "avg_duration_minutes": (
                float(row["avg_duration_ms"]) / 60_000
                if row.get("avg_duration_ms") is not None
                else ""
            ),
        }
        for category in ("transitions", "recovery")
        for row in result[category]
    ]


def _edge_observations(document: dict[str, Any], leading_type: str) -> list[EdgeObservation]:
    objects = {item["id"]: item["type"] for item in document["objects"]}
    lifecycles: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for event in document["events"]:
        leading = [
            relation["objectId"]
            for relation in event["relationships"]
            if objects[relation["objectId"]] == leading_type
        ]
        if len(leading) == 1:
            lifecycles[leading[0]].append(event)
    rows = []
    for events in lifecycles.values():
        events.sort(key=lambda event: (parse_timestamp(event["time"]), event["id"]))
        for left, right in pairwise(events):
            state = _attribute_map(left).get("state")
            if state is not None:
                rows.append(
                    EdgeObservation(
                        left["type"],
                        right["type"],
                        str(state),
                        (
                            parse_timestamp(right["time"]) - parse_timestamp(left["time"])
                        ).total_seconds(),
                    )
                )
    return rows


def _write_graph_outputs(
    writer: RunWriter,
    document: dict[str, Any],
    ordinary_graph: dict[str, Any],
    state_graph: dict[str, Any],
    leading_type: str,
) -> None:
    score = graph_heterogeneity(_edge_observations(document, leading_type))
    writer.write_csv(
        "analytics/graph_metrics.csv",
        (
            "edge_state_entropy",
            "weighted_jensen_shannon_divergence",
            "conditional_mutual_information",
            "state_conditioned_edge_count",
        ),
        [
            {
                "edge_state_entropy": score.edge_state_entropy,
                "weighted_jensen_shannon_divergence": score.weighted_jensen_shannon_divergence,
                "conditional_mutual_information": score.conditional_mutual_information,
                "state_conditioned_edge_count": len(score.state_conditioned_frequency),
            }
        ],
    )
    edge_rows = [
        {
            "activity": key[0],
            "next_activity": key[1],
            "state": key[2],
            "frequency": count,
            "median_waiting_seconds": score.state_conditioned_median_waiting_seconds[key],
        }
        for key, count in sorted(score.state_conditioned_frequency.items())
    ]
    writer.write_csv("analytics/state_conditioned_edges.csv", tuple(edge_rows[0]), edge_rows)
    writer.write_json("analytics/ocdfg.json", ordinary_graph)
    writer.write_json("analytics/sa_ocdfg.json", state_graph)


def _normalize_pattern_sequence(sequence: list[str]) -> tuple[str, ...]:
    return tuple(
        STATE_SUFFIX.sub("", item)
        for item in sequence
        if not item.startswith(("START ", "END ", "CHANGE "))
    )


def _truth_patterns(run_dir: Path) -> list[PatternRecord]:
    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in _read_csv(run_dir / "truth" / "injected_pattern_instances.csv"):
        grouped[row["pattern_id"]].append(row)
    result = []
    for pattern_id, rows in sorted(grouped.items()):
        first = rows[0]
        result.append(
            PatternRecord(
                pattern_id,
                first["family"],
                tuple(json.loads(first["expected_sequence_json"])),
                frozenset(json.loads(first["expected_object_types_json"])),
                len(rows),
                frozenset(
                    (
                        row["leading_object_id"],
                        row["start_event_id"],
                        row["end_event_id"],
                    )
                    for row in rows
                ),
            )
        )
    return result


def _detected_patterns(result: dict[str, Any]) -> list[PatternRecord]:
    rows = []
    for family in ("intra", "inter"):
        for pattern in result[family]:
            rows.append(
                PatternRecord(
                    pattern["id"],
                    family,
                    _normalize_pattern_sequence(pattern["sequence"]),
                    frozenset(pattern["object_types"]),
                    int(pattern["support"]),
                    frozenset(
                        (
                            occurrence["object_id"],
                            occurrence["start_event"],
                            occurrence["end_event"],
                        )
                        for occurrence in pattern.get("occurrences", [])
                    ),
                )
            )
    return rows


def _write_pattern_outputs(
    writer: RunWriter, pattern_result: dict[str, Any], scenario: str, run_id: str
) -> None:
    detected = _detected_patterns(pattern_result)
    rows = [
        {"scenario": scenario, "run_id": run_id, **asdict(score_pattern(truth, detected))}
        for truth in _truth_patterns(writer.root)
    ]
    writer.write_csv("analytics/pattern_scores.csv", tuple(rows[0]), rows)
    writer.write_json("analytics/detected_patterns.json", pattern_result)


def _majority_window_labels(
    windows: list[dict[str, Any]], latent_rows: list[dict[str, str]]
) -> tuple[list[str], list[str], list[bool]]:
    by_object: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in latent_rows:
        by_object[row["leading_object_id"]].append(row)
    for rows in by_object.values():
        rows.sort(key=lambda row: (parse_timestamp(row["event_time"]), row["event_id"]))
    positions_by_object = {
        object_id: {row["event_id"]: index for index, row in enumerate(rows)}
        for object_id, rows in by_object.items()
    }
    endpoint = []
    majority = []
    contains_transition = []
    for window in windows:
        rows = by_object[window["object_id"]]
        positions = positions_by_object[window["object_id"]]
        start = positions[window["start_event"]]
        end = positions[window["end_event"]]
        labels = [row["primary_regime"] for row in rows[start : end + 1]]
        endpoint.append(labels[-1])
        majority.append(Counter(labels).most_common(1)[0][0])
        contains_transition.append(len(set(labels)) > 1)
    return endpoint, majority, contains_transition


def _mean_cell_run_length(object_ids: list[str], cells: list[str]) -> float:
    grouped: dict[str, list[str]] = defaultdict(list)
    for object_id, cell in zip(object_ids, cells, strict=True):
        grouped[object_id].append(cell)
    lengths = []
    for sequence in grouped.values():
        start = 0
        for index in range(1, len(sequence) + 1):
            if index == len(sequence) or sequence[index] != sequence[start]:
                lengths.append(index - start)
                start = index
    return sum(lengths) / len(lengths) if lengths else 0.0


def _evenly_spaced_indices(size: int, maximum: int | None) -> list[int]:
    if maximum is None or size <= maximum:
        return list(range(size))
    return [
        min(size - 1, ((2 * index + 1) * size) // (2 * maximum))
        for index in range(maximum)
    ]


def _detection_summary(assignments: dict[str, Any]) -> dict[str, Any]:
    fields = (
        "object_type",
        "window_size",
        "som_width",
        "som_height",
        "object_count",
        "feature_count",
        "training_window_count",
        "window_count",
        "pca",
        "som",
    )
    return {field: assignments[field] for field in fields}


def _write_som_outputs(
    writer: RunWriter,
    detection: dict[str, Any],
    assignments: dict[str, Any],
    document: dict[str, Any],
    scenario: str,
    run_id: str,
    *,
    diagnostic_window_cap: int | None,
    bootstrap_repetitions: int,
) -> dict[str, str]:
    windows = assignments["windows"]
    cells = [f"{row['cell_x']},{row['cell_y']}" for row in windows]
    coordinates = [(int(row["cell_x"]), int(row["cell_y"])) for row in windows]
    objects = [str(row["object_id"]) for row in windows]
    endpoint, majority, contains_transition = _majority_window_labels(
        windows,
        _read_csv(writer.root / "truth" / "latent_regime_at_event.csv"),
    )
    common = {
        "window_size": assignments["window_size"],
        "som_width": assignments["som_width"],
        "som_height": assignments["som_height"],
        "window_count": assignments["window_count"],
        "quantization_error": detection["som"]["quantization_error"],
        "mean_cell_run_length": _mean_cell_run_length(objects, cells),
        "nearby_transition_proportion": nearby_transition_proportion(objects, coordinates),
        "transition_window_fraction": sum(contains_transition) / len(contains_transition),
    }
    total_cells = int(assignments["som_width"]) * int(assignments["som_height"])
    rows = []
    mappings: dict[str, str] = {}
    for label_kind, labels in (("endpoint", endpoint), ("majority", majority)):
        score = align_cells(cells, labels, total_cell_count=total_cells)
        rows.append(
            {
                "scenario": scenario,
                "run_id": run_id,
                "label_kind": label_kind,
                "label_definition": label_kind,
                **common,
                "purity": score.purity,
                "adjusted_rand_index": score.adjusted_rand_index,
                "ari": score.adjusted_rand_index,
                "normalized_mutual_information": score.normalized_mutual_information,
                "nmi": score.normalized_mutual_information,
                "balanced_accuracy": score.balanced_accuracy,
                "mean_cell_entropy": score.mean_cell_entropy,
                "empty_cell_rate": score.empty_cell_rate,
            }
        )
        if label_kind == "endpoint":
            mappings = score.mapping
    writer.write_json("analytics/som_detection.json", detection)
    writer.write_json("analytics/som_assignments.json", assignments)
    total_cells = int(assignments["som_width"]) * int(assignments["som_height"])
    event_times = {str(event["id"]): parse_timestamp(event["time"]) for event in document["events"]}
    diagnostic_indices = _evenly_spaced_indices(len(windows), diagnostic_window_cap)
    diagnostic_windows = [windows[index] for index in diagnostic_indices]
    diagnostic_endpoint = [endpoint[index] for index in diagnostic_indices]
    diagnostic_majority = [majority[index] for index in diagnostic_indices]
    stability = bootstrap_stability_rows(
        diagnostic_windows,
        diagnostic_endpoint,
        diagnostic_majority,
        total_cells=total_cells,
        repetitions=bootstrap_repetitions,
    )
    stability.extend(
        period_stability_rows(
            diagnostic_windows,
            diagnostic_endpoint,
            diagnostic_majority,
            event_times,
            total_cells=total_cells,
        )
    )
    for row in stability:
        row["population_window_count"] = len(windows)
        row["diagnostic_window_count"] = len(diagnostic_windows)
        row["sampling_method"] = (
            "deterministic evenly spaced"
            if len(windows) > len(diagnostic_windows)
            else "census"
        )
    stability_fields = tuple(sorted({field for row in stability for field in row}))
    writer.write_csv(
        "analytics/som_stability.csv",
        stability_fields,
        [{field: row.get(field, "") for field in stability_fields} for row in stability],
    )
    transfer = transfer_rows(diagnostic_windows, diagnostic_endpoint, document, scenario)
    if transfer:
        writer.write_csv("analytics/som_transfer.csv", tuple(transfer[0]), transfer)
    warning = warning_rows(writer.root, windows, mappings, event_times, scenario)
    writer.write_csv("analytics/som_warning.csv", tuple(warning[0]), warning)
    warning_summary = warning[0]
    for row in rows:
        row["nearby_transition_rate"] = row["nearby_transition_proportion"]
        row["warning_precision"] = warning_summary["warning_precision"]
        row["warning_recall"] = warning_summary["warning_recall"]
        row["median_lead_time_minutes"] = warning_summary["median_lead_time_minutes"]
    writer.write_csv("analytics/som_scores.csv", tuple(rows[0]), rows)
    explanations = cell_explanation_rows(diagnostic_windows, document, event_times)
    writer.write_csv("analytics/som_cell_explanations.csv", tuple(explanations[0]), explanations)
    return mappings


def _augment_automatic_features(
    frame: pd.DataFrame,
    assignment_by_end: dict[str, str],
    mapping: dict[str, str],
) -> pd.DataFrame:
    frame = frame.copy()
    cells = [
        assignment_by_end.get(str(event_id), "unassigned")
        for event_id in frame["cutoff_event_id"]
    ]
    frame["automatic.cell"] = cells
    frame["automatic.regime"] = [mapping.get(cell, "unassigned") for cell in cells]
    return frame


def _write_prediction_outputs(
    writer: RunWriter,
    assignments: dict[str, Any],
    mapping: dict[str, str],
    document: dict[str, Any],
    scenario: str,
    run_id: str,
    *,
    max_samples_per_task: int | None,
) -> None:
    scores: list[dict[str, Any]] = []
    errors = []
    population: dict[str, Counter[str]] = defaultdict(Counter)
    with (writer.root / "truth" / "prediction_samples.csv").open(
        encoding="utf-8", newline=""
    ) as source:
        for row in csv.DictReader(source):
            population[row["label_name"]][row["label"].lower()] += 1
    tasks = sorted(population)
    context = prepare_prediction_context(writer.root, document)
    assignment_by_end = {
        str(row["end_event"]): f"{row['cell_x']},{row['cell_y']}"
        for row in assignments["windows"]
    }
    sampling_rows = []
    for task in tasks:
        task_frame = _augment_automatic_features(
            build_prediction_features(
                writer.root,
                task=task,
                context=context,
                max_samples=max_samples_per_task,
            ),
            assignment_by_end,
            mapping,
        ).reset_index(drop=True)
        sampling_rows.append(
            {
                "task": task,
                "population_count": sum(population[task].values()),
                "population_positive_count": population[task]["true"],
                "sample_count": len(task_frame),
                "sample_positive_count": int(task_frame["label"].sum()),
                "sampling_method": (
                    "deterministic label-stratified temporal spread"
                    if len(task_frame) < sum(population[task].values())
                    else "census"
                ),
            }
        )
        horizon = timedelta(minutes=float(task_frame["horizon_minutes"].iloc[0]))
        is_regression = str(task).lower().startswith("time to")
        for split_name in ("temporal-holdout", "group-holdout"):
            try:
                split: DataSplit = (
                    temporal_split(task_frame, prediction_horizon=horizon)
                    if split_name == "temporal-holdout"
                    else grouped_split(task_frame)
                )
                if is_regression:
                    scores.extend(
                        {
                            "scenario": scenario,
                            "run_id": run_id,
                            "task": task,
                            **asdict(score),
                        }
                        for score in evaluate_regression_baselines(task_frame, split)
                    )
                else:
                    scores.extend(
                        {
                            "scenario": scenario,
                            "run_id": run_id,
                            "task": task,
                            **asdict(score),
                            "median_lead_time_minutes": score.median_warning_lead_time_minutes,
                            "false_alerts_per_object_month": (
                                score.false_alerts_per_object_week * 52 / 12
                            ),
                        }
                        for score in evaluate_baselines(task_frame, split)
                    )
            except ValueError as exc:
                errors.append({"task": task, "split": split_name, "error": str(exc)})
    fields = (
        "scenario",
        "run_id",
        "task",
        "model",
        "feature_set",
        "split",
        "sample_count",
        "positive_count",
        "auprc",
        "auroc",
        "brier",
        "ece",
        "recall_at_alert_budget",
        "median_warning_lead_time_minutes",
        "median_lead_time_minutes",
        "false_alerts_per_object_week",
        "false_alerts_per_object_month",
        "mae_minutes",
        "concordance",
    )
    writer.write_csv(
        "analytics/prediction_scores.csv",
        fields,
        [{field: row.get(field, "") for field in fields} for row in scores],
    )
    writer.write_json("analytics/prediction_errors.json", errors)
    writer.write_csv(
        "analytics/prediction_sampling.csv", tuple(sampling_rows[0]), sampling_rows
    )


def _write_conformance_outputs(
    writer: RunWriter,
    document: dict[str, Any],
    scenario: str,
    profile: str,
) -> None:
    truth = [
        Violation(
            row["rule_id"],
            row["leading_object_id"],
            parse_timestamp(row["event_time"]),
            row["event_id"],
        )
        for row in _read_csv(writer.root / "truth" / "conformance_violations.csv")
    ]
    detected = detect_conformance(document, scenario, profile=profile)
    overall = score_violations(truth, detected, timedelta(minutes=1))
    rows = [{"rule_id": "ALL", **asdict(overall)}]
    rule_ids = sorted({row.rule_id for row in truth}.union(row.rule_id for row in detected))
    for rule_id in rule_ids:
        score = score_violations(
            [row for row in truth if row.rule_id == rule_id],
            [row for row in detected if row.rule_id == rule_id],
            timedelta(minutes=1),
        )
        rows.append({"rule_id": rule_id, **asdict(score)})
    writer.write_csv("analytics/conformance_scores.csv", tuple(rows[0]), rows)
    writer.write_csv(
        "analytics/detected_conformance_violations.csv",
        ("rule_id", "object_id", "time", "event_id"),
        [asdict(row) for row in detected],
    )
    breakdown = conformance_breakdown_rows([asdict(row) for row in detected], document, scenario)
    if breakdown:
        writer.write_csv("analytics/conformance_breakdown.csv", tuple(breakdown[0]), breakdown)


def _performance_row(operation: str, result: CliResult) -> dict[str, Any]:
    return {"operation": operation, **result.metrics}


def _write_causal_outputs(writer: RunWriter) -> None:
    truth = json.loads((writer.root / "truth" / "causal_truth.json").read_text(encoding="utf-8"))
    pairs = truth.get("paired_potential_outcomes", [])
    fields = (
        "outcome",
        "estimated_effect",
        "standard_error",
        "true_effect",
        "sign_matches_truth",
        "magnitude_error",
        "status",
    )
    rows = []
    for outcome, true_effect in truth.get("true_effects", {}).items():
        score = paired_effect(
            [float(row[f"{outcome}_treated"]) for row in pairs],
            [float(row[f"{outcome}_untreated"]) for row in pairs],
            float(true_effect),
        )
        rows.append(
            {
                "outcome": outcome,
                "estimated_effect": score.average_treatment_effect,
                "standard_error": score.standard_error,
                "true_effect": true_effect,
                "sign_matches_truth": score.sign_matches_truth,
                "magnitude_error": score.magnitude_error,
                "status": "paired randomized validation",
            }
        )
    if not rows:
        rows.append(
            {
                "outcome": "",
                "estimated_effect": "",
                "standard_error": "",
                "true_effect": "",
                "sign_matches_truth": "",
                "magnitude_error": "",
                "status": truth.get("design", "not enabled"),
            }
        )
    writer.write_csv("analytics/causal_scores.csv", fields, rows)


def analyze_run(run_dir: Path, *, force: bool = False) -> None:
    """Preflight a validated run and materialize all pre-specified analysis artifacts."""

    run_dir = run_dir.resolve()
    manifest = preflight_run(run_dir)
    identity = _analysis_identity(manifest)
    if not force and _completed_analysis_matches(run_dir, identity):
        return
    scenario = str(manifest["scenario"])
    profile = str(manifest["profile"])
    run_id = str(manifest["run_id"])
    leading_type = "ItemLocation" if scenario == "inventory" else "Machine"
    recovery = (
        [["Critical Understock", "Understock"], ["Understock", "Normal"]]
        if scenario == "inventory"
        else [["Down", "Recovery"], ["Recovery", "Running"]]
    )
    input_path = run_dir / "observed.ocel.json"
    behavior_path = run_dir / "observed.behavior.ocel.json"
    query_path = run_dir / "state_query.sql"
    writer = RunWriter(run_dir)
    (run_dir / "analytics").mkdir(exist_ok=True)
    application_commit = repository_commit(run_dir)
    performance = []
    budget = ANALYSIS_BUDGETS.get(profile, ANALYSIS_BUDGETS["paper"])
    writer.write_json(
        "analytics/scalability_budget.json",
        {
            "target_wall_time_seconds_per_dataset": 300,
            "observed_log_imports": 1,
            "behavior_log_imports": 1,
            "analysis_preflight": "manifest schema and file sizes",
            "som_training_calls": 1,
            "som_epochs": budget["som_epochs"],
            "som_training_windows": budget["som_training_windows"],
            "prediction_samples_per_task": budget["prediction_samples_per_task"],
            "diagnostic_windows": budget["diagnostic_windows"],
            "bootstrap_repetitions": budget["bootstrap_repetitions"],
            "prediction_sampling": "deterministic label-stratified temporal spread",
            "diagnostic_sampling": "deterministic evenly spaced",
        },
    )
    detection_request = {
        "object_type": leading_type,
        "window_size": 3 if scenario == "inventory" else 4,
        "som_width": 3,
        "som_height": 3,
        "epochs": int(budget["som_epochs"] or 10),
        "max_training_windows": budget["som_training_windows"],
    }
    bundle = run_evaluation_bundle(
        input_path=input_path,
        query_path=query_path,
        request={
            "object_type": leading_type,
            "transition_kpis": {
                "object_type": leading_type,
                "recovery_transitions": recovery,
            },
            "state_detection": detection_request,
        },
    )
    performance.append(_performance_row("single_import_evaluation_bundle", bundle))
    document = bundle.value["enriched"]
    kpis_value = bundle.value["transition_kpis"]
    ordinary_graph = bundle.value["ocdfg"]
    state_graph = bundle.value["sa_ocdfg"]
    assignments_value = bundle.value["state_detection_assignments"]
    branch_coverage = json.loads(
        (run_dir / "expected" / "branch_coverage.json").read_text(encoding="utf-8")
    )
    data_quality = data_quality_metrics(document, manifest, branch_coverage)
    writer.write_csv("analytics/data_quality.csv", tuple(data_quality[0]), data_quality)
    _write_state_outputs(
        writer,
        _truth_observations(run_dir),
        _predicted_states(document),
        parse_timestamp(manifest["end_time"]),
        scenario,
        run_id,
    )

    kpi_rows = _transition_kpi_rows(kpis_value, scenario, run_id, leading_type)
    writer.write_csv("analytics/transition_kpis.csv", tuple(kpi_rows[0]), kpi_rows)
    writer.write_json("analytics/transition_kpis.json", kpis_value)
    oracle_rows = transition_kpi_oracle_rows(
        _read_csv(run_dir / "truth" / "transitions.csv"), kpis_value
    )
    writer.write_csv("analytics/transition_kpi_oracle.csv", tuple(oracle_rows[0]), oracle_rows)
    operational = operational_metric_rows(run_dir, scenario)
    writer.write_csv("analytics/operational_metrics.csv", tuple(operational[0]), operational)
    cohorts = cohort_metric_rows(run_dir, document, scenario)
    writer.write_csv("analytics/cohort_metrics.csv", tuple(cohorts[0]), cohorts)
    if scenario == "manufacturing":
        noisy = noisy_operational_agreement_rows(run_dir)
        writer.write_csv("analytics/noisy_operational_agreement.csv", tuple(noisy[0]), noisy)
    tasks = task_dataset_summary(run_dir)
    writer.write_csv("analytics/analyst_tasks.csv", tuple(tasks[0]), tasks)

    _write_graph_outputs(writer, document, ordinary_graph, state_graph, leading_type)

    patterns = run_json(
        "state-patterns",
        input_path=behavior_path,
        query_path=query_path,
        request={
            "leading_object_type": leading_type,
            "family": "both",
            "pre_radius": 3,
            "post_radius": 3,
            "ignored_event_types": ["Inventory Snapshot", "Sensor Snapshot"],
            "min_support": 1,
            "include_occurrences": True,
        },
    )
    performance.append(_performance_row("state_patterns", patterns))
    _write_pattern_outputs(writer, patterns.value, scenario, run_id)

    detection = _detection_summary(assignments_value)
    mapping = _write_som_outputs(
        writer,
        detection,
        assignments_value,
        document,
        scenario,
        run_id,
        diagnostic_window_cap=budget["diagnostic_windows"],
        bootstrap_repetitions=int(budget["bootstrap_repetitions"] or 10),
    )
    _write_prediction_outputs(
        writer,
        assignments_value,
        mapping,
        document,
        scenario,
        run_id,
        max_samples_per_task=budget["prediction_samples_per_task"],
    )
    _write_conformance_outputs(writer, document, scenario, profile)
    _write_causal_outputs(writer)
    performance_rows = [
        {
            "scenario": scenario,
            "profile": profile,
            "run_id": run_id,
            "repetition": 1,
            "events": manifest["counts"]["events"],
            "objects": manifest["counts"]["objects"],
            "windows": assignments_value["window_count"],
            "features": detection["feature_count"],
            "patterns": len(patterns.value["intra"]) + len(patterns.value["inter"]),
            "commit": application_commit,
            **row,
        }
        for row in performance
    ]
    writer.write_csv("analytics/performance.csv", tuple(performance_rows[0]), performance_rows)
    output_paths = [
        path.relative_to(run_dir).as_posix()
        for path in (run_dir / "analytics").rglob("*")
        if path.is_file()
        and path.name not in {"analysis_manifest.json", "validation_manifest.json"}
    ]
    atomic_write_json(
        run_dir / "analytics" / "analysis_manifest.json",
        {
            **identity,
            "application_commit": application_commit,
            "validation_mode": "manifest-schema-and-file-size preflight",
            "run_id": run_id,
            "input_snapshot": file_snapshot(
                run_dir, ("manifest.json", *manifest["files"])
            ),
            "output_inventory": size_inventory(run_dir, output_paths),
            "complete": True,
        },
    )
