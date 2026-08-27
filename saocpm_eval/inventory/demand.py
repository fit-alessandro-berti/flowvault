"""Inventory demand-class processes."""

from __future__ import annotations

import math

from numpy.random import Generator

DEMAND_CLASSES = (
    "smooth",
    "intermittent",
    "seasonal",
    "trending",
    "promotion_sensitive",
)


def daily_demand(
    rng: Generator,
    demand_class: str,
    base_rate: float,
    day_index: int,
) -> float:
    if demand_class not in DEMAND_CLASSES:
        raise ValueError(f"unknown demand class {demand_class!r}")
    if base_rate < 0:
        raise ValueError("base demand rate must be non-negative")
    if day_index < 0:
        raise ValueError("day index must be non-negative")
    adjusted = base_rate
    if demand_class == "intermittent":
        if float(rng.random()) >= min(0.5, base_rate / 10.0):
            return 0.0
        return float(rng.negative_binomial(2, 0.45) + 1)
    if demand_class == "seasonal":
        adjusted *= max(0.1, 1.0 + 0.55 * math.sin(2 * math.pi * day_index / 7))
    elif demand_class == "trending":
        adjusted *= min(2.0, 1.0 + day_index * 0.002)
    elif demand_class == "promotion_sensitive" and day_index % 30 in {0, 1, 2}:
        adjusted *= 3.0
    return float(rng.poisson(adjusted))
