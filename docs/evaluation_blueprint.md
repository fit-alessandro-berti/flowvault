# Implementation blueprint for the two SA-OCPM evaluation scenarios

## 1. Evaluation claims and separation of concerns

The generators should support six distinct claims. They should not be collapsed into one accuracy number.

| Claim | What is tested | Ground truth |
|---|---|---|
| C1: expression correctness | FLOWVAULT applies a deterministic state notion correctly | independently computed manual state at every leading-object event |
| C2: state validity | the state notion corresponds to meaningful operational conditions | policy thresholds, machine condition, and reviewed transition boundaries |
| C3: automatic abstraction | PCA/SOM cells recover stable, interpretable execution regimes | hidden simulation regime at every window endpoint and over the complete window |
| C4: analytical utility | state-aware graphs, KPIs, and patterns expose mechanisms hidden by state-agnostic views | injected mechanisms, violations, causes, and outcomes |
| C5: predictive utility | state and object context improve warning and recovery models | future Understock or Down episodes and exact recovery times |
| C6: robustness and performance | findings survive perturbations and the implementation scales | clean-run reference outputs and controlled scale parameters |

The simulation code and the state-query code must be independent implementations. A bug shared by both must not produce artificial agreement.

## 2. Common generator architecture

### 2.1 Recommended package layout

```text
saocpm-eval/
  pyproject.toml
  configs/
  saocpm_eval/
    cli.py
    common/
      ids.py
      rng.py
      clock.py
      ocel_builder.py
      truth_writer.py
      perturbations.py
      validation.py
      hashing.py
    inventory/
      config.py
      entities.py
      demand.py
      replenishment.py
      simulation.py
      state_reference.py
      pattern_injection.py
    manufacturing/
      config.py
      entities.py
      production.py
      physics.py
      maintenance.py
      simulation.py
      state_reference.py
      pattern_injection.py
    analytics/
      episodes.py
      state_agreement.py
      som_evaluation.py
      graph_metrics.py
      pattern_evaluation.py
      conformance.py
      prediction.py
      causal_checks.py
      robustness.py
      benchmark.py
  tests/
    unit/
    integration/
    golden/
```

### 2.2 Reproducible random generation

Use `numpy.random.SeedSequence` and named child streams. Do not use one global stream for every purpose. A recommended hierarchy is:

```text
root seed
  entity parameters
  exogenous demand or production schedule
  supplier or maintenance response
  sensor or stock noise
  forced mechanisms
  missingness and corruption
  timestamp jitter
```

This permits paired counterfactual runs and makes output stable when one module changes. Store the root seed, child-spawn keys, configuration hash, generator commit, and output checksums in `manifest.json`.

### 2.3 Time and state semantics

All timestamps are UTC and serialized as ISO 8601. Within each leading-object lifecycle, timestamps must be strictly increasing. When several domain actions occur at the same conceptual time, assign deterministic offsets of at least one second according to a documented event-priority table.

The state attached to an event is the **post-event state**. The simulator performs the following steps:

1. read the pre-event object state;
2. apply the domain effect;
3. calculate the post-event operational attributes;
4. emit the event with both before and after values where applicable;
5. append changed object attributes at the event timestamp;
6. calculate the independent reference state and write it only to the sidecar truth file.

An initialization event establishes the initial state. A final observation event at the simulation horizon closes right-censored dwell intervals. This is necessary because FLOWVAULT measures episode duration from the first event in one state to the first event in the next state, and otherwise the final episode ends at the last observed event rather than at the study horizon.

### 2.4 OCEL constraints for the current FLOWVAULT implementation

1. Every event used for state analysis must relate to exactly one leading object. A transfer between two item-locations is represented by separate `Transfer Ship` and `Transfer Receive` events. A production order spanning machines uses machine-specific operation events. This avoids one event-level state being forced onto two leading objects with different conditions.
2. Every leading-object event type declares the common state-driving event attributes. FLOWVAULT validates event-type schemas.
3. Dynamic features needed by automatic state detection are also written as object-attribute histories. The current detector uses lifecycle activity counts, related-object counts, and leading-object attributes resolved at the window endpoint. Event-only sensor or stock values are not part of its feature vector.
4. Passive observation events are tagged through a boolean event attribute `passive_observation`. Pattern analysis should either filter them out before state enrichment or use the radius-limited pattern extension described in Section 8.
5. The observed OCEL contains no `reference_state`, `latent_regime`, future outcome, injected-pattern ID, or causal-treatment outcome.

### 2.5 Output contract for every generated run

```text
<run>/
  observed.ocel.json
  observed.behavior.ocel.json
  state_query.sql
  manifest.json
  truth/
    state_at_event.csv
    state_episodes.csv
    transitions.csv
    latent_regime_at_event.csv
    latent_regime_episodes.csv
    injected_pattern_instances.csv
    conformance_violations.csv
    prediction_samples.csv
    outcomes_by_object.csv
    causal_truth.json
  expected/
    summary.json
    branch_coverage.json
    golden_assertions.json
  analytics/
    # produced later by the analysis command
```

`observed.ocel.json` includes all events. `observed.behavior.ocel.json` omits passive sensor or inventory observations but retains the same objects and object histories. It is the preferred pattern fixture. Both logs are derived from the same clean simulation.

### 2.6 Ground-truth record definitions

`state_at_event.csv` has one row for each event related to a leading object:

```text
scenario,leading_object_type,leading_object_id,event_id,event_time,
reference_state,state_reason,policy_or_rule_version,data_complete,
state_before,state_after,is_transition,transition_id
```

`latent_regime_at_event.csv` contains:

```text
leading_object_id,event_id,event_time,primary_regime,
regime_factors_json,regime_started_at,transition_window
```

`injected_pattern_instances.csv` contains:

```text
pattern_id,instance_id,family,leading_object_id,start_event_id,end_event_id,
from_state,to_state,expected_sequence_json,expected_object_types_json,
noise_level,should_be_exact_in_behavior_log
```

`prediction_samples.csv` is generated without leakage and contains only feature timestamps and labels, not model features:

```text
leading_object_id,cutoff_event_id,cutoff_time,current_state,
label_name,horizon_minutes,label,time_to_event_minutes,split_group
```

## 3. Scenario A: inventory management

### 3.1 Objective

Create item-location lifecycles in which a policy state is directly definable from stock and thresholds, while hidden execution regimes explain why the state was entered, persisted, or resolved. The generator must create cohorts with similar activity sequences but different stock states so that ordinary OCDFGs merge behavior that SA-OCDFGs separate.

### 3.2 Object model

The leading object type is `ItemLocation`.

| Object type | Role | Important attributes |
|---|---|---|
| ItemLocation | leading item-location lifecycle | static material and location class; dynamic on-hand, reserved, backorder, on-order, inventory position, lower threshold, upper threshold, demand estimate, lead-time estimate, policy version, data completeness |
| Material | product identity | product class, unit cost, shelf life, lot size, criticality |
| Location | stocking site | region, capacity class, service-level target, planner team |
| Supplier | replenishment source | reliability, mean lead time, lead-time coefficient of variation, fill rate |
| SalesOrderItem | demand transaction | requested quantity, due time, priority, customer class |
| ReplenishmentProposal | policy recommendation | suggested quantity, reason, creation time, status |
| PurchaseOrderItem | committed inbound supply | ordered quantity, confirmed quantity, planned and actual receipt times, status |
| Delivery | shipment or receipt context | carrier, shipment status, delay code |
| TransferOrder | inter-location recovery | quantity, source item-location, target item-location, status |
| Planner | organizational context | team, workload band, experience band |

Recommended object-object relations include `ItemLocation --material--> Material`, `ItemLocation --location--> Location`, `PurchaseOrderItem --supplier--> Supplier`, `PurchaseOrderItem --replenishes--> ItemLocation`, `SalesOrderItem --consumes-from--> ItemLocation`, and `TransferOrder --source/target--> ItemLocation`.

### 3.3 Event catalog

All listed events include the common inventory snapshot attributes specified below. Transaction-specific attributes and related objects are added as needed.

| Event | Main effect | Required context |
|---|---|---|
| Initialize Inventory | creates initial stock and thresholds | ItemLocation, Material, Location |
| Sales Order Item Created | records confirmed demand | SalesOrderItem, Material, Location |
| Reservation Created | increases reserved stock | SalesOrderItem |
| Goods Issue | decreases on-hand and reservation | SalesOrderItem |
| Backorder Registered | increases backorder | SalesOrderItem |
| Demand Cancelled | decreases reservation or backorder | SalesOrderItem |
| Replenishment Proposal Created | policy response | ReplenishmentProposal, Planner |
| Replenishment Proposal Approved | planner response | ReplenishmentProposal, Planner |
| Purchase Order Item Created | increases on-order | PurchaseOrderItem, Supplier, Planner |
| Purchase Order Item Changed | changes inbound quantity or date | PurchaseOrderItem, Supplier |
| Supplier Confirmation Received | establishes expected delivery | PurchaseOrderItem, Supplier |
| Delivery Delayed | marks late inbound | Delivery, PurchaseOrderItem, Supplier |
| Expedite Requested | mitigation action | PurchaseOrderItem, Supplier, Planner |
| Goods Receipt | increases on-hand, decreases on-order/backorder | Delivery, PurchaseOrderItem, Supplier |
| Receipt Rejected | records quality or quantity rejection | Delivery, PurchaseOrderItem, Supplier |
| Transfer Requested | recovery proposal | TransferOrder, Planner |
| Transfer Ship | decreases source ItemLocation | TransferOrder, source Location |
| Transfer Receive | increases target ItemLocation | TransferOrder, target Location |
| Cycle Count Performed | observes physical stock | ItemLocation, Planner or auditor |
| Inventory Adjustment | changes on-hand due to discrepancy | ItemLocation |
| Policy Threshold Updated | changes lower or upper threshold | ItemLocation, Planner |
| Data Gap Started / Ended | controls Unknown state | ItemLocation |
| Simulation End Snapshot | closes the final episode | ItemLocation |

Common event attributes, declared on every event type related to `ItemLocation`:

```text
quantity: float
on_hand_before: float
on_hand_after: float
reserved_after: float
backorder_after: float
on_order_after: float
inventory_position_after: float
lower_threshold: float
upper_threshold: float
confirmed_demand_horizon: float
inbound_horizon: float
critical_understock: boolean
data_complete: boolean
passive_observation: boolean
cause_code: string
policy_version: string
```

### 3.4 Demand generation

Assign each item-location one demand class:

- smooth: daily demand follows a Poisson distribution;
- intermittent: a Bernoulli arrival process multiplied by a negative-binomial quantity;
- seasonal: a Poisson rate with weekly or annual sinusoidal modulation;
- trending: a rate with bounded positive or negative drift;
- promotion-sensitive: a base process plus scheduled promotion multipliers.

For item-location object `o` and day `d`, a general rate is:

```text
lambda[o,d] = base_rate[o]
            * weekly_factor[o,d]
            * seasonal_factor[o,d]
            * trend_factor[o,d]
            * promotion_factor[o,d]
            * injected_shock_factor[o,d]
```

Generate order creation times within the day. For each order, fulfill up to available stock and create a backorder for the remainder. Whether the system permits negative on-hand is configurable, but the default is non-negative on-hand plus explicit backorder.

### 3.5 Replenishment policy

Use a periodic-review `(s, S)` policy with item-location-specific review cadence. Define:

```text
inventory_position = on_hand + on_order - backorder - reserved
lower_threshold = safety_stock or reorder_point
upper_threshold = order_up_to_level
```

At a review, if `inventory_position <= reorder_point`, create a proposal. Planner delay and approval probability depend on planner workload. The order quantity is:

```text
q = round_to_lot(max(min_order_quantity, order_up_to_level - inventory_position))
```

Supplier lead time follows a positive distribution such as lognormal or gamma. It is modified by supplier reliability, congestion, injected delay episodes, and expedite actions. Goods receipt quantity follows the supplier fill rate and can be rejected with a small probability.

### 3.6 Stock-conservation invariant

For every item-location and event `e`:

```text
on_hand_after[e]
  = on_hand_before[e]
  + accepted_receipts[e]
  + transfer_receipts[e]
  + positive_adjustments[e]
  - goods_issues[e]
  - transfer_shipments[e]
  - negative_adjustments[e]
```

The validator recomputes this equation from event attributes and fails the run on any unexplained difference larger than the configured numerical tolerance.

### 3.7 Manual state notion

The independent reference state uses the post-event snapshot:

```text
Unknown
  if required stock or threshold data are unavailable
Critical Understock
  if confirmed demand within the decision horizon exceeds usable stock plus timely inbound
Understock
  if on_hand_after < lower_threshold
Overstock
  if on_hand_after > upper_threshold
Normal
  otherwise
```

Critical Understock has precedence over Understock. In the basic three-state analysis, merge Critical Understock into Understock after validating the five-state definition.

The FLOWVAULT query is stored in `queries/inventory_state.sql`. It must be applied to the clean observed log and compared event by event with the independently computed reference file.

### 3.8 Hidden execution regimes for automatic state detection

The simulation records a primary regime and a set of contributing factors. The observed log does not contain these labels.

| Primary regime | Defining mechanism | Expected evidence in observed features |
|---|---|---|
| Nominal Replenishment | stable demand and timely replenishment | ordinary sales, proposals, purchase orders, receipts |
| Stable Low Movement | few sales and no urgent supply activity | low activity counts and stable stock |
| Demand Surge Without Inbound | abrupt demand increase and insufficient timely inbound | many sales/issues, low stock, few open PO objects |
| Supplier Delay | open inbound exceeds expected lead time | PO and delivery context, delay events, prolonged low stock |
| Replenishment In Transit | low stock with confirmed inbound due soon | PO and delivery objects, on-order quantity, confirmation events |
| Receipt-Driven Excess | a large or duplicated receipt causes Overstock | receipt events, sharp stock increase, high on-hand |
| Forecast or Policy Bias | thresholds or order-up-to levels are systematically too high or low | policy updates, recurring overstock or understock |
| Transfer Recovery | stock is restored through another location | transfer request, ship, and receive context |
| Count Discrepancy | cycle count reveals a stock mismatch | count and adjustment events |
| Data Gap | missing snapshots or delayed updates | data-gap events and `data_complete=false` |

Use a deterministic precedence order when several mechanisms overlap, and retain all active mechanisms in `regime_factors_json`.

### 3.9 Forced mechanisms and pattern families

Random simulation alone may fail to produce enough repeated patterns. Schedule a configured minimum number of forced episodes while keeping their exact objects and dates randomized.

| Pattern ID | Mechanism | Canonical boundary behavior |
|---|---|---|
| INV-P1 | demand surge into shortage | Sales Order Item Created, Reservation Created, Goods Issue, Backorder Registered, Replenishment Proposal Created |
| INV-P2 | supplier-delay persistence | Purchase Order Item Created, Supplier Confirmation Received, Delivery Delayed, Expedite Requested, Goods Receipt |
| INV-P3 | receipt-driven overstock | Goods Receipt, Storage or Capacity Alert represented by Policy Threshold Updated or a dedicated event, Transfer Requested |
| INV-P4 | transfer recovery | Transfer Requested, Transfer Ship on source, Transfer Receive on target, target returns to Normal |
| INV-P5 | policy failure | Critical Understock occurs and no proposal or purchase-order action appears within the required SLA |
| INV-P6 | count discrepancy | Cycle Count Performed, Inventory Adjustment, abrupt state correction |

The golden profile generates exact canonical sequences with no unrelated event inside the pattern radius. The paper profile adds configurable noise events and timing variability.

### 3.10 Required inventory analytics

#### A. Import and semantic validation

Report OCEL schema validity, summary counts, leading-object lifecycle coverage, relationship density, branch coverage, and all stock-conservation errors. Verify that every event references existing objects and that every event related to `ItemLocation` has exactly one such relationship.

#### B. Expression-based state agreement

Compare FLOWVAULT-exported event states with `state_at_event.csv`.

Required metrics:

```text
assignment coverage
exact accuracy
macro and weighted F1
per-state precision and recall
Unknown exposure
CASE branch coverage
transition precision and recall within a configurable time tolerance
transition-time median absolute error and 95th percentile
short-episode or chattering rate
state-episode temporal intersection over union
```

The golden fixture must achieve 100 percent event-state agreement and exact transition timestamps.

#### C. State occupancy, dwell, and recovery

Use `stateTransitionKpisJson` and an independent Python implementation. Compare:

- state and transition counts;
- episode counts and durations;
- time to Understock recovery, defined as Understock or Critical Understock to Normal;
- repeated Understock within 7, 14, and 30 days;
- stuck item-locations at the observation horizon;
- cohort differences by supplier, location, material class, and policy version.

The independent implementation is the oracle for API tests.

#### D. SA-OCDFG utility

Compute ordinary OCDFG and SA-OCDFG outputs on the same log. In addition to visual inspection, calculate:

```text
state-conditioned edge frequency
state-conditioned median waiting time
edge-state entropy
weighted Jensen-Shannon divergence between P(next activity | activity, state)
and P(next activity | activity)
mutual information I(state; next activity | current activity)
```

The generator includes matched cohorts with the same activity pair but different stock states and outcomes. The ordinary graph should merge them, while the SA-OCDFG should expose distinct state-labeled nodes or change nodes.

#### E. Pattern recovery

For every injected pattern, score:

```text
top-k retrieval
rank of the first matching pattern
support absolute and relative error
sequence exact match or normalized edit similarity
context object-type Jaccard similarity
occurrence precision and recall
```

Run pattern analytics on `observed.behavior.ocel.json` or use the radius-limited pattern API. Do not use passive snapshots in the exact-sequence pattern key.

#### F. Policy conformance

Evaluate at least these constraints:

1. Critical Understock is followed by a replenishment proposal within the configured SLA.
2. An approved proposal is converted to a purchase-order item within the planner SLA.
3. Goods Receipt does not precede Purchase Order Item Created.
4. Transfer Receive follows Transfer Ship for the same transfer order.
5. An order is not created above the policy reorder point unless an explicit exception reason exists.
6. Duplicate receipt or over-delivery is flagged when it causes receipt-driven excess.

Compare detected violations with `conformance_violations.csv` and report precision, recall, timing error, and rate by planner, location, and state.

#### G. Automatic state discovery

Label each lifecycle window in two ways:

```text
endpoint label = hidden regime at the end event
majority label = modal hidden regime over all events in the window
```

Mark windows containing a hidden-regime transition. Evaluate endpoint and majority labels separately.

Required metrics:

```text
cell occupancy and empty-cell rate
cell label entropy
purity after optimal Hungarian mapping
adjusted Rand index
normalized mutual information
balanced accuracy after mapping
quantization error
mean cell-run length per item-location
nearby SOM transition proportion
bootstrap or object-resample stability
period-to-period stability
transfer across material and location classes
early warning precision, recall, and median lead time before Understock
```

Cell explanations list the largest standardized feature differences from the rest, dominant activities, related-object counts, and representative entry and exit windows.

#### H. Prediction

Create decision samples only while the current policy state is Normal or Understock, depending on the task.

Tasks:

```text
Understock within 7 days
Critical Understock within 7 days
recovery to Normal within 3 days after shortage entry
time to stable Normal recovery
```

Feature-set ablations:

```text
static item and location attributes only
raw stock and demand summaries
process activity counts only
manual state plus dwell and transition history
automatic cell plus cell-transition history
object-context counts only
full model
```

Use temporal holdout plus grouped splits by item-location. Purge windows overlapping the prediction horizon. Report AUPRC, AUROC, Brier score, expected calibration error, recall at a fixed alert budget, median warning lead time, false alerts per item-location-month, and recovery-time MAE or concordance.

#### I. Optional causal validation

Randomize an expedite intervention for eligible Critical Understock episodes. Preserve the same exogenous demand and supplier random streams in paired treated and untreated runs. Estimate the effect on recovery time, shortage units, and total cost. Compare estimated signs and magnitudes with `causal_truth.json`. FLOWVAULT's current DAG workbench should be described as exploratory association support, not as the definitive causal estimator.

## 4. Scenario B: manufacturing and predictive maintenance

### 4.1 Objective

Create machine lifecycles in which operational state depends on mode, alarms, sensors, production context, maintenance, and quality. The scenario tests multi-signal state definitions, object-attribute updates, learned regimes, transitions into Down, recovery behavior, conformance, and predictive warning.

### 4.2 Object model

The leading object type is `Machine`.

| Object type | Role | Important attributes |
|---|---|---|
| Machine | leading lifecycle | machine family, age, criticality, site; dynamic mode, health index, sensors, latched degraded flag, maintenance and quality flags |
| ProductionOrder | production demand | product family, priority, quantity, due time |
| Operation | machine-specific execution step | operation type, planned duration, actual duration |
| WorkOrder | maintenance case | priority, fault family, creation and completion times, status |
| Component | replaceable asset part | component family, age, expected life, replacement cost |
| MaterialLot | production input | material grade, quality score, supplier |
| Alarm | alarm occurrence | category, severity, threshold, acknowledgement status |
| Inspection | quality or maintenance check | inspection type, result, measured value |
| Operator | human production context | skill band, shift, team |
| MaintenanceTeam | maintenance context | response-time profile, skill mix, workload |
| Shift | organizational and temporal context | shift name, staffing band |

Each process event is related to one machine, plus any relevant production order, operation, work order, component, alarm, inspection, operator, team, and material lot.

### 4.3 Event catalog

| Event | Main effect |
|---|---|
| Initialize Machine | establishes initial mode, health, components, and sensors |
| Production Order Released | creates production context |
| Setup Started / Completed | moves through Setup |
| Operation Started / Completed | starts and ends production |
| Sensor Snapshot | updates health-observation attributes |
| Warning Alarm Raised | records moderate deterioration |
| Critical Alarm Raised | records severe deterioration |
| Alarm Acknowledged | human response |
| Defect Detected | records a quality signal |
| Quality Hold Started / Released | blocks or releases production |
| Automatic Stop | moves to Down |
| Maintenance Request Created | requests intervention |
| Work Order Created | creates maintenance context |
| Maintenance Team Dispatched | response step |
| Maintenance Started | begins active repair |
| Diagnosis Performed | identifies or misidentifies fault |
| Part Unavailable | introduces waiting behavior |
| Component Replaced | restores component health |
| Repair Performed | partially restores health |
| Calibration Performed | corrects sensor or process drift |
| Inspection Performed | verifies safety or quality |
| Test Run Started / Completed | post-maintenance validation |
| Test Failed | causes rework or recurrence |
| Machine Restarted | enters Recovery |
| Maintenance Completed | closes work order after stable operation |
| Simulation End Snapshot | closes final state episode |

Common event attributes, declared for every machine-related event type:

```text
mode: string
health_index: float
vibration_rms: float
temperature_c: float
power_kw: float
load_fraction: float
alarm_severity: string
degraded_latched: boolean
down_active: boolean
recovery_active: boolean
quality_hold_active: boolean
maintenance_open: boolean
stable_run_minutes: integer
data_complete: boolean
passive_observation: boolean
fault_family_observed: string
```

The same dynamic values are written as time-indexed `Machine` object attributes. Hidden physical health components and true fault cause remain only in the truth sidecar.

### 4.4 Production process

Generate production orders with product families, quantities, priorities, and due dates. Assign machine-specific operations. A product-family change creates Setup events. Operation duration depends on quantity, nominal rate, machine health, operator skill, and material quality.

The probability of a quality defect increases with degradation, setup instability, poor material lots, and excessive load. Defect detections can trigger a Quality Hold and an inspection workflow.

### 4.5 Hidden degradation and sensor model

Represent total health as one observable summary plus one or more hidden component health values. For a component `c` at time step `t`:

```text
wear[c,t+1] = clip(
  wear[c,t]
  + base_wear_rate[c] * load[t]^alpha * age_factor[c] * material_factor[t]
  + shock[c,t]
  + process_noise[c,t],
  0, 1)

health_index[t] = 1 - weighted_sum(wear[c,t])
```

Sensor values are generated from mode, load, wear, ambient conditions, and measurement noise:

```text
vibration_rms = vibration_base[machine_family]
              + beta_bearing * bearing_wear
              + beta_load * load_fraction
              + noise

temperature_c = ambient
              + thermal_base
              + gamma_load * load_fraction
              + gamma_wear * thermal_wear
              + noise

power_kw = nominal_power * load_fraction * (1 + delta_wear * total_wear) + noise
```

Failure hazard per observation interval can be modeled with a logistic function of wear, overload, critical alarms, and recent defects. Forced faults guarantee that all required regimes and transitions occur.

### 4.6 Maintenance process

A warning or critical alarm may trigger acknowledgement, request creation, work-order creation, dispatch, active maintenance, diagnosis, part replacement or repair, inspection, test, restart, and completion. Maintenance-team workload controls response delay. Component availability controls waiting. Repair quality controls restored health and recurrence risk.

Inject conformance violations independently of physical faults so that detection can be evaluated:

- delayed maintenance after a critical alarm;
- restart before required inspection;
- missing test run;
- maintenance completion before stable recovery;
- quality-hold release after a failed inspection;
- incorrect component replacement or incomplete repair.

### 4.7 Manual state notion

Use this precedence order:

```text
Unknown
  if telemetry or required mode data are stale or incomplete
Down
  if mode is DOWN or a failure stop is active
Quality Hold
  if a quality hold is active and the machine is not Down
Recovery
  after restart or test run until a configured stable-run period has elapsed
Setup
  if mode is SETUP
Degraded
  if the latched degraded condition is active
Running
  if mode is RUNNING
Idle
  otherwise
```

The latched degraded condition is computed by an independent preprocessing state machine over observed sensors and alarms:

```text
enter Degraded if health_index <= 0.65
  or a critical alarm occurs
  or vibration/temperature exceeds the entry threshold for two samples

exit Degraded only if health_index >= 0.75
  and all sensor values are below exit thresholds for four samples
```

Recovery starts at restart or test-run start and ends only after the stable-run requirement is met without a new warning. The reference state is computed separately from the FLOWVAULT CASE query.

### 4.8 Hidden execution regimes

| Primary regime | Meaning |
|---|---|
| Healthy Steady Run | stable production with low wear |
| Setup or Changeover | product transition and setup actions |
| High-Load Wear | high load causing accelerated but not yet fault-specific wear |
| Bearing Degradation | increasing vibration caused by bearing wear |
| Thermal Drift | increasing temperature or calibration drift |
| Quality Drift | defect probability rises before a formal hold |
| Alarm Escalation | warning sequence approaching critical condition |
| Waiting for Maintenance | machine is Down or Degraded while waiting for response or parts |
| Active Repair | diagnosis, replacement, repair, or calibration in progress |
| Post-Repair Recovery | test and monitored return to operation |
| Failed | active failure and Down condition |
| Idle | available but not processing |
| Data Gap | missing or stale telemetry |

As in inventory, record a primary label plus all active factors.

### 4.9 Forced mechanisms and pattern families

| Pattern ID | Mechanism | Canonical behavior |
|---|---|---|
| MFG-P1 | bearing degradation into Down | Sensor Snapshot, Warning Alarm Raised, Maintenance Request Created, Critical Alarm Raised, Automatic Stop |
| MFG-P2 | thermal or quality deterioration | Sensor Snapshot, Defect Detected, Quality Hold Started, Inspection Performed, Calibration Performed |
| MFG-P3 | quick recovery | Maintenance Started, Diagnosis Performed, Component Replaced or Repair Performed, Inspection Performed, Test Run Completed, Machine Restarted |
| MFG-P4 | slow recovery | Maintenance Started, Diagnosis Performed, Part Unavailable, Component Replaced, Test Failed, Repair Performed, Test Run Completed, Machine Restarted |
| MFG-P5 | recurrent degradation | Machine Restarted, short Recovery, Running, Warning Alarm Raised or Degraded within the recurrence horizon |
| MFG-P6 | unsafe restart | Critical Alarm or Down, Machine Restarted without the required inspection or passed test |

The golden fixture fixes exact sequences. The paper profile adds realistic optional steps, delays, and repeated diagnosis or test loops.

### 4.10 Required manufacturing analytics

#### A. Import, physical, and workflow validation

Validate OCEL structure and relationship references. Recompute sensor-model ranges, health bounds, component replacement effects, and event-order constraints. Verify one machine per event and the presence of initialization and end snapshots.

#### B. Manual state agreement

Use the same state-agreement measures as inventory, plus:

```text
transition boundary error in multiples of telemetry cadence
chattering transitions per machine-day
fraction of state episodes shorter than the minimum-duration rule
agreement with a deliberately noisy operational mode record
```

The clean reference and noisy operational label must be stored separately.

#### C. Transition and recovery analysis

Required results:

```text
Running or Degraded to Down counts and durations
Down to Recovery and Recovery to Running durations
quick versus slow recovery cohorts
Down recurrence and Degraded recurrence after maintenance
stuck Down machines at the horizon
response and repair duration by maintenance team, machine family, component, and shift
```

The current FLOWVAULT recovery helper recognizes state names containing Normal, Available, or Standard. For this scenario, either compute recovery externally or extend the request with explicit recovery target states. Do not rename the paper's Running state merely to satisfy the current helper.

#### D. SA-OCDFG and pattern analysis

Compare ordinary and state-aware graphs for paths into Down, during Down, and through Recovery. Quantify state-conditioned waiting times and edge divergence. Score the six injected pattern families using top-k retrieval, support error, sequence similarity, context similarity, and occurrence recall.

Passive `Sensor Snapshot` events should be ignored in behavior-pattern keys or analyzed at a bounded radius.

#### E. Conformance

Evaluate at least:

1. Critical Alarm Raised to Maintenance Request Created within the alarm SLA.
2. Down to Maintenance Started within the machine-criticality SLA.
3. Machine Restarted occurs only after a passed Inspection or Test Run Completed.
4. Maintenance Completed occurs only after the stable-run requirement.
5. Quality Hold Released occurs only after a passed quality inspection.
6. Component Replaced is linked to an open WorkOrder and the relevant Machine.

Compare with injected truth and report precision, recall, delay, and violation rates by team and state.

#### F. Automatic state discovery

Evaluate window cells against hidden regimes using purity, ARI, NMI, balanced accuracy after mapping, cell entropy, quantization error, topology continuity, and stability. Additional transfer tests:

```text
train or fit on selected machine families, map held-out families
fit on early months, evaluate later months
fit on one site, evaluate another site
```

Report whether risk cells appear before Degraded or Down, with warning precision, recall, median lead time, and false risk-cell entries per machine-week.

#### G. Prediction

Primary tasks:

```text
Down within 4 hours
Down within 24 hours
time to Down
stable Running recovery within 8 hours after restart
time to stable recovery
recurrent Degraded or Down within 24 hours after maintenance
```

Feature-set ablations:

```text
static machine and component attributes
raw sensor summaries only
process activity counts only
manual state, dwell, and recent transitions
learned SOM cell and recent cell transitions
object context such as work order, component, material lot, team, and shift
full model
```

Use temporal holdout and grouped machine-family or machine splits. Purge overlapping windows around the split. For alerts, count at most one true alert per failure episode. Report AUPRC, AUROC, Brier score, calibration error, median lead time, false alerts per machine-week, event-based sensitivity, recovery-time MAE, and concordance where appropriate.

#### H. Maintenance-effect causal check

Randomize dispatch priority or preventive maintenance eligibility for a subset of otherwise comparable machines. Use shared exogenous production and sensor-noise streams in paired runs. Estimate effects on failure probability, Down duration, quality defects, and maintenance cost. Store true intervention parameters and paired potential outcomes in `causal_truth.json`.

## 5. Common robustness design

Run a pre-specified perturbation matrix. Every run records a perturbation ID and preserves the clean-run manifest link.

### 5.1 Inventory perturbations

```text
lower and upper thresholds: -20%, -10%, baseline, +10%, +20%
policy-version lag: 0, 1, 3, 7 days
event deletion: 0%, 1%, 5%, 10%
relationship deletion: 0%, 1%, 5%
attribute missingness: MCAR and block missingness at 1%, 5%, 10%, 20%
timestamp jitter: 0, 5 minutes, 60 minutes
stock-adjustment omission: 0%, 25%, 50%
window size: 3, 5, 8, 12 events
SOM grid: 3x3, 5x5, 7x7
pattern radius: 1, 2, 3, full episode
```

### 5.2 Manufacturing perturbations

```text
sensor noise multiplier: 0.5, 1.0, 2.0
telemetry cadence: 5, 15, 30, 60 minutes
telemetry block missingness: 0, 1, 4, 12 hours
alarm-recording delay: 0, 5, 30 minutes
process-event deletion: 0%, 1%, 5%, 10%
relationship deletion: 0%, 1%, 5%
timestamp jitter: 0, 1, 15 minutes
Degraded thresholds: +/-5% and +/-10%
window size: 4, 8, 16, 32 events
SOM grid: 3x3, 5x5, 7x7
pattern radius: 1, 2, 3, full episode
```

### 5.3 Robustness outputs

For each perturbation, calculate:

```text
state coverage and macro-F1 change
transition-time drift
state occupancy change
rank correlation of top state-conditioned edges
Jaccard overlap of top-k patterns
pattern support change
SOM NMI or variation of information after alignment
prediction metric change
runtime and peak-memory change
```

Classify a finding as stable, conditionally stable, or sensitive according to pre-registered tolerances. Do not hide sensitivity by averaging across perturbations.

## 6. Analyst-task datasets

Generate task cases and answer keys from truth sidecars.

### Inventory tasks

1. Identify the dominant cause of a selected Understock entry.
2. Distinguish receipt-driven Overstock from low-demand or policy-driven Overstock.
3. Find a Critical Understock episode with no timely replenishment action.
4. Compare recovery performance for two suppliers or locations.

### Manufacturing tasks

1. Identify the dominant path from Degraded to Down for a selected machine.
2. Explain why one recovery was slower than another.
3. Find a machine that returned to Degraded shortly after repair.
4. Verify whether restart and quality-hold policies were followed.

Each task JSON should include the object cohort, time range, correct answer, required evidence items, acceptable alternative explanations, and a scoring rubric. Baseline views use ordinary lifecycles, OCDFGs, and conventional KPIs. Treatment views add state calendars, SA-OCDFGs, transition KPIs, patterns, and boundary windows.

## 7. Test strategy

### 7.1 Profiles

| Profile | Purpose | Properties |
|---|---|---|
| golden | exact unit and integration oracle | tiny, no noise, fixed event counts, exact patterns and states |
| smoke | continuous integration and UI testing | all objects, states, transitions, and violations represented |
| paper | statistical evaluation | realistic heterogeneity, noise, missingness, and sufficient support |
| scale | performance benchmark | geometric event and object counts, minimal expensive sidecars if needed |

### 7.2 Generator unit tests

Test demand distributions, policy calculations, stock conservation, sensor equations, health bounds, hysteresis, recovery logic, event ordering, ID uniqueness, relationship validity, and deterministic substreams.

### 7.3 Golden tests

For each scenario, hard-code a small manifest containing expected:

```text
object and event-type counts
event and object counts
E2O and O2O relationship counts
state branch counts
state and transition sequences per leading object
state episode durations
injected pattern supports
conformance violations
prediction labels
SHA-256 checksum for the canonical observed log
```

Same seed and configuration must reproduce the checksum. A different seed must change event content while preserving schema and required branch coverage.

### 7.4 FLOWVAULT Rust integration tests

Add generated golden fixtures under `files/ocel2/evaluation/` and test:

1. import and exact summary counts;
2. JSON, XML, CSV, SQLite, and bundle round trips for one fixture;
3. state-query assignment count and exact exported state labels;
4. SA-OCDFG contains expected `CHANGE A -> B` nodes;
5. transition KPI rows equal the independent oracle;
6. pattern summary contains the canonical pattern and support;
7. state detection returns configured dimensions and exact window count;
8. full assignment export maps every expected window;
9. filter then reapply state query produces expected behavior-log patterns;
10. missingness fixtures yield Unknown rather than silently defaulting.

### 7.5 UI end-to-end tests

Use Playwright against a production build:

```text
upload or select smoke fixture
apply named state query
open SA-OCDFG and assert state-change node text
open transition KPI page and assert target transition row
open Patterns and select a known pattern
run State Detection and open a populated cell
export enriched JSON and verify state attribute exists
```

Use data attributes rather than CSS layout selectors for stable tests.

### 7.6 Analytics tests

Use hand-constructed micro-data for state agreement, transition matching, episode IoU, Hungarian cell mapping, pattern edit similarity, conformance detection, prediction labeling, temporal purging, and alert-event scoring.

## 8. Small FLOWVAULT extensions needed for rigorous evaluation

The current implementation is sufficient for interactive demonstration but needs the following batch-oriented extensions.

### 8.1 Full state-detection assignment export

The current JSON view limits projected windows. Add an endpoint returning every window:

```json
{
  "object_type": "Machine",
  "window_size": 8,
  "windows": [
    {
      "object_id": "M-001",
      "start_event": "e-10",
      "end_event": "e-17",
      "cell_x": 2,
      "cell_y": 1,
      "pc1": 1.2,
      "pc2": -0.4
    }
  ]
}
```

CSV export is also useful. This is required for ARI, NMI, lead-time, and stability calculations.

### 8.2 Configurable pattern extraction

Change `statePatternsJson()` to accept an optional request:

```json
{
  "leading_object_type": "ItemLocation",
  "family": "both",
  "pre_radius": 3,
  "post_radius": 3,
  "ignored_event_types": ["Inventory Snapshot", "Sensor Snapshot"],
  "min_support": 2,
  "include_occurrences": true
}
```

Return occurrence object IDs, start and end events, and timestamps. Keep the existing no-argument behavior as the full-episode default.

### 8.3 Explicit recovery targets

Extend transition KPI requests with:

```json
{
  "object_type": "Machine",
  "recovery_transitions": [
    ["Down", "Recovery"],
    ["Recovery", "Running"]
  ]
}
```

Do not infer all recovery semantics from substrings in state names.

### 8.4 Headless batch runner

Add a small Rust CLI around `OcelDocumentCore` so CI and paper scripts do not require a browser. Suggested commands:

```text
summary
apply-state-query
state-transition-kpis
ocdfg
sa-ocdfg
state-patterns
state-detection
state-detection-assignments
export
```

Each command writes deterministic JSON and optionally timing and peak-memory metadata.

### 8.5 Optional later extension: incidence-specific states

The formal framework can associate state with a leading object incidence. The current implementation stores one event-level state. The proposed generators avoid ambiguous multi-leading-object events. A later extension can store `(event_id, leading_object_id, state)` internally and materialize a selected perspective for export.

## 9. Performance benchmark

Use geometric scales and record actual counts after generation. Benchmark separate dimensions rather than only file size:

```text
events
objects
leading objects
average lifecycle length
E2O density
O2O density
number of object attributes and updates
state count
transition frequency
feature dimension
pattern diversity
```

Measure:

```text
import time
peak resident memory
state-query time
state-detection feature time
PCA/SOM time
state-application time
OCDFG and SA-OCDFG time
transition KPI time
pattern time
export time
browser interaction latency where applicable
```

Record hardware, operating system, browser, Rust profile, compiler version, WASM build mode, application commit, generator commit, and configuration hash.

## 10. Recommended implementation order

1. Implement the common OCEL builder, truth writers, deterministic IDs, and validators.
2. Implement the inventory golden generator and its independent state reference.
3. Add FLOWVAULT integration tests for inventory.
4. Implement inventory paper and robustness profiles.
5. Implement the manufacturing golden generator, physical model, hysteresis, and recovery logic.
6. Add manufacturing integration tests.
7. Add full SOM assignment export, configurable patterns, and recovery targets.
8. Implement common analytics and prediction labeling.
9. Add Playwright tasks and the headless runner.
10. Run paper and performance configurations only after all golden and smoke tests pass.
