import pytest

from saocpm_eval.common.ids import DeterministicIds


def test_deterministic_ids_are_fixed_width_and_repeatable() -> None:
    first = DeterministicIds("INV-E-", width=4)
    second = DeterministicIds("INV-E-", width=4)
    assert [first.next(), first.next()] == ["INV-E-0001", "INV-E-0002"]
    assert [second.next(), second.next()] == ["INV-E-0001", "INV-E-0002"]


def test_deterministic_ids_reject_invalid_configuration() -> None:
    with pytest.raises(ValueError, match="prefix"):
        DeterministicIds("")
