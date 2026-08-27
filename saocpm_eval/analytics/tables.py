"""Paper-ready CSV and LaTeX table generation from completed analysis artifacts."""

from __future__ import annotations

import csv
import json
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any

from saocpm_eval.common.hashing import canonical_json_bytes


def _read_csv(path: Path) -> list[dict[str, str]]:
    if not path.is_file():
        return []
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source))


def _latex(value: Any) -> str:
    text = str(value)
    replacements = {
        "\\": r"\textbackslash{}",
        "&": r"\&",
        "%": r"\%",
        "$": r"\$",
        "#": r"\#",
        "_": r"\_",
        "{": r"\{",
        "}": r"\}",
        "~": r"\textasciitilde{}",
        "^": r"\textasciicircum{}",
    }
    return "".join(replacements.get(character, character) for character in text)


def _format(value: Any) -> str:
    try:
        number = float(value)
    except (TypeError, ValueError):
        return str(value)
    return f"{number:.4f}".rstrip("0").rstrip(".")


def _write_table(root: Path, name: str, rows: list[dict[str, Any]]) -> None:
    if not rows:
        rows = [{"status": "not_available"}]
    fields = tuple(rows[0])
    csv_path = root / f"{name}.csv"
    with csv_path.open("w", encoding="utf-8", newline="") as target:
        writer = csv.DictWriter(
            target, fieldnames=fields, extrasaction="ignore", lineterminator="\n"
        )
        writer.writeheader()
        writer.writerows({field: row.get(field, "") for field in fields} for row in rows)
    alignment = "l" * len(fields)
    lines = [
        f"\\begin{{tabular}}{{{alignment}}}",
        "\\toprule",
        " & ".join(_latex(field.replace("_", " ").title()) for field in fields) + r" \\",
        "\\midrule",
    ]
    for row in rows:
        lines.append(" & ".join(_latex(_format(row.get(field, ""))) for field in fields) + r" \\")
    lines.extend(("\\bottomrule", "\\end{tabular}", ""))
    (root / f"{name}.tex").write_text("\n".join(lines), encoding="utf-8")


def _metric_rows(
    scenario: str, profile: str, source: dict[str, Any], names: Sequence[str]
) -> list[dict[str, Any]]:
    return [
        {
            "scenario": scenario,
            "profile": profile,
            "metric": name,
            "value": source.get(name, ""),
        }
        for name in names
    ]


def _run_tables(run_dirs: Iterable[Path]) -> dict[str, list[dict[str, Any]]]:
    tables: dict[str, list[dict[str, Any]]] = {
        "data_characteristics": [],
        "state_validity": [],
        "automatic_state_quality": [],
        "automatic_state_diagnostics": [],
        "analytical_tasks": [],
        "causal_validation": [],
        "prediction": [],
        "performance": [],
        "exploratory_causal": [],
    }
    for run_dir in run_dirs:
        manifest = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
        scenario = manifest["scenario"]
        profile = manifest["profile"]
        counts = manifest["counts"]
        tables["data_characteristics"].append(
            {
                "scenario": scenario,
                "profile": profile,
                "events": counts["events"],
                "objects": counts["objects"],
                "leading_objects": counts["leading_objects"],
                "e2o": counts["e2o"],
                "o2o": counts["o2o"],
                "event_types": counts.get("event_types", ""),
                "object_types": counts.get("object_types", ""),
            }
        )
        state = _read_csv(run_dir / "analytics" / "state_agreement.csv")
        episode = _read_csv(run_dir / "analytics" / "episode_scores.csv")
        transition = _read_csv(run_dir / "analytics" / "transition_agreement.csv")
        if state:
            tables["state_validity"].extend(
                _metric_rows(
                    scenario,
                    profile,
                    state[0],
                    ("coverage", "accuracy", "macro_f1", "weighted_f1", "unknown_exposure"),
                )
            )
        if episode:
            tables["state_validity"].extend(
                _metric_rows(
                    scenario,
                    profile,
                    episode[0],
                    ("temporal_iou", "chattering_rate_under_60_seconds"),
                )
            )
        if transition:
            tables["state_validity"].extend(
                _metric_rows(
                    scenario,
                    profile,
                    transition[0],
                    ("precision", "recall", "median_absolute_error_seconds"),
                )
            )
        for row in _read_csv(run_dir / "analytics" / "som_scores.csv"):
            tables["automatic_state_quality"].append(
                {
                    "scenario": scenario,
                    "profile": profile,
                    "label_kind": row["label_kind"],
                    "window_size": row["window_size"],
                    "grid": f"{row['som_width']}x{row['som_height']}",
                    "purity": row["purity"],
                    "ari": row["adjusted_rand_index"],
                    "nmi": row["normalized_mutual_information"],
                    "balanced_accuracy": row["balanced_accuracy"],
                    "cell_entropy": row["mean_cell_entropy"],
                    "empty_cell_rate": row["empty_cell_rate"],
                    "quantization_error": row["quantization_error"],
                    "nearby_transitions": row["nearby_transition_proportion"],
                }
            )
        for source_name, filename in (
            ("stability", "som_stability.csv"),
            ("transfer", "som_transfer.csv"),
            ("warning", "som_warning.csv"),
        ):
            for row in _read_csv(run_dir / "analytics" / filename):
                for metric, value in row.items():
                    if metric in {
                        "diagnostic",
                        "label_kind",
                        "period",
                        "dimension",
                        "held_out_value",
                    }:
                        continue
                    tables["automatic_state_diagnostics"].append(
                        {
                            "scenario": scenario,
                            "profile": profile,
                            "source": source_name,
                            "context": "/".join(
                                str(row.get(name, ""))
                                for name in (
                                    "diagnostic",
                                    "label_kind",
                                    "period",
                                    "dimension",
                                    "held_out_value",
                                )
                                if row.get(name, "")
                            ),
                            "metric": metric,
                            "value": value,
                        }
                    )
        graph = _read_csv(run_dir / "analytics" / "graph_metrics.csv")
        conformance = _read_csv(run_dir / "analytics" / "conformance_scores.csv")
        patterns = _read_csv(run_dir / "analytics" / "pattern_scores.csv")
        if graph:
            tables["analytical_tasks"].extend(
                _metric_rows(
                    scenario,
                    profile,
                    graph[0],
                    (
                        "edge_state_entropy",
                        "weighted_jensen_shannon_divergence",
                        "conditional_mutual_information",
                    ),
                )
            )
        if conformance:
            tables["analytical_tasks"].extend(
                _metric_rows(
                    scenario,
                    profile,
                    conformance[0],
                    ("precision", "recall", "median_timing_error_seconds"),
                )
            )
        if patterns:
            hits = sum(row["top_k_hit"] == "true" for row in patterns) / len(patterns)
            sequence = sum(float(row["sequence_similarity"]) for row in patterns) / len(patterns)
            tables["analytical_tasks"].extend(
                (
                    {
                        "scenario": scenario,
                        "profile": profile,
                        "metric": "pattern_top_k_rate",
                        "value": hits,
                    },
                    {
                        "scenario": scenario,
                        "profile": profile,
                        "metric": "pattern_sequence_similarity",
                        "value": sequence,
                    },
                )
            )
        for row in _read_csv(run_dir / "analytics" / "operational_metrics.csv"):
            tables["analytical_tasks"].append(
                {
                    "scenario": scenario,
                    "profile": profile,
                    "metric": "/".join(("operational", row["scope"], row["key"], row["metric"])),
                    "value": row["value"],
                }
            )
        analyst_cases = _read_csv(run_dir / "analytics" / "analyst_tasks.csv")
        tables["analytical_tasks"].append(
            {
                "scenario": scenario,
                "profile": profile,
                "metric": "analyst_task_case_count",
                "value": len(analyst_cases),
            }
        )
        for row in _read_csv(run_dir / "analytics" / "prediction_scores.csv"):
            tables["prediction"].append({"scenario": scenario, "profile": profile, **row})
        for row in _read_csv(run_dir / "analytics" / "performance.csv"):
            tables["performance"].append({"scenario": scenario, "profile": profile, **row})
        for row in _read_csv(run_dir / "analytics" / "causal_scores.csv"):
            tables["causal_validation"].append({"scenario": scenario, "profile": profile, **row})
        tables["exploratory_causal"].append(
            {
                "scenario": scenario,
                "profile": profile,
                "analysis": "workbench causal association",
                "status": "exploratory; not estimated by the pre-specified harness",
            }
        )
    return tables


def write_paper_tables(
    *,
    run_dirs: Sequence[Path],
    output_dir: Path,
    robustness_dirs: Sequence[Path] = (),
    benchmark_dirs: Sequence[Path] = (),
) -> None:
    """Aggregate evaluated runs into separate pre-specified and exploratory tables."""

    if not run_dirs:
        raise ValueError("paper tables require at least one analyzed run")
    tables = _run_tables(run_dirs)
    robustness = [
        row for directory in robustness_dirs for row in _read_csv(directory / "results.csv")
    ]
    benchmarks = [
        row for directory in benchmark_dirs for row in _read_csv(directory / "results.csv")
    ]
    tables["performance"].extend(benchmarks)
    pre_specified = output_dir / "pre_specified"
    exploratory = output_dir / "exploratory"
    pre_specified.mkdir(parents=True, exist_ok=True)
    exploratory.mkdir(parents=True, exist_ok=True)
    for name in (
        "data_characteristics",
        "state_validity",
        "automatic_state_quality",
        "automatic_state_diagnostics",
        "analytical_tasks",
        "causal_validation",
        "prediction",
        "performance",
    ):
        _write_table(pre_specified, name, tables[name])
    _write_table(pre_specified, "robustness", robustness)
    _write_table(exploratory, "causal_associations", tables["exploratory_causal"])
    (output_dir / "table_manifest.json").write_bytes(
        canonical_json_bytes(
            {
                "pre_specified": [
                    "data_characteristics",
                    "state_validity",
                    "automatic_state_quality",
                    "automatic_state_diagnostics",
                    "analytical_tasks",
                    "causal_validation",
                    "prediction",
                    "robustness",
                    "performance",
                ],
                "exploratory": ["causal_associations"],
                "run_count": len(run_dirs),
                "robustness_result_count": len(robustness),
                "benchmark_result_count": len(benchmarks),
            },
            pretty=True,
        )
    )
