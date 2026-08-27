import numpy as np

from saocpm_eval.common.rng import SeedTree


def test_named_streams_are_independent_of_request_order() -> None:
    first = SeedTree(42)
    demand_first = first.stream("exogenous demand").integers(0, 100, size=8)
    supplier_second = first.stream("supplier response").integers(0, 100, size=8)

    second = SeedTree(42)
    supplier_first = second.stream("supplier response").integers(0, 100, size=8)
    demand_second = second.stream("exogenous demand").integers(0, 100, size=8)

    np.testing.assert_array_equal(demand_first, demand_second)
    np.testing.assert_array_equal(supplier_second, supplier_first)


def test_seed_tree_metadata_records_used_streams() -> None:
    tree = SeedTree(7)
    tree.stream("sensor noise")
    assert tree.metadata()["root_seed"] == 7
    assert "sensor noise" in tree.metadata()["streams"]  # type: ignore[operator]
