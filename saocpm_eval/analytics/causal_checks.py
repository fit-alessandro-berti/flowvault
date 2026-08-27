"""Paired randomized-intervention checks against stored causal truth."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass

import numpy as np


@dataclass(frozen=True, slots=True)
class PairedEffect:
    average_treatment_effect: float
    standard_error: float
    sign_matches_truth: bool | None
    magnitude_error: float | None


def paired_effect(
    treated: Sequence[float],
    untreated: Sequence[float],
    truth_effect: float | None = None,
) -> PairedEffect:
    if len(treated) != len(untreated) or not treated:
        raise ValueError("paired effects require equal non-empty potential outcomes")
    differences = np.asarray(treated, dtype=float) - np.asarray(untreated, dtype=float)
    estimate = float(np.mean(differences))
    standard_error = (
        float(np.std(differences, ddof=1) / np.sqrt(len(differences)))
        if len(differences) > 1
        else 0.0
    )
    return PairedEffect(
        average_treatment_effect=estimate,
        standard_error=standard_error,
        sign_matches_truth=(
            bool(np.sign(estimate) == np.sign(truth_effect)) if truth_effect is not None else None
        ),
        magnitude_error=abs(estimate - truth_effect) if truth_effect is not None else None,
    )
