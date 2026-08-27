"""SOM-cell alignment and hidden-regime quality measures."""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Sequence
from dataclasses import dataclass
from itertools import pairwise

import numpy as np
from scipy.optimize import linear_sum_assignment
from sklearn.metrics import (
    adjusted_rand_score,
    balanced_accuracy_score,
    normalized_mutual_info_score,
)


@dataclass(frozen=True, slots=True)
class SomAlignment:
    mapping: dict[str, str]
    purity: float
    adjusted_rand_index: float
    normalized_mutual_information: float
    balanced_accuracy: float
    mean_cell_entropy: float
    empty_cell_rate: float


def align_cells(
    cells: Sequence[str],
    labels: Sequence[str],
    *,
    total_cell_count: int | None = None,
) -> SomAlignment:
    if len(cells) != len(labels) or not cells:
        raise ValueError("cell alignment requires equal non-empty cell and label sequences")
    unique_cells = sorted(set(cells))
    unique_labels = sorted(set(labels))
    matrix = np.zeros((len(unique_cells), len(unique_labels)), dtype=int)
    cell_index = {cell: index for index, cell in enumerate(unique_cells)}
    label_index = {label: index for index, label in enumerate(unique_labels)}
    for cell, label in zip(cells, labels, strict=True):
        matrix[cell_index[cell], label_index[label]] += 1
    rows, columns = linear_sum_assignment(-matrix)
    mapping = {
        unique_cells[row]: unique_labels[column] for row, column in zip(rows, columns, strict=True)
    }
    fallback = Counter(labels).most_common(1)[0][0]
    mapped = [mapping.get(cell, fallback) for cell in cells]
    entropies = []
    weights = []
    for row in matrix:
        total = int(row.sum())
        if not total:
            continue
        probabilities = row[row > 0] / total
        entropies.append(float(-np.sum(probabilities * np.log2(probabilities))))
        weights.append(total)
    declared_cells = total_cell_count or len(unique_cells)
    if declared_cells < len(unique_cells):
        raise ValueError("total cell count cannot be smaller than occupied cells")
    return SomAlignment(
        mapping=mapping,
        purity=float(matrix.max(axis=1).sum() / len(cells)),
        adjusted_rand_index=float(adjusted_rand_score(labels, cells)),
        normalized_mutual_information=float(normalized_mutual_info_score(labels, cells)),
        balanced_accuracy=float(balanced_accuracy_score(labels, mapped)),
        mean_cell_entropy=float(np.average(entropies, weights=weights)),
        empty_cell_rate=(declared_cells - len(unique_cells)) / declared_cells,
    )


def nearby_transition_proportion(
    object_ids: Sequence[str], cells: Sequence[tuple[int, int]]
) -> float:
    if len(object_ids) != len(cells):
        raise ValueError("object and cell sequences must have equal lengths")
    grouped: dict[str, list[tuple[int, int]]] = defaultdict(list)
    for object_id, cell in zip(object_ids, cells, strict=True):
        grouped[object_id].append(cell)
    distances = [
        abs(left[0] - right[0]) + abs(left[1] - right[1])
        for sequence in grouped.values()
        for left, right in pairwise(sequence)
        if left != right
    ]
    return sum(distance <= 1 for distance in distances) / len(distances) if distances else 1.0
