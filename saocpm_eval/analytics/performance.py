"""Wall-clock, CPU, and peak-memory collection for benchmark operations."""

from __future__ import annotations

import resource
import time
from collections.abc import Callable
from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class PerformanceMeasurement:
    value: object
    wall_time_ms: float
    cpu_time_ms: float
    peak_rss_bytes: int


def measure[T](operation: Callable[[], T]) -> PerformanceMeasurement:
    wall_start = time.perf_counter()
    cpu_start = time.process_time()
    value = operation()
    cpu_time_ms = (time.process_time() - cpu_start) * 1000
    wall_time_ms = (time.perf_counter() - wall_start) * 1000
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    peak_rss_bytes = int(peak * 1024)
    return PerformanceMeasurement(value, wall_time_ms, cpu_time_ms, peak_rss_bytes)
