"""Deterministic paired potential outcomes for the registered interventions."""

from __future__ import annotations

from typing import Any

import numpy as np


def inventory_causal_truth(rng: np.random.Generator, *, profile: str, pairs: int) -> dict[str, Any]:
    if profile == "golden":
        return {"scenario": "inventory", "design": "not enabled in golden profile"}
    exogenous_demand = rng.gamma(shape=2.5, scale=18.0, size=pairs)
    supplier_delay = rng.lognormal(mean=1.8, sigma=0.45, size=pairs)
    untreated_recovery = 24.0 + 1.8 * supplier_delay + 0.35 * exogenous_demand
    treated_recovery = untreated_recovery * 0.62
    untreated_shortage = exogenous_demand * (1.0 + supplier_delay / 20.0)
    treated_shortage = untreated_shortage * 0.68
    untreated_cost = 40.0 * untreated_shortage
    treated_cost = 40.0 * treated_shortage + 350.0
    assigned = rng.integers(0, 2, size=pairs)
    rows = [
        {
            "pair_id": f"INV-PAIR-{index + 1:04d}",
            "assigned_treatment": bool(assigned[index]),
            "recovery_hours_treated": float(treated_recovery[index]),
            "recovery_hours_untreated": float(untreated_recovery[index]),
            "shortage_units_treated": float(treated_shortage[index]),
            "shortage_units_untreated": float(untreated_shortage[index]),
            "total_cost_treated": float(treated_cost[index]),
            "total_cost_untreated": float(untreated_cost[index]),
        }
        for index in range(pairs)
    ]
    return {
        "scenario": "inventory",
        "design": "paired randomized expedite eligibility",
        "intervention": "expedite eligible Critical Understock replenishment",
        "shared_exogenous_streams": ["demand", "supplier delay"],
        "assignment_probability": 0.5,
        "true_effects": {
            "recovery_hours": float(np.mean(treated_recovery - untreated_recovery)),
            "shortage_units": float(np.mean(treated_shortage - untreated_shortage)),
            "total_cost": float(np.mean(treated_cost - untreated_cost)),
        },
        "paired_potential_outcomes": rows,
    }


def manufacturing_causal_truth(
    rng: np.random.Generator, *, profile: str, pairs: int
) -> dict[str, Any]:
    if profile == "golden":
        return {"scenario": "manufacturing", "design": "not enabled in golden profile"}
    wear = rng.beta(3.0, 2.0, size=pairs)
    workload = rng.uniform(0.4, 1.0, size=pairs)
    untreated_failure = np.clip(0.08 + 0.65 * wear * workload, 0.0, 1.0)
    treated_failure = untreated_failure * 0.58
    untreated_down = 2.0 + 18.0 * wear + 6.0 * workload
    treated_down = untreated_down * 0.64
    untreated_defects = 1.0 + 9.0 * wear * workload
    treated_defects = untreated_defects * 0.7
    untreated_cost = 900.0 * untreated_down + 250.0 * untreated_defects
    treated_cost = 900.0 * treated_down + 250.0 * treated_defects + 1800.0
    assigned = rng.integers(0, 2, size=pairs)
    rows = [
        {
            "pair_id": f"MFG-PAIR-{index + 1:04d}",
            "assigned_treatment": bool(assigned[index]),
            "failure_probability_treated": float(treated_failure[index]),
            "failure_probability_untreated": float(untreated_failure[index]),
            "down_hours_treated": float(treated_down[index]),
            "down_hours_untreated": float(untreated_down[index]),
            "quality_defects_treated": float(treated_defects[index]),
            "quality_defects_untreated": float(untreated_defects[index]),
            "maintenance_cost_treated": float(treated_cost[index]),
            "maintenance_cost_untreated": float(untreated_cost[index]),
        }
        for index in range(pairs)
    ]
    return {
        "scenario": "manufacturing",
        "design": "paired randomized dispatch priority",
        "intervention": "priority maintenance dispatch eligibility",
        "shared_exogenous_streams": ["production schedule", "wear", "sensor noise"],
        "assignment_probability": 0.5,
        "true_effects": {
            "failure_probability": float(np.mean(treated_failure - untreated_failure)),
            "down_hours": float(np.mean(treated_down - untreated_down)),
            "quality_defects": float(np.mean(treated_defects - untreated_defects)),
            "maintenance_cost": float(np.mean(treated_cost - untreated_cost)),
        },
        "paired_potential_outcomes": rows,
    }
