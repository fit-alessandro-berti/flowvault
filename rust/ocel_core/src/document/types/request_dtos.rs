
#[derive(Clone, Deserialize)]
struct OcelFilterRequest {
    #[serde(default)]
    event_types: Vec<String>,
    #[serde(default)]
    object_types: Vec<String>,
    #[serde(default)]
    time_range: Option<TimeRangeFilter>,
    #[serde(default)]
    df_nodes: Vec<String>,
    #[serde(default)]
    df_edges: Vec<DfEdgeFilter>,
    #[serde(default)]
    text_attributes: Vec<TextAttributeFilter>,
    #[serde(default)]
    patterns: Vec<PatternFilter>,
}

#[derive(Clone, Deserialize)]
struct TimeRangeFilter {
    #[serde(default)]
    start_ms: Option<i64>,
    #[serde(default)]
    end_ms: Option<i64>,
}

#[derive(Clone, Deserialize)]
struct DfEdgeFilter {
    source: String,
    target: String,
}

#[derive(Clone, Deserialize)]
struct TextAttributeFilter {
    #[serde(default = "default_text_attribute_scope")]
    scope: String,
    name: String,
    #[serde(default)]
    values: Vec<String>,
}

#[derive(Clone, Deserialize)]
struct PatternFilter {
    #[serde(default)]
    family: String,
    #[serde(default)]
    leading_object_type: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    from_state: Option<String>,
    #[serde(default)]
    to_state: Option<String>,
    #[serde(default)]
    sequence: Vec<String>,
    #[serde(default)]
    eo_edges: Vec<PatternEdgeFilter>,
    #[serde(default)]
    oo_edges: Vec<PatternEdgeFilter>,
}

#[derive(Clone, Deserialize)]
struct PatternEdgeFilter {
    source: String,
    target: String,
}

#[derive(Serialize)]
struct FilterOptions {
    event_types: Vec<String>,
    object_types: Vec<String>,
    text_attributes: Vec<TextAttributeOption>,
    time_min_ms: Option<i64>,
    time_max_ms: Option<i64>,
    time_buckets: Vec<FilterTimeBucket>,
}

#[derive(Serialize)]
struct FilterTimeBucket {
    start_ms: i64,
    end_ms: i64,
    count: usize,
}

#[derive(Serialize)]
struct TextAttributeOption {
    scope: String,
    name: String,
    values: Vec<String>,
}

#[derive(Default, Deserialize)]
struct GraphFilterRequest {
    object_types: Option<Vec<String>>,
    min_activity_frequency: Option<usize>,
    min_path_frequency: Option<usize>,
}

#[derive(Deserialize)]
struct StateDetectionRequest {
    object_type: String,
    window_size: Option<usize>,
    som_width: Option<usize>,
    som_height: Option<usize>,
    epochs: Option<usize>,
    max_training_windows: Option<usize>,
    color_attribute: Option<String>,
}

#[derive(Deserialize)]
struct StatePatternRequest {
    #[serde(default)]
    leading_object_type: Option<String>,
    #[serde(default = "default_pattern_family")]
    family: String,
    #[serde(default)]
    pre_radius: Option<usize>,
    #[serde(default)]
    post_radius: Option<usize>,
    #[serde(default)]
    ignored_event_types: Vec<String>,
    #[serde(default = "default_pattern_min_support")]
    min_support: usize,
    #[serde(default)]
    include_occurrences: bool,
}

impl Default for StatePatternRequest {
    fn default() -> Self {
        Self {
            leading_object_type: None,
            family: default_pattern_family(),
            pre_radius: None,
            post_radius: None,
            ignored_event_types: Vec::new(),
            min_support: default_pattern_min_support(),
            include_occurrences: false,
        }
    }
}

fn default_pattern_family() -> String {
    "both".to_owned()
}

fn default_pattern_min_support() -> usize {
    1
}

#[derive(Deserialize)]
struct StateDetectionCellRequest {
    object_type: String,
    window_size: Option<usize>,
    som_width: Option<usize>,
    som_height: Option<usize>,
    epochs: Option<usize>,
    max_training_windows: Option<usize>,
    color_attribute: Option<String>,
    cell_x: usize,
    cell_y: usize,
}

#[derive(Deserialize)]
struct CausalFeatureTableRequest {
    object_type: String,
}

#[derive(Deserialize)]
struct TimePerspectiveRequest {
    #[serde(default)]
    object_type: Option<String>,
    #[serde(default)]
    from_state: Option<String>,
    #[serde(default)]
    to_state: Option<String>,
    #[serde(default)]
    roundtrip: bool,
    #[serde(default)]
    buckets: Option<usize>,
}

#[derive(Deserialize)]
struct StateTransitionKpiRequest {
    #[serde(default)]
    object_type: Option<String>,
    #[serde(default)]
    stuck_limit: Option<usize>,
    #[serde(default)]
    recovery_transitions: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct ObjectSearchRequest {
    #[serde(default)]
    object_type: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct CausalModelFitRequest {
    object_type: String,
    #[serde(default)]
    nodes: Vec<CausalModelNodeRequest>,
    #[serde(default)]
    edges: Vec<CausalModelEdgeRequest>,
}

#[derive(Clone, Deserialize)]
struct CausalModelNodeRequest {
    id: String,
    label: String,
    role: String,
    #[serde(default)]
    feature: Option<String>,
    #[serde(default)]
    operation: String,
}

#[derive(Clone, Deserialize)]
struct CausalModelEdgeRequest {
    source: String,
    target: String,
}

#[derive(Default)]
struct GraphLayoutFilter {
    min_activity_frequency: usize,
    min_path_frequency: usize,
}
