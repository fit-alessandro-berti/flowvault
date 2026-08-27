"""Command-line interface for generation and quantitative evaluation."""

from __future__ import annotations

import argparse
import logging
from collections.abc import Callable, Sequence
from pathlib import Path

from saocpm_eval import __version__
from saocpm_eval.config import Scenario, load_config, load_scale_matrix
from saocpm_eval.logging import configure_logging

LOGGER = logging.getLogger("saocpm_eval")
Command = Callable[[argparse.Namespace], int]


def _path(value: str) -> Path:
    return Path(value).expanduser().resolve()


def _generate(args: argparse.Namespace) -> int:
    scenario: Scenario = args.scenario
    config = load_config(args.config, scenario)
    from saocpm_eval.generation import generate_run

    generate_run(config=config, config_path=args.config, output_dir=args.out)
    return 0


def _validate(args: argparse.Namespace) -> int:
    from saocpm_eval.validation import validate_run

    validate_run(args.run, force=args.force)
    return 0


def _analyze(args: argparse.Namespace) -> int:
    from saocpm_eval.analytics.runner import analyze_run

    analyze_run(args.run, force=args.force)
    return 0


def _robustness(args: argparse.Namespace) -> int:
    scenario: Scenario = args.scenario
    config = load_config(args.config, scenario)
    from saocpm_eval.analytics.robustness import run_robustness

    run_robustness(config=config, config_path=args.config, output_dir=args.out)
    return 0


def _benchmark(args: argparse.Namespace) -> int:
    matrix = load_scale_matrix(args.matrix)
    from saocpm_eval.analytics.benchmark import run_benchmark

    run_benchmark(matrix=matrix, matrix_path=args.matrix, output_dir=args.out)
    return 0


def _tables(args: argparse.Namespace) -> int:
    from saocpm_eval.analytics.tables import write_paper_tables

    write_paper_tables(
        run_dirs=args.run,
        robustness_dirs=args.robustness,
        benchmark_dirs=args.benchmark,
        output_dir=args.out,
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="saocpm_eval",
        description="Generate and evaluate the FLOWVAULT SA-OCPM scenarios.",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    parser.add_argument("--verbose", action="store_true", help="enable diagnostic logging")
    subparsers = parser.add_subparsers(dest="command", required=True)

    generate = subparsers.add_parser("generate", help="generate an observed OCEL and truth data")
    generate.add_argument("scenario", choices=("inventory", "manufacturing"))
    generate.add_argument("--config", type=_path, required=True)
    generate.add_argument("--out", type=_path, required=True)
    generate.set_defaults(handler=_generate)

    validate = subparsers.add_parser("validate", help="validate a generated run")
    validate.add_argument("run", type=_path)
    validate.add_argument("--force", action="store_true", help="ignore a valid completion record")
    validate.set_defaults(handler=_validate)

    analyze = subparsers.add_parser("analyze", help="compute evaluation metrics for a run")
    analyze.add_argument("run", type=_path)
    analyze.add_argument("--force", action="store_true", help="ignore a valid completion record")
    analyze.set_defaults(handler=_analyze)

    robustness = subparsers.add_parser("robustness", help="run a scenario perturbation matrix")
    robustness.add_argument("scenario", choices=("inventory", "manufacturing"))
    robustness.add_argument("--config", type=_path, required=True)
    robustness.add_argument("--out", type=_path, default=_path("artifacts/robustness"))
    robustness.set_defaults(handler=_robustness)

    benchmark = subparsers.add_parser("benchmark", help="run the performance scale matrix")
    benchmark.add_argument("--matrix", type=_path, required=True)
    benchmark.add_argument("--out", type=_path, default=_path("artifacts/benchmark"))
    benchmark.set_defaults(handler=_benchmark)

    tables = subparsers.add_parser("tables", help="create paper-ready CSV and LaTeX tables")
    tables.add_argument("--run", type=_path, action="append", required=True)
    tables.add_argument("--robustness", type=_path, action="append", default=[])
    tables.add_argument("--benchmark", type=_path, action="append", default=[])
    tables.add_argument("--out", type=_path, default=_path("artifacts/tables"))
    tables.set_defaults(handler=_tables)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    configure_logging(args.verbose)
    handler: Command = args.handler
    try:
        return handler(args)
    except ValueError as exc:
        LOGGER.error("%s", exc)
        return 2
