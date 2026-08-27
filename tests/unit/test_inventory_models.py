import numpy as np
import pytest

from saocpm_eval.inventory.demand import DEMAND_CLASSES, daily_demand
from saocpm_eval.inventory.entities import InventoryState
from saocpm_eval.inventory.replenishment import (
    periodic_order_quantity,
    sample_supplier_lead_time_days,
)
from saocpm_eval.inventory.state_reference import reference_state


def test_all_demand_classes_are_non_negative_and_reproducible() -> None:
    for demand_class in DEMAND_CLASSES:
        first = np.random.default_rng(42)
        second = np.random.default_rng(42)
        left = [daily_demand(first, demand_class, 4.0, day) for day in range(120)]
        right = [daily_demand(second, demand_class, 4.0, day) for day in range(120)]
        assert left == right
        assert min(left) >= 0


def test_promotion_window_increases_expected_demand() -> None:
    promoted = np.mean(
        [
            daily_demand(np.random.default_rng(seed), "promotion_sensitive", 10.0, 1)
            for seed in range(500)
        ]
    )
    baseline = np.mean(
        [
            daily_demand(np.random.default_rng(seed), "promotion_sensitive", 10.0, 10)
            for seed in range(500)
        ]
    )
    assert promoted > baseline * 2.5


def test_periodic_policy_respects_reorder_point_minimum_and_lot() -> None:
    assert periodic_order_quantity(21, 20, 80, 10, 12) == 0
    assert periodic_order_quantity(15, 20, 80, 10, 12) == 72
    assert periodic_order_quantity(79, 80, 80, 10, 12) == 12


def test_supplier_lead_times_are_positive_and_have_target_mean() -> None:
    rng = np.random.default_rng(7)
    samples = np.array([sample_supplier_lead_time_days(rng, 10.0, 0.4) for _ in range(20_000)])
    assert np.all(samples > 0)
    assert float(np.mean(samples)) == pytest.approx(10.0, rel=0.02)


@pytest.mark.parametrize(
    ("state", "expected"),
    [
        (InventoryState(50, 20, 80), "Normal"),
        (InventoryState(10, 20, 80), "Understock"),
        (InventoryState(90, 20, 80), "Overstock"),
        (
            InventoryState(10, 20, 80, confirmed_demand_horizon=20),
            "Critical Understock",
        ),
        (InventoryState(50, 20, 80, data_complete=False), "Unknown"),
    ],
)
def test_independent_reference_state_precedence(state: InventoryState, expected: str) -> None:
    assert reference_state(state).name == expected
