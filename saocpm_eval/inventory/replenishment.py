"""Periodic-review policy and supplier response calculations."""

from __future__ import annotations

import math

from numpy.random import Generator


def periodic_order_quantity(
    inventory_position: float,
    reorder_point: float,
    order_up_to_level: float,
    minimum_order_quantity: float,
    lot_size: float,
) -> float:
    if order_up_to_level < reorder_point:
        raise ValueError("order-up-to level must not be below the reorder point")
    if minimum_order_quantity <= 0 or lot_size <= 0:
        raise ValueError("minimum order quantity and lot size must be positive")
    if inventory_position > reorder_point:
        return 0.0
    raw = max(minimum_order_quantity, order_up_to_level - inventory_position)
    return float(math.ceil(raw / lot_size) * lot_size)


def sample_supplier_lead_time_days(
    rng: Generator,
    mean_days: float,
    coefficient_of_variation: float,
    multiplier: float = 1.0,
) -> float:
    if mean_days <= 0 or coefficient_of_variation < 0 or multiplier <= 0:
        raise ValueError("supplier lead-time parameters are out of range")
    if coefficient_of_variation == 0:
        return mean_days * multiplier
    sigma = math.sqrt(math.log(coefficient_of_variation**2 + 1.0))
    mu = math.log(mean_days) - sigma**2 / 2
    return float(rng.lognormal(mu, sigma) * multiplier)
