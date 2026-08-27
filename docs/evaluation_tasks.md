# Ordered Codex implementation tasks

Complete tasks in order. Each task is independently testable and should end with passing tests before the next task starts.

## Task 1: create the Python package and CLI

Implement `saocpm_eval` with commands `generate`, `validate`, `analyze`, `robustness`, and `benchmark`. Use typed dataclasses or Pydantic models for configuration. Add Ruff, MyPy, Pytest, and deterministic logging.

Done when:

```bash
python -m saocpm_eval --help
pytest -q
ruff check .
mypy saocpm_eval
```

all succeed.

## Task 2: implement a strict OCEL 2.0 builder

Implement typed object and event declarations, object attribute histories, E2O and O2O relations, deterministic IDs, chronological sorting, JSON export, and structural validation. Reject duplicate IDs, undeclared attributes, invalid value types, unknown relationships, and multi-leading-object events.

Add tests using `examples/ocel_fragment.json` and one deliberately invalid file per validation rule.

## Task 3: implement common truth and manifest writers

Write CSV and JSON sidecars, SHA-256 hashes, configuration hashes, seed-tree metadata, expected counts, and canonical JSON serialization. Same seed and config must be byte-for-byte reproducible.

## Task 4: implement the inventory golden simulator

Implement only the golden profile first. Include initialization, sales, issue, proposal, purchase order, receipt, transfer, count adjustment, policy update, data gap, and final snapshot. Implement the independent reference state and stock-conservation validator.

Create at least one exact instance of every inventory state, transition, pattern, and conformance violation.

## Task 5: add FLOWVAULT inventory integration tests

Copy the golden fixture into the Rust test fixtures. Apply `queries/inventory_state.sql`, export JSON, and compare every state to the sidecar oracle. Assert exact transition KPIs and presence/support of canonical patterns.

## Task 6: implement stochastic inventory profiles

Add demand classes, `(s,S)` policy, supplier lead times, planner delays, transfers, forced mechanisms, and perturbations. Generate smoke and paper profiles.

## Task 7: implement manufacturing physical and workflow state machines

Implement machine health, component wear, sensors, alarms, production, quality, maintenance, hysteresis, recovery, and final snapshots. Keep hidden physical state out of the observed OCEL.

## Task 8: implement the manufacturing golden fixture and tests

Guarantee all states, transitions, six canonical patterns, and six conformance rules. Apply `queries/manufacturing_state.sql` and compare with the independent state oracle.

## Task 9: extend FLOWVAULT for evaluation

Implement:

1. full state-detection assignment JSON or CSV;
2. configurable pattern radius, ignored activities, minimum support, and occurrences;
3. explicit recovery transitions in KPI requests;
4. a headless Rust CLI over `OcelDocumentCore`.

Preserve all existing API behavior and tests.

## Task 10: implement analytics oracles

Implement state agreement, episode extraction, transition matching, graph heterogeneity, pattern scoring, conformance, SOM alignment, prediction labeling, temporal purging, and performance collection. Add hand-calculated micro-tests for each metric.

## Task 11: implement prediction baselines

Use reproducible scikit-learn pipelines. Start with logistic regression and histogram gradient boosting. Add static-only, raw-only, process-only, state-only, object-context-only, and full feature sets. Never split overlapping windows across train and test.

## Task 12: implement robustness and benchmark runners

Run the predefined perturbation and scale matrices. Resume safely after interruption, write one immutable row per run, and include commit/config hashes.

## Task 13: add Playwright end-to-end tests

Test file load, state query, SA-OCDFG, KPI, pattern, state detection, cell detail, and export for both smoke scenarios.

## Task 14: create paper-ready result tables

Produce machine-readable CSV and LaTeX tables for data characteristics, state validity, automatic-state quality, analytical tasks, prediction, robustness, and performance. Keep pre-specified and exploratory results separate.
