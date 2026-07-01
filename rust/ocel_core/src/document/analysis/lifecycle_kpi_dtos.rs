
#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct TimePerformanceSpectrum {
    object_type: String,
    from_state: String,
    to_state: String,
    roundtrip: bool,
    sample_count: usize,
    min_duration_ms: Option<i64>,
    median_duration_ms: Option<i64>,
    avg_duration_ms: Option<f64>,
    max_duration_ms: Option<i64>,
    samples: Vec<TimePerformanceSample>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct TimePerformanceSample {
    object_id: String,
    start_time_ms: i64,
    middle_time_ms: i64,
    end_time_ms: Option<i64>,
    duration_ms: i64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateTransitionKpiResult {
    object_type: String,
    object_count: usize,
    stateful_object_count: usize,
    state_count: usize,
    states: Vec<String>,
    transitions: Vec<StateTransitionKpiRow>,
    dwell: Vec<StateDwellKpiRow>,
    recovery: Vec<StateTransitionKpiRow>,
    stuck: Vec<StuckStateRow>,
}

#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateTransitionKpiRow {
    from_state: String,
    to_state: String,
    count: usize,
    object_count: usize,
    min_duration_ms: Option<i64>,
    median_duration_ms: Option<i64>,
    avg_duration_ms: Option<f64>,
    max_duration_ms: Option<i64>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDwellKpiRow {
    state: String,
    episode_count: usize,
    object_count: usize,
    total_duration_ms: i64,
    min_duration_ms: Option<i64>,
    median_duration_ms: Option<i64>,
    avg_duration_ms: Option<f64>,
    max_duration_ms: Option<i64>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StuckStateRow {
    object_id: String,
    state: String,
    entered_time_ms: i64,
    last_time_ms: i64,
    duration_ms: i64,
    event_count: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct ObjectSearchResult {
    objects: Vec<ObjectSearchHit>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct ObjectSearchHit {
    object_id: String,
    object_type: String,
    event_count: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct ObjectLifecycleDetail {
    object_id: String,
    object_type: String,
    event_count: usize,
    event_min_ms: Option<i64>,
    event_max_ms: Option<i64>,
    events: Vec<LifecycleEventDetail>,
    state_bands: Vec<LifecycleStateBand>,
    stock_points: Vec<LifecycleStockPoint>,
    related_objects: Vec<LifecycleRelatedObjectSummary>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LifecycleEventDetail {
    event_id: String,
    event_type: String,
    time_ms: i64,
    state: Option<String>,
    attributes: Vec<LifecycleAttribute>,
    related_objects: Vec<LifecycleRelatedObject>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LifecycleAttribute {
    name: String,
    value: Value,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LifecycleRelatedObject {
    object_id: String,
    object_type: String,
    qualifier: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LifecycleRelatedObjectSummary {
    object_id: String,
    object_type: String,
    qualifier: String,
    event_count: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LifecycleStateBand {
    state: String,
    start_time_ms: i64,
    end_time_ms: i64,
    event_count: usize,
    start_event_id: String,
    end_event_id: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LifecycleStockPoint {
    name: String,
    time_ms: i64,
    value: f64,
    event_id: String,
}

#[derive(Default)]
struct TransitionAccumulator {
    durations: Vec<i64>,
}

struct DurationStats {
    sample_count: usize,
    min_duration_ms: Option<i64>,
    median_duration_ms: Option<i64>,
    avg_duration_ms: Option<f64>,
    max_duration_ms: Option<i64>,
}

struct LifecycleRelatedObjectAccumulator {
    object_type: String,
    qualifier: String,
    event_count: usize,
}
