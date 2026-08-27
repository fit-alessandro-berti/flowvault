# FlowVault event-log analysis screenshots

These screenshots were captured on 2026-08-27 by running the production FlowVault browser application against the two paper-scale OCEL 2.0 logs in `results/saocpm_15x/`.

The screenshots are intended to document the most informative analysis views rather than every available screen. Because both source files are named `observed.ocel.json`, the `inventory_` and `manufacturing_` filename prefixes identify the source log unambiguously.

## Analysis scope

| Log | Events | Objects | E2O | O2O | Leading object | Derived states | Transitions |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| Inventory | 46,256 | 18,506 | 187,191 | 18,551 | `ItemLocation` | 5 | 10,833 |
| Manufacturing | 43,458 | 3,578 | 92,581 | 3,554 | `Machine` | 8 | 11,196 |

For the inventory log, the state query classifies each event as `Unknown`, `Critical Understock`, `Understock`, `Overstock`, or `Normal`. For the manufacturing log, the machine-oriented query classifies events as `Unknown`, `Down`, `Quality Hold`, `Recovery`, `Setup`, `Degraded`, `Running`, or `Idle`. Both queries enriched every event in their respective logs.

## Inventory screenshots

### `01_inventory_log_statistics.jpg`

The unfiltered import summary. It establishes the scale and structural complexity of the inventory log: 46,256 events, 18,506 objects, 24 event types, 10 object types, and 187,191 event-to-object relationships. The zero stateful-event count records the baseline before enrichment.

### `02_inventory_intra_state_pattern_graph.jpg`

The most frequent intra-state pattern after enrichment. It shows 1,595 occurrences of `Sales Order Item Created` in the `Unknown` state and its relationships to `ItemLocation`, `Location`, `Material`, and `SalesOrderItem`. The page also records that FlowVault found 1,424 distinct intra-state and 3,765 distinct inter-state patterns.

### `03_inventory_inter_state_transition_details.jpg`

The detailed representation of the leading inter-state pattern: `Critical Understock -> Unknown`, with support 92 and mass 368. The control sequence links `Sales Order Item Created` to the explicit state-change node and then to `Backorder Registered`, while the lower sections preserve directly-follows and object-type context counts.

### `03_inventory_inter_state_transition_graph.jpg`

The full-screen graphical form of the same `Critical Understock -> Unknown` pattern. It emphasizes the explicit state-change node between the two event activities and the participating object types. The graph is wider than the browser viewport, so this capture documents the leading portion of the horizontally scrollable canvas.

### `04_inventory_transition_kpis.jpg`

The transition-matrix and duration summary for all 48 `ItemLocation` objects. The largest transition counts are `Unknown -> Critical Understock` (3,379) and `Critical Understock -> Unknown` (3,358). The view also exposes dwell-time, recovery-time, and long-running-state outliers, making it the main operational KPI screenshot for this log.

### `05_inventory_time_perspective.jpg`

The temporal state-frequency view over 32 time buckets. `Critical Understock` accounts for 61.9% of classified events, followed by `Normal` at 21.3% and `Unknown` at 13.7%. The lower performance spectrum shows the timing distribution for a selected state pair over the two-year observation period.

### `06_inventory_lifecycle_timeline.jpg`

The lifecycle of `ItemLocation` object `IL-0001`, which contains 567 events. The upper band visualizes state changes from January 2023 through January 2025; the event table below ties those intervals back to concrete activities such as initialization, goods issue, backorder registration, replenishment, and goods receipt.

### `07_inventory_state_aware_ocdfg.jpg`

A filtered state-aware object-centric directly-follows graph for `ItemLocation`. Activity frequency is set to 500 and path frequency to 250, reducing the full graph to 18 nodes and 39 edges so high-volume state-conditioned behavior remains readable. The screenshot also preserves the exact filters used to produce the view.

### `08_inventory_state_detection_som.jpg`

The unsupervised state-detection workspace. FlowVault generated 51 features and 46,112 lifecycle windows for 48 objects, projected them with PCA, and organized them into a 3 x 3 self-organizing map. The map and ranked transition list show where execution windows concentrate and which neighboring cells exchange the most windows.

## Manufacturing screenshots

### `09_manufacturing_log_statistics.jpg`

The unfiltered manufacturing-log baseline: 43,458 events, 3,578 objects, 30 event types, 11 object types, and 92,581 event-to-object relationships. As in the inventory baseline, the stateful-event count is zero before applying the state query.

### `10_manufacturing_intra_state_pattern_graph.jpg`

The dominant manufacturing intra-state pattern: 3,694 `Sensor Snapshot` events in the `Unknown` state, related to `Machine` and `Component`. FlowVault found 586 distinct intra-state and 1,708 distinct inter-state patterns after enriching all 43,458 events.

### `11_manufacturing_transition_kpis.jpg`

The transition analysis for all eight machines. The most common movements are `Running -> Unknown` (3,963) and `Unknown -> Running` (3,960), followed by the balanced `Setup -> Running` and `Running -> Setup` pair (843 and 842). The matrix also highlights operational sequences such as `Degraded -> Down` and `Down -> Recovery`, with dwell-time and stuck-state summaries below.

### `12_manufacturing_time_perspective.jpg`

The state-frequency trend from January 2024 to March 2025. `Running` dominates at 77.6%, while `Unknown` accounts for 11.0%, `Down` for 5.2%, and `Recovery` for 2.8%. The lower performance spectrum provides duration statistics for a selected state pair across the same observation window.

### `13_manufacturing_state_detection_som.jpg`

The machine-level unsupervised state map. The analysis uses 60 features and 43,434 lifecycle windows from eight machines; most windows cluster in cell `S3-1` (30,777). The adjacent list ranks movement between SOM cells, providing a compact view of recurrent execution-regime changes.

## Interpretation notes

- `Unknown` is produced when `event.data_complete = false`; frequent transitions through it therefore indicate incomplete observations in the generated log, not necessarily a physical process state.
- Pattern support counts occurrences of a structurally identical segment, while mass summarizes the total control-flow weight represented by that pattern.
- The self-organizing-map states are unsupervised execution clusters and are distinct from the rule-derived business states used by the state-aware pattern, KPI, timeline, and time-perspective pages.
