"""Order-independent named random streams backed by NumPy SeedSequence."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass, field

import numpy as np
from numpy.random import Generator, SeedSequence


def _spawn_key(name: str) -> tuple[int, ...]:
    if not name:
        raise ValueError("random stream name must not be empty")
    digest = hashlib.sha256(name.encode("utf-8")).digest()
    return tuple(int.from_bytes(digest[offset : offset + 4], "big") for offset in range(0, 16, 4))


@dataclass(slots=True)
class SeedTree:
    """Produce a stable stream for each semantic source of randomness.

    A stream's seed depends on the root seed and its name, not on request order. This keeps
    exogenous schedules stable when an unrelated generator module adds another stream.
    """

    root_seed: int
    _streams: dict[str, Generator] = field(default_factory=dict, init=False, repr=False)
    _keys: dict[str, tuple[int, ...]] = field(default_factory=dict, init=False, repr=False)

    def __post_init__(self) -> None:
        if self.root_seed < 0:
            raise ValueError("root seed must be non-negative")

    def stream(self, name: str) -> Generator:
        if name not in self._streams:
            key = _spawn_key(name)
            self._keys[name] = key
            self._streams[name] = np.random.default_rng(
                SeedSequence(entropy=self.root_seed, spawn_key=key)
            )
        return self._streams[name]

    def fresh_stream(self, name: str) -> Generator:
        """Return a new generator at the beginning of a named stream."""

        key = _spawn_key(name)
        self._keys[name] = key
        return np.random.default_rng(SeedSequence(entropy=self.root_seed, spawn_key=key))

    def metadata(self) -> dict[str, object]:
        return {
            "algorithm": "numpy.random.PCG64",
            "root_seed": self.root_seed,
            "streams": {name: {"spawn_key": list(key)} for name, key in sorted(self._keys.items())},
        }
