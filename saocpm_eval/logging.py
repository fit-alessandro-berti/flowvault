"""Deterministic command-line logging."""

from __future__ import annotations

import logging
import sys


def configure_logging(verbose: bool = False) -> None:
    """Configure a stable message-only logger for reproducible command output."""

    logging.basicConfig(
        level=logging.DEBUG if verbose else logging.INFO,
        format="%(levelname)s %(name)s: %(message)s",
        stream=sys.stderr,
        force=True,
    )
