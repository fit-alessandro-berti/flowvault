#[derive(Serialize)]
struct PatternAnalysis {
    intra: Vec<PatternSummary>,
    inter: Vec<PatternSummary>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct PatternSummary {
    id: String,
    family: &'static str,
    label: String,
    leading_object_type: String,
    state: Option<String>,
    from_state: Option<String>,
    to_state: Option<String>,
    support: usize,
    mass: usize,
    sequence: Vec<String>,
    object_types: Vec<String>,
    df_edges: Vec<PatternEdge>,
    eo_edges: Vec<PatternEdge>,
    oo_edges: Vec<PatternEdge>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct PatternEdge {
    source: String,
    target: String,
    weight: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionResult {
    object_type: String,
    window_size: usize,
    som_width: usize,
    som_height: usize,
    color_attribute: String,
    color_attributes: Vec<StateDetectionColorOption>,
    object_count: usize,
    feature_count: usize,
    window_count: usize,
    feature_columns: Vec<String>,
    table_preview: Vec<FeaturePreviewRow>,
    pca: PcaSummary,
    som: SomSummary,
    windows: Vec<StateWindowProjection>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionColorOption {
    id: String,
    label: String,
    kind: &'static str,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct FeaturePreviewRow {
    object_id: String,
    values: Vec<f64>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct PcaSummary {
    pc1_variance: f64,
    pc2_variance: f64,
    pc1_explained_ratio: f64,
    pc2_explained_ratio: f64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct SomSummary {
    cells: Vec<SomCellSummary>,
    transitions: Vec<SomTransitionSummary>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct SomCellSummary {
    x: usize,
    y: usize,
    label: String,
    count: usize,
    color_value: f64,
    color_label: String,
    color_kind: String,
    avg_pc1: f64,
    avg_pc2: f64,
    dominant_activity: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct SomTransitionSummary {
    source_x: usize,
    source_y: usize,
    target_x: usize,
    target_y: usize,
    count: usize,
    distance: usize,
    nearby: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateWindowProjection {
    object_id: String,
    start_event: String,
    end_event: String,
    pc1: f64,
    pc2: f64,
    cell_x: usize,
    cell_y: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionCellDetail {
    cell: SomCellSummary,
    dfg: LayoutGraph,
    entering_dfg: LayoutGraph,
    exiting_dfg: LayoutGraph,
    entering_window_count: usize,
    exiting_window_count: usize,
    entering_windows: Vec<StateDetectionBoundaryWindow>,
    exiting_windows: Vec<StateDetectionBoundaryWindow>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionBoundaryWindow {
    object_id: String,
    start_event: String,
    end_event: String,
    source_cell: String,
    target_cell: String,
    pc1: f64,
    pc2: f64,
    activities: Vec<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct CausalFeatureTableResult {
    object_type: String,
    object_count: usize,
    feature_count: usize,
    feature_columns: Vec<String>,
    table_preview: Vec<FeaturePreviewRow>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct TimePerspectiveResult {
    object_type: String,
    event_min_ms: i64,
    event_max_ms: i64,
    states: Vec<String>,
    buckets: Vec<TimeFrequencyBucket>,
    performance: TimePerformanceSpectrum,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct TimeFrequencyBucket {
    start_ms: i64,
    end_ms: i64,
    total: usize,
    percentages: Vec<TimeStatePercentage>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct TimeStatePercentage {
    state: String,
    percentage: f64,
    count: usize,
}
