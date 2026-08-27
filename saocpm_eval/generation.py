"""Top-level dispatch for scenario generation."""

from pathlib import Path

from saocpm_eval.common.hashing import config_sha256
from saocpm_eval.config import ConfigEnvelope, load_yaml


def generate_run(*, config: ConfigEnvelope, config_path: Path, output_dir: Path) -> None:
    """Dispatch generation after complete scenario-specific validation."""

    manifest_path = output_dir / "manifest.json"
    if manifest_path.is_file():
        from saocpm_eval.validation import preflight_run

        manifest = preflight_run(output_dir)
        expected_hash = config_sha256(load_yaml(config_path))
        identity = (config.scenario, config.profile, config.seed, expected_hash)
        observed = (
            manifest.get("scenario"),
            manifest.get("profile"),
            manifest.get("seed"),
            manifest.get("config_sha256"),
        )
        if observed == identity:
            return
        raise ValueError(
            "generation output already belongs to a different dataset configuration"
        )

    if config.scenario == "inventory":
        from saocpm_eval.inventory.config import load_inventory_config
        from saocpm_eval.inventory.simulation import generate_inventory_golden

        inventory_config = load_inventory_config(config_path)
        if inventory_config.profile == "golden":
            generate_inventory_golden(inventory_config, config_path, output_dir)
            return
        if inventory_config.profile in {"smoke", "paper"}:
            from saocpm_eval.inventory.stochastic import generate_inventory_stochastic

            generate_inventory_stochastic(inventory_config, config_path, output_dir)
            return
        raise ValueError(f"inventory profile {inventory_config.profile!r} is not implemented yet")
    if config.scenario == "manufacturing":
        from saocpm_eval.manufacturing.config import load_manufacturing_config
        from saocpm_eval.manufacturing.simulation import generate_manufacturing_golden

        manufacturing_config = load_manufacturing_config(config_path)
        if manufacturing_config.profile == "golden":
            generate_manufacturing_golden(manufacturing_config, config_path, output_dir)
            return
        if manufacturing_config.profile in {"smoke", "paper"}:
            from saocpm_eval.manufacturing.stochastic import generate_manufacturing_stochastic

            generate_manufacturing_stochastic(manufacturing_config, config_path, output_dir)
            return
        raise ValueError(
            f"manufacturing profile {manufacturing_config.profile!r} is not implemented yet"
        )
    raise ValueError(f"generation for {config.scenario!r} is not implemented yet")
