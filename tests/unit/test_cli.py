from pathlib import Path

import pytest

from saocpm_eval.cli import build_parser, main
from saocpm_eval.config import load_config


def test_cli_declares_all_commands() -> None:
    parser = build_parser()
    for command in ("generate", "validate", "analyze", "robustness", "benchmark", "tables"):
        with pytest.raises(SystemExit) as result:
            parser.parse_args([command, "--help"])
        assert result.value.code == 0


def test_validate_and_analyze_accept_explicit_force() -> None:
    parser = build_parser()
    assert parser.parse_args(["validate", "/tmp/run", "--force"]).force is True
    assert parser.parse_args(["analyze", "/tmp/run", "--force"]).force is True


def test_load_config_rejects_scenario_mismatch(tmp_path: Path) -> None:
    path = tmp_path / "config.yaml"
    path.write_text(
        "scenario: inventory\nprofile: golden\nseed: 1\n"
        'start_time: "2025-01-01T00:00:00Z"\nhorizon_days: 1\n',
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="expected 'manufacturing'"):
        load_config(path, "manufacturing")


def test_main_reports_configuration_error(tmp_path: Path) -> None:
    missing = tmp_path / "missing.yaml"
    assert main(["generate", "inventory", "--config", str(missing), "--out", str(tmp_path)]) == 2
