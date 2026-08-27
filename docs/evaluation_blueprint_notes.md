# Notes relative to the current FLOWVAULT repository

The blueprint intentionally reflects the current implementation inspected in the supplied repository.

- State queries enrich events with one string `state` attribute and select one leading object type.
- Object attributes are resolved at the event timestamp.
- Automatic state detection uses event-count lifecycle windows, activity counts, distinct related-object counts, and leading-object attribute values at the window endpoint, followed by standardized PCA and deterministic SOM training.
- The current state-detection view truncates projected windows, so a full assignment export is required for quantitative evaluation.
- Current pattern keys use exact episode sequences and exact sets of event-object and object-object type pairs. Passive observation events can therefore fragment support.
- Current recovery classification is based on target-state name substrings and should become explicit for the manufacturing taxonomy.
- Current state storage is event-level rather than leading-object-incidence-level. The generators avoid ambiguous events with two leading objects.

## Bounded-compute execution protocol

The paper and smoke analyses target at most 300 seconds of wall time per dataset. The bound is part of the protocol rather than an unreported post-hoc shortcut:

- PCA basis fitting and SOM weight training use at most 2,000 deterministic, evenly spaced windows and 10 SOM epochs. The fitted PCA basis projects every lifecycle window in one linear pass, and final SOM assignment still covers the complete window population.
- Import, query application, export, transition KPIs, OC-DFGs, PCA/SOM training, and full assignment execute in one native process after one OCEL parse. The assignment response includes PCA and SOM summaries, eliminating the former duplicate training call.
- Prediction tasks use at most 1,000 label-stratified observations selected at deterministic, evenly spaced label ordinals before feature construction. Both temporal and grouped holdouts are then formed from that fixed sample.
- Bootstrap, period, transfer, and cell-explanation diagnostics use at most 2,000 deterministic, evenly spaced windows. Primary alignment scores and warning evaluation continue to use all windows.
- The scale matrix uses 5k and 20k-event profiles, one measured repetition, no warm-up, and only five core operations. Runs remain deterministic and resumable, so larger profiles can be scheduled explicitly outside the bounded experiment suite.

Every analysis writes `analytics/scalability_budget.json` and `analytics/prediction_sampling.csv` so population counts, sample counts, and sampling rules remain auditable.

Generation and the explicit `validate` command retain checksum and full semantic validation. Analysis uses a manifest-schema, required-file, and file-size preflight because rerunning multi-gigabyte hashing and semantic validation inside every experiment duplicates work without changing analytical results.

## Modular completion and resume semantics

- Generation treats a matching, preflight-complete dataset manifest as success and returns without regenerating it. A mismatched configuration fails instead of overwriting the existing dataset.
- Validation records the implementation fingerprint and an input file snapshot in `analytics/validation_manifest.json`. Unchanged validated datasets are no-ops; `validate --force` performs an intentional revalidation.
- Analysis records its protocol, implementation and input fingerprint, plus its output-size inventory, in `analytics/analysis_manifest.json`. A matching complete dataset is skipped before any OCEL import; `analyze --force` intentionally recomputes it.
- Robustness stores an experiment-definition manifest, caches the clean baseline, and checkpoints every perturbation. Completed perturbations and fully completed matrices are not evaluated again.
- Scale benchmarking checkpoints every `(profile, repetition, operation)` tuple, caches each fixture summary, and skips a profile before loading its dataset when all requested tuples already exist.

Completion records are written only after their module succeeds. Changes to configuration, input identity, protocol, or relevant implementation sources invalidate the corresponding record automatically.
