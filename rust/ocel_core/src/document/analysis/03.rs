
#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateCorrelationResult {
    object_type: String,
    object_count: usize,
    stateful_object_count: usize,
    state_count: usize,
    feature_count: usize,
    state_distribution: Vec<StateCorrelationStateCount>,
    rows: Vec<StateCorrelationRow>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateCorrelationStateCount {
    state: String,
    count: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateCorrelationRow {
    feature: String,
    state: String,
    correlation: f64,
    strength: f64,
    sample_count: usize,
    state_count: usize,
    mean_in_state: f64,
    mean_outside_state: f64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct CausalModelFitResult {
    object_type: String,
    sample_count: usize,
    nodes: Vec<CausalFitNode>,
    edges: Vec<CausalFitEdge>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct CausalFitNode {
    id: String,
    label: String,
    role: String,
    feature: Option<String>,
    operation: String,
    mean: f64,
    std_dev: f64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct CausalFitEdge {
    source: String,
    target: String,
    correlation: f64,
    intensity: f64,
    p_value: f64,
    sample_count: usize,
}

struct FeatureTableData {
    columns: Vec<String>,
    rows: Vec<FeatureRow>,
}

struct FeatureRow {
    object_id: String,
    values: Vec<f64>,
}

#[derive(Clone)]
enum FeatureColumn {
    Activity { event_type: String },
    RelatedObjectType { object_type: String },
    NumericAttribute { name: String },
    CategoricalAttribute { name: String, value: String },
}

impl FeatureColumn {
    fn label(&self) -> String {
        match self {
            Self::Activity { event_type } => format!("activity.{event_type}"),
            Self::RelatedObjectType { object_type } => format!("related_objects.{object_type}"),
            Self::NumericAttribute { name } => format!("attribute.{name}"),
            Self::CategoricalAttribute { name, value } => {
                format!("attribute.{name}={value}")
            }
        }
    }
}

#[derive(Default)]
struct AttributeFeatureCollector {
    has_numeric: bool,
    categories: BTreeSet<String>,
}

struct FeatureEncoder {
    columns: Vec<FeatureColumn>,
}

struct WindowEncoding {
    object_index: usize,
    event_indices: Vec<usize>,
    values: Vec<f64>,
}

struct PcaProjection {
    points: Vec<(f64, f64)>,
    pc1_variance: f64,
    pc2_variance: f64,
    pc1_explained_ratio: f64,
    pc2_explained_ratio: f64,
}

struct SomModel {
    width: usize,
    assignments: Vec<(usize, usize)>,
    weights: Vec<(f64, f64)>,
}

struct StateDetectionRun {
    object_indices: Vec<usize>,
    encoder: FeatureEncoder,
    feature_table: FeatureTableData,
    windows: Vec<WindowEncoding>,
    pca: PcaProjection,
    som: SomModel,
    color_metric: ColorMetric,
    color_options: Vec<StateDetectionColorOption>,
}

struct StateDetectionStateAssignments {
    leading_object_type: String,
    states: Vec<StateDetectionEventState>,
}

struct StateDetectionEventState {
    event_id: Symbol,
    state: String,
}

#[derive(Clone, Default)]
struct StateDetectionEventVote {
    counts: BTreeMap<(usize, usize), usize>,
    latest_window_index: BTreeMap<(usize, usize), usize>,
}

impl StateDetectionEventVote {
    fn add(&mut self, cell: (usize, usize), window_index: usize) {
        *self.counts.entry(cell).or_default() += 1;
        self.latest_window_index.insert(cell, window_index);
    }

    fn winning_cell(&self) -> Option<(usize, usize)> {
        let mut best = None;

        for (cell, count) in &self.counts {
            let latest_window_index = self.latest_window_index.get(cell).copied().unwrap_or(0);
            match best {
                None => best = Some((*cell, *count, latest_window_index)),
                Some((best_cell, best_count, best_latest_window_index)) => {
                    if *count > best_count
                        || (*count == best_count
                            && (latest_window_index > best_latest_window_index
                                || (latest_window_index == best_latest_window_index
                                    && *cell < best_cell)))
                    {
                        best = Some((*cell, *count, latest_window_index));
                    }
                }
            }
        }

        best.map(|(cell, _, _)| cell)
    }
}

#[derive(Clone)]
enum ColorMetric {
    WindowCount,
    NumericAttribute(String),
    CategoricalAttribute(String),
}
