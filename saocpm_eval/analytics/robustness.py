"""Pre-specified, resumable one-factor-at-a-time robustness evaluation."""

from __future__ import annotations

import csv
import json
import os
import shutil
import tempfile
from collections import Counter, defaultdict
from copy import deepcopy
from datetime import timedelta
from pathlib import Path
from typing import Any

import numpy as np
from scipy.stats import spearmanr

from saocpm_eval.analytics.episodes import (
    StateObservation,
    extract_episodes,
    transitions_from_episodes,
)
from saocpm_eval.analytics.features import build_prediction_features
from saocpm_eval.analytics.flowvault_cli import run_json
from saocpm_eval.analytics.prediction_baselines import evaluate_baselines, grouped_split
from saocpm_eval.analytics.runner import (
    _detected_patterns,
    _edge_observations,
    _read_csv,
)
from saocpm_eval.analytics.som_evaluation import align_cells
from saocpm_eval.analytics.state_agreement import event_state_agreement, match_transitions
from saocpm_eval.common.hashing import canonical_json_bytes, sha256_file
from saocpm_eval.common.ocel_builder import OcelBuilder, format_timestamp, parse_timestamp
from saocpm_eval.common.perturbations import (
    delete_context_relationships,
    delete_events,
    jitter_event_timestamps,
    mask_event_attributes,
)
from saocpm_eval.common.truth_writer import repository_commit
from saocpm_eval.completion import implementation_fingerprint
from saocpm_eval.config import ConfigEnvelope
from saocpm_eval.generation import generate_run
from saocpm_eval.validation import preflight_run

ROBUSTNESS_PROTOCOL_VERSION = "scalable-modular-v1"


def _specification(scenario: str) -> list[dict[str, Any]]:
    common: list[dict[str, Any]] = [
        dict(kind="relationship_deletion", value=0.05),
        dict(kind="pattern_radius", value=1),
    ]
    if scenario == "inventory":
        specific: list[dict[str, Any]] = [
            dict(kind="threshold_scale", value=0.9),
            dict(kind="threshold_scale", value=1.1),
            dict(kind="policy_version_lag_days", value=3),
            dict(kind="event_deletion", value=0.05),
            dict(kind="attribute_missingness_mcar", value=0.05),
            dict(kind="timestamp_jitter_minutes", value=5),
            dict(kind="stock_adjustment_omission", value=0.25),
            dict(kind="window_size", value=5),
            dict(kind="som_grid", value="5x5"),
        ]
    else:
        specific = [
            dict(kind="sensor_noise_multiplier", value=2.0),
            dict(kind="telemetry_cadence_minutes", value=60),
            dict(kind="telemetry_block_missingness_hours", value=4),
            dict(kind="alarm_recording_delay_minutes", value=5),
            dict(kind="process_event_deletion", value=0.05),
            dict(kind="timestamp_jitter_minutes", value=1),
            dict(kind="degraded_threshold_scale", value=0.95),
            dict(kind="window_size", value=8),
            dict(kind="som_grid", value="5x5"),
        ]
    result: list[dict[str, Any]] = []
    for item in [*specific, *common]:
        value_text = str(item["value"]).replace(".", "p").replace(" ", "-")
        result.append({"id": f"{item['kind']}--{value_text}", **item})
    return result


def _attributes(event: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {attribute["name"]: attribute for attribute in event["attributes"]}


def _scale_attributes(document: dict[str, Any], names: frozenset[str], factor: float) -> None:
    for collection in (document["events"], document["objects"]):
        for item in collection:
            for attribute in item.get("attributes", []):
                if attribute["name"] in names and isinstance(attribute["value"], (int, float)):
                    attribute["value"] = float(attribute["value"]) * factor


def _mark_block_incomplete(
    document: dict[str, Any], probability: float, rng: np.random.Generator, event_type: str | None
) -> None:
    candidates = [
        event for event in document["events"] if event_type is None or event["type"] == event_type
    ]
    if not candidates:
        return
    count = max(1, round(len(candidates) * probability))
    start = int(rng.integers(0, max(1, len(candidates) - count + 1)))
    for event in candidates[start : start + count]:
        attributes = _attributes(event)
        if "data_complete" in attributes:
            attributes["data_complete"]["value"] = False


def _delete_for_cadence(document: dict[str, Any], minutes: int) -> None:
    by_machine: dict[str, list[dict[str, Any]]] = defaultdict(list)
    object_types = {item["id"]: item["type"] for item in document["objects"]}
    for event in document["events"]:
        if event["type"] != "Sensor Snapshot":
            continue
        machine = next(
            relation["objectId"]
            for relation in event["relationships"]
            if object_types[relation["objectId"]] == "Machine"
        )
        by_machine[machine].append(event)
    retained = {event["id"] for event in document["events"] if event["type"] != "Sensor Snapshot"}
    for events in by_machine.values():
        events.sort(key=lambda event: parse_timestamp(event["time"]))
        last = None
        for event in events:
            time = parse_timestamp(event["time"])
            if last is None or time - last >= timedelta(minutes=minutes):
                retained.add(event["id"])
                last = time
    document["events"] = [event for event in document["events"] if event["id"] in retained]


def _shift_event_types(
    document: dict[str, Any],
    event_types: frozenset[str],
    delay: timedelta,
    leading_object_type: str,
) -> None:
    for event in document["events"]:
        if event["type"] in event_types:
            event["time"] = format_timestamp(parse_timestamp(event["time"]) + delay)
    document["events"].sort(key=lambda event: (parse_timestamp(event["time"]), event["id"]))
    object_types = {item["id"]: item["type"] for item in document["objects"]}
    previous_by_leading: dict[str, Any] = {}
    for event in document["events"]:
        leading_ids = [
            relationship["objectId"]
            for relationship in event.get("relationships", [])
            if object_types[relationship["objectId"]] == leading_object_type
        ]
        time = parse_timestamp(event["time"])
        lower_bound = max(
            (
                previous_by_leading[object_id] + timedelta(seconds=1)
                for object_id in leading_ids
                if object_id in previous_by_leading
            ),
            default=time,
        )
        if time < lower_bound:
            time = lower_bound
            event["time"] = format_timestamp(time)
        for object_id in leading_ids:
            previous_by_leading[object_id] = time
    document["events"].sort(key=lambda event: (parse_timestamp(event["time"]), event["id"]))


def _perturb(
    source: dict[str, Any],
    spec: dict[str, Any],
    scenario: str,
    rng: np.random.Generator,
) -> tuple[dict[str, Any], dict[str, Any]]:
    document = deepcopy(source)
    analysis: dict[str, Any] = {}
    kind = spec["kind"]
    value = spec["value"]
    leading_type = "ItemLocation" if scenario == "inventory" else "Machine"
    if kind == "threshold_scale":
        _scale_attributes(document, frozenset({"lower_threshold", "upper_threshold"}), float(value))
    elif kind == "policy_version_lag_days":
        _shift_event_types(
            document,
            frozenset({"Policy Threshold Updated"}),
            timedelta(days=float(value)),
            leading_type,
        )
    elif kind in {"event_deletion", "process_event_deletion"}:
        boundaries = {
            "Initialize Inventory",
            "Simulation End Snapshot",
            "Initialize Machine",
        }
        eligible = frozenset(
            event["type"] for event in document["events"] if event["type"] not in boundaries
        )
        document = delete_events(
            document,
            float(value),
            rng,
            eligible_event_types=eligible,
            leading_object_type=leading_type,
        )
    elif kind == "relationship_deletion":
        document = delete_context_relationships(
            document, float(value), rng, leading_object_type=leading_type
        )
    elif kind == "attribute_missingness_mcar":
        document = mask_event_attributes(document, float(value), rng)
    elif kind == "attribute_missingness_block":
        _mark_block_incomplete(document, float(value), rng, None)
    elif kind == "timestamp_jitter_minutes":
        document = jitter_event_timestamps(
            document, float(value), rng, leading_object_type=leading_type
        )
    elif kind == "stock_adjustment_omission":
        document = delete_events(
            document,
            float(value),
            rng,
            eligible_event_types=frozenset({"Inventory Adjustment"}),
            leading_object_type=leading_type,
        )
    elif kind == "sensor_noise_multiplier":
        _scale_attributes(
            document,
            frozenset({"health_index", "vibration_rms", "temperature_c", "power_kw"}),
            float(value),
        )
    elif kind == "telemetry_cadence_minutes":
        _delete_for_cadence(document, int(value))
    elif kind == "telemetry_block_missingness_hours":
        probability = min(1.0, float(value) / 24.0)
        if probability:
            _mark_block_incomplete(document, probability, rng, "Sensor Snapshot")
    elif kind == "alarm_recording_delay_minutes":
        _shift_event_types(
            document,
            frozenset({"Warning Alarm Raised", "Critical Alarm Raised"}),
            timedelta(minutes=float(value)),
            leading_type,
        )
    elif kind == "degraded_threshold_scale":
        analysis["degraded_threshold_scale"] = float(value)
    elif kind == "window_size":
        analysis["window_size"] = int(value)
    elif kind == "som_grid":
        width, height = (int(part) for part in str(value).split("x"))
        analysis.update({"som_width": width, "som_height": height})
    elif kind == "pattern_radius":
        radius = None if value == "full" else int(value)
        analysis.update({"pre_radius": radius, "post_radius": radius})
    else:
        raise ValueError(f"unsupported robustness perturbation {kind!r}")
    OcelBuilder.from_dict(document, leading_object_type=leading_type)
    return document, analysis


def _query_text(clean: Path, analysis: dict[str, Any]) -> str:
    text = (clean / "state_query.sql").read_text(encoding="utf-8")
    factor = analysis.get("degraded_threshold_scale")
    if factor is not None:
        text = text.replace("0.65", f"{0.65 * factor:.6f}")
        text = text.replace("0.75", f"{0.75 * factor:.6f}")
    return text


def _write_temp_input(document: dict[str, Any], query_text: str) -> tuple[Path, Path, Path]:
    root = Path(tempfile.mkdtemp(prefix="flowvault-robustness-"))
    input_path = root / "observed.ocel.json"
    input_path.write_bytes(canonical_json_bytes(document))
    query_path = root / "state_query.sql"
    query_path.write_text(query_text, encoding="utf-8")
    return root, input_path, query_path


def _state_truth(clean: Path) -> list[StateObservation]:
    return [
        StateObservation(
            row["leading_object_id"],
            row["event_id"],
            parse_timestamp(row["event_time"]),
            row["reference_state"],
        )
        for row in _read_csv(clean / "truth" / "state_at_event.csv")
    ]


def _prediction_auprc(clean: Path, input_path: Path) -> float | None:
    clean = clean.resolve()
    input_path = input_path.resolve()
    with tempfile.TemporaryDirectory(prefix="flowvault-prediction-") as temporary:
        root = Path(temporary)
        (root / "truth").symlink_to(clean / "truth", target_is_directory=True)
        (root / "manifest.json").symlink_to(clean / "manifest.json")
        (root / "observed.ocel.json").symlink_to(input_path)
        try:
            with (root / "truth" / "prediction_samples.csv").open(
                encoding="utf-8", newline=""
            ) as source:
                task = next(
                    row["label_name"]
                    for row in csv.DictReader(source)
                    if not row["label_name"].lower().startswith("time to")
                )
            frame = build_prediction_features(root, task=task, max_samples=12_000)
            split = grouped_split(frame)
            scores = evaluate_baselines(frame, split, feature_sets=("full",))
        except ValueError:
            return None
    logistic = next(score for score in scores if score.model == "logistic-regression")
    return logistic.auprc


def _rank_correlation(
    baseline: Counter[tuple[str, str, str]], current: Counter[tuple[str, str, str]]
) -> float:
    keys = set(key for key, _ in baseline.most_common(20)).union(
        key for key, _ in current.most_common(20)
    )
    if len(keys) < 2:
        return 1.0
    ordered = sorted(keys)
    value = spearmanr(
        [baseline.get(key, 0) for key in ordered],
        [current.get(key, 0) for key in ordered],
    ).statistic
    return float(value) if np.isfinite(value) else 1.0


def _evaluate(
    clean: Path,
    document: dict[str, Any],
    analysis: dict[str, Any],
    scenario: str,
) -> dict[str, Any]:
    leading_type = "ItemLocation" if scenario == "inventory" else "Machine"
    root, input_path, query_path = _write_temp_input(document, _query_text(clean, analysis))
    try:
        enriched = run_json("export", input_path=input_path, query_path=query_path)
        enriched_document = enriched.value
        predicted = {
            event["id"]: str(_attributes(event)["state"]["value"])
            for event in enriched_document["events"]
            if "state" in _attributes(event)
        }
        event_times = {
            event["id"]: parse_timestamp(event["time"]) for event in enriched_document["events"]
        }
        truth = _state_truth(clean)
        agreement = event_state_agreement(truth, predicted)
        predicted_observations = [
            StateObservation(
                row.object_id,
                row.event_id,
                event_times[row.event_id],
                predicted[row.event_id],
            )
            for row in truth
            if row.event_id in predicted
        ]
        predicted_transitions = transitions_from_episodes(extract_episodes(predicted_observations))
        truth_transitions = transitions_from_episodes(extract_episodes(truth))
        transition_score = match_transitions(
            truth_transitions, predicted_transitions, timedelta(hours=24)
        )
        truth_occupancy = Counter(row.state for row in truth)
        predicted_occupancy = Counter(predicted.values())
        states = set(truth_occupancy).union(predicted_occupancy)
        occupancy_change = 0.5 * sum(
            abs(
                truth_occupancy[state] / len(truth)
                - predicted_occupancy[state] / max(1, len(predicted))
            )
            for state in states
        )
        edges = Counter(
            (row.activity, row.next_activity, row.state)
            for row in _edge_observations(enriched_document, leading_type)
        )

        radius = analysis.get("pre_radius", 3)
        patterns_result = run_json(
            "state-patterns",
            input_path=input_path,
            query_path=query_path,
            request={
                "leading_object_type": leading_type,
                "family": "both",
                "pre_radius": radius,
                "post_radius": analysis.get("post_radius", 3),
                "ignored_event_types": ["Inventory Snapshot", "Sensor Snapshot"],
                "min_support": 1,
                "include_occurrences": False,
            },
        )
        patterns = _detected_patterns(patterns_result.value)
        signatures = Counter(
            {
                json.dumps([row.family, row.sequence], separators=(",", ":")): row.support
                for row in patterns
            }
        )

        request = {
            "object_type": leading_type,
            "window_size": analysis.get("window_size", 3 if scenario == "inventory" else 4),
            "som_width": analysis.get("som_width", 3),
            "som_height": analysis.get("som_height", 3),
            "epochs": 10,
            "max_training_windows": 50_000,
        }
        assignments = run_json(
            "state-detection-assignments", input_path=input_path, request=request
        )
        windows = assignments.value["windows"]
        latent_by_event = {
            row["event_id"]: row["primary_regime"]
            for row in _read_csv(clean / "truth" / "latent_regime_at_event.csv")
        }
        endpoint = [latent_by_event[row["end_event"]] for row in windows]
        cells = [f"{row['cell_x']},{row['cell_y']}" for row in windows]
        som = align_cells(
            cells,
            endpoint,
            total_cell_count=int(request["som_width"]) * int(request["som_height"]),
        )
        return {
            "state_coverage": agreement.coverage,
            "state_macro_f1": agreement.macro_f1,
            "transition_recall": transition_score.recall,
            "transition_median_error_seconds": transition_score.median_absolute_error_seconds,
            "state_occupancy_total_variation": occupancy_change,
            "edges": edges,
            "patterns": signatures,
            "som_nmi": som.normalized_mutual_information,
            "som_quantization_error": assignments.value["som"]["quantization_error"],
            "prediction_auprc": _prediction_auprc(clean, input_path),
            "runtime_ms": sum(
                row.metrics["wall_time_ms"]
                for row in (enriched, patterns_result, assignments)
            ),
            "peak_rss_bytes": max(
                int(row.metrics["peak_rss_bytes"] or 0)
                for row in (enriched, patterns_result, assignments)
            ),
        }
    finally:
        shutil.rmtree(root)


def _jaccard(left: set[str], right: set[str]) -> float:
    union = left | right
    return len(left & right) / len(union) if union else 1.0


def _compare(
    spec: dict[str, Any], baseline: dict[str, Any], current: dict[str, Any]
) -> dict[str, Any]:
    baseline_patterns: Counter[str] = baseline["patterns"]
    current_patterns: Counter[str] = current["patterns"]
    top_baseline = {key for key, _ in baseline_patterns.most_common(10)}
    top_current = {key for key, _ in current_patterns.most_common(10)}
    common = set(baseline_patterns).intersection(current_patterns)
    support_change = (
        sum(
            abs(current_patterns[key] - baseline_patterns[key]) / baseline_patterns[key]
            for key in common
        )
        / len(common)
        if common
        else 1.0
    )
    prediction_change = (
        current["prediction_auprc"] - baseline["prediction_auprc"]
        if current["prediction_auprc"] is not None and baseline["prediction_auprc"] is not None
        else None
    )
    row = {
        "perturbation_id": spec["id"],
        "kind": spec["kind"],
        "value": spec["value"],
        "state_coverage": current["state_coverage"],
        "state_coverage_change": current["state_coverage"] - baseline["state_coverage"],
        "state_macro_f1": current["state_macro_f1"],
        "state_macro_f1_change": current["state_macro_f1"] - baseline["state_macro_f1"],
        "transition_time_drift_seconds": current["transition_median_error_seconds"],
        "state_occupancy_change": current["state_occupancy_total_variation"],
        "edge_rank_correlation": _rank_correlation(baseline["edges"], current["edges"]),
        "top_k_pattern_jaccard": _jaccard(top_baseline, top_current),
        "mean_pattern_support_change": support_change,
        "som_nmi": current["som_nmi"],
        "som_nmi_change": current["som_nmi"] - baseline["som_nmi"],
        "prediction_auprc": current["prediction_auprc"],
        "prediction_auprc_change": prediction_change,
        "runtime_ms": current["runtime_ms"],
        "runtime_change": current["runtime_ms"] / baseline["runtime_ms"] - 1,
        "peak_rss_bytes": current["peak_rss_bytes"],
        "peak_memory_change": current["peak_rss_bytes"] / baseline["peak_rss_bytes"] - 1,
    }
    severity = max(
        abs(float(row["state_macro_f1_change"])),
        float(row["state_occupancy_change"]),
        abs(float(row["som_nmi_change"])),
        1 - float(row["top_k_pattern_jaccard"]),
    )
    row["stability_class"] = (
        "stable"
        if severity <= 0.05
        else "conditionally stable"
        if severity <= 0.15
        else "sensitive"
    )
    return row


def _load_rows(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    rows = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise ValueError(f"corrupt robustness JSONL row {number}: {exc}") from exc
    return rows


def _serialize_baseline(value: dict[str, Any]) -> dict[str, Any]:
    return {
        **{key: item for key, item in value.items() if key not in {"edges", "patterns"}},
        "edges": [[*key, count] for key, count in sorted(value["edges"].items())],
        "patterns": dict(value["patterns"]),
    }


def _deserialize_baseline(value: dict[str, Any]) -> dict[str, Any]:
    return {
        **{key: item for key, item in value.items() if key not in {"edges", "patterns"}},
        "edges": Counter(
            {(str(row[0]), str(row[1]), str(row[2])): int(row[3]) for row in value["edges"]}
        ),
        "patterns": Counter({str(key): int(count) for key, count in value["patterns"].items()}),
    }


def _append(path: Path, row: dict[str, Any]) -> None:
    with path.open("ab") as target:
        target.write(canonical_json_bytes(row))
        target.flush()
        os.fsync(target.fileno())


def _write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    fields = tuple(rows[0])
    with path.open("w", encoding="utf-8", newline="") as target:
        writer = csv.DictWriter(
            target, fieldnames=fields, extrasaction="ignore", lineterminator="\n"
        )
        writer.writeheader()
        writer.writerows({field: row.get(field, "") for field in fields} for row in rows)


def _relative_change(base: float, perturbed: float) -> float | str:
    if base == 0:
        return 0.0 if perturbed == 0 else ""
    return (perturbed - base) / abs(base)


def _contract_rows(
    rows: list[dict[str, Any]], baseline: dict[str, Any], base_run_id: str
) -> list[dict[str, Any]]:
    """Normalize the wide robustness ledger to the public long-form contract."""

    result: list[dict[str, Any]] = []
    metrics = (
        ("state_coverage", "state_coverage", baseline["state_coverage"]),
        ("state_macro_f1", "state_macro_f1", baseline["state_macro_f1"]),
        (
            "transition_time_drift_seconds",
            "transition_time_drift_seconds",
            baseline["transition_median_error_seconds"],
        ),
        (
            "state_occupancy_total_variation",
            "state_occupancy_change",
            baseline["state_occupancy_total_variation"],
        ),
        ("edge_rank_correlation", "edge_rank_correlation", 1.0),
        ("top_k_pattern_jaccard", "top_k_pattern_jaccard", 1.0),
        ("mean_pattern_support_change", "mean_pattern_support_change", 0.0),
        ("som_nmi", "som_nmi", baseline["som_nmi"]),
        ("prediction_auprc", "prediction_auprc", baseline["prediction_auprc"]),
        ("runtime_ms", "runtime_ms", baseline["runtime_ms"]),
        ("peak_rss_bytes", "peak_rss_bytes", baseline["peak_rss_bytes"]),
    )
    for row in rows:
        for metric, column, base_value in metrics:
            perturbed_value = row.get(column)
            if base_value is None or perturbed_value is None or perturbed_value == "":
                continue
            base = float(base_value)
            perturbed = float(perturbed_value)
            result.append(
                {
                    "scenario": row["scenario"],
                    "base_run_id": base_run_id,
                    "perturbation_id": row["perturbation_id"],
                    "metric": metric,
                    "base_value": base,
                    "perturbed_value": perturbed,
                    "absolute_change": perturbed - base,
                    "relative_change": _relative_change(base, perturbed),
                    "stability_class": row["stability_class"],
                }
            )
    return result


def _checkpoint_run(output_dir: Path, spec: dict[str, Any], row: dict[str, Any]) -> None:
    runs = output_dir / "runs"
    target = runs / spec["id"]
    if target.exists():
        if (target / "result.json").is_file():
            return
        interrupted = output_dir / "interrupted"
        interrupted.mkdir(exist_ok=True)
        index = 1
        destination = interrupted / f"{spec['id']}--{index}"
        while destination.exists():
            index += 1
            destination = interrupted / f"{spec['id']}--{index}"
        target.rename(destination)
    staging = Path(tempfile.mkdtemp(prefix=f".{spec['id']}--", dir=runs))
    (staging / "perturbation.json").write_bytes(canonical_json_bytes(spec, pretty=True))
    (staging / "result.json").write_bytes(canonical_json_bytes(row, pretty=True))
    staging.rename(target)


def run_robustness(*, config: ConfigEnvelope, config_path: Path, output_dir: Path) -> None:
    """Execute the registered perturbation matrix and append one immutable result per run."""

    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "runs").mkdir(exist_ok=True)
    config_sha256 = sha256_file(config_path)
    repository_root = Path(__file__).parents[2]
    current_metadata = {
        "protocol_version": ROBUSTNESS_PROTOCOL_VERSION,
        "scenario": config.scenario,
        "profile": config.profile,
        "seed": config.seed,
        "config_sha256": config_sha256,
        "implementation_sha256": implementation_fingerprint(
            (
                Path(__file__),
                Path(__file__).parent / "features.py",
                Path(__file__).parent / "prediction_baselines.py",
                repository_root / "rust" / "ocel_core" / "src",
            )
        ),
    }
    metadata_path = output_dir / "metadata.json"
    if metadata_path.is_file():
        prior_metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
        if prior_metadata != current_metadata:
            raise ValueError("robustness output belongs to a different experiment definition")
    else:
        metadata_path.write_bytes(canonical_json_bytes(current_metadata, pretty=True))
    results_path = output_dir / "results.jsonl"
    rows = _load_rows(results_path)
    completed = {row["perturbation_id"] for row in rows}
    specification = _specification(config.scenario)
    expected = {spec["id"] for spec in specification}
    if expected.issubset(completed) and all(
        (output_dir / name).is_file() for name in ("results.csv", "robustness_scores.csv")
    ):
        return
    clean = output_dir / "clean"
    if (clean / "manifest.json").is_file():
        preflight_run(clean)
    elif clean.exists() and any(clean.iterdir()):
        raise ValueError("clean robustness run is incomplete")
    else:
        generate_run(config=config, config_path=config_path, output_dir=clean)
        preflight_run(clean)
    manifest_path = clean / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    clean_document = json.loads((clean / "observed.ocel.json").read_text(encoding="utf-8"))
    baseline_path = output_dir / "baseline.json"
    if baseline_path.is_file():
        baseline = _deserialize_baseline(json.loads(baseline_path.read_text(encoding="utf-8")))
    else:
        baseline = _evaluate(clean, clean_document, {}, config.scenario)
        baseline_path.write_bytes(canonical_json_bytes(_serialize_baseline(baseline), pretty=True))
    for spec in specification:
        if spec["id"] in completed:
            continue
        checkpoint = output_dir / "runs" / spec["id"] / "result.json"
        if checkpoint.is_file():
            row = json.loads(checkpoint.read_text(encoding="utf-8"))
        else:
            digest = int(config_sha256[:8], 16)
            spec_digest = sum(spec["id"].encode("utf-8"))
            rng = np.random.default_rng(np.random.SeedSequence([config.seed, digest, spec_digest]))
            document, analysis = _perturb(clean_document, spec, config.scenario, rng)
            current = _evaluate(clean, document, analysis, config.scenario)
            row = {
                **_compare(spec, baseline, current),
                "scenario": config.scenario,
                "profile": config.profile,
                "seed": config.seed,
                "application_commit": repository_commit(Path(__file__).parents[2]),
                "generator_commit": manifest["generator_commit"],
                "config_sha256": config_sha256,
                "clean_manifest_sha256": sha256_file(manifest_path),
            }
            _checkpoint_run(output_dir, spec, row)
        _append(results_path, row)
        rows.append(row)
        completed.add(spec["id"])
        _write_csv(output_dir / "results.csv", rows)
    if rows:
        _write_csv(output_dir / "results.csv", rows)
        _write_csv(
            output_dir / "robustness_scores.csv",
            _contract_rows(rows, baseline, str(manifest["run_id"])),
        )
