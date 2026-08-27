"""Deterministic identifier allocation."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(slots=True)
class DeterministicIds:
    """Allocate monotonically increasing, fixed-width identifiers."""

    prefix: str
    width: int = 6
    start: int = 1
    _next_value: int = field(init=False, repr=False)

    def __post_init__(self) -> None:
        if not self.prefix:
            raise ValueError("identifier prefix must not be empty")
        if self.width < 1:
            raise ValueError("identifier width must be positive")
        if self.start < 0:
            raise ValueError("identifier start must be non-negative")
        self._next_value = self.start

    def next(self) -> str:
        identifier = f"{self.prefix}{self._next_value:0{self.width}d}"
        self._next_value += 1
        return identifier
