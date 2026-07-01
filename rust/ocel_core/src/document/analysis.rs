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

impl ColorMetric {
    fn id(&self) -> String {
        match self {
            Self::WindowCount => "__window_count".to_owned(),
            Self::NumericAttribute(name) | Self::CategoricalAttribute(name) => {
                format!("attribute::{name}")
            }
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutGraph {
    title: String,
    subtitle: String,
    width: f64,
    height: f64,
    nodes: Vec<LayoutNode>,
    edges: Vec<LayoutEdge>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutNode {
    id: String,
    label: String,
    kind: String,
    shape: String,
    color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    object_type: Option<String>,
    count: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    lines: Vec<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutEdge {
    id: String,
    source: String,
    target: String,
    kind: String,
    path: String,
    label: String,
    title: String,
    weight: usize,
    object_type: String,
    color: String,
    directed: bool,
    points: Vec<LayoutPoint>,
    label_x: f64,
    label_y: f64,
    object_types: Vec<WeightedObjectType>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutPoint {
    x: f64,
    y: f64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct WeightedObjectType {
    object_type: String,
    weight: usize,
}

struct GraphAccumulator {
    title: String,
    subtitle: String,
    nodes: BTreeMap<String, GraphNodeAccumulator>,
    edges: BTreeMap<(String, String, String), GraphEdgeAccumulator>,
    object_type_colors: BTreeMap<String, String>,
}

struct GraphNodeAccumulator {
    label: String,
    kind: String,
    shape: String,
    color: String,
    object_type: Option<String>,
    count: usize,
    order_sum: f64,
    order_count: usize,
}

struct GraphEdgeAccumulator {
    source: String,
    target: String,
    object_type: String,
    color: String,
    weight: usize,
}

impl GraphAccumulator {
    fn new(title: String, subtitle: String) -> Self {
        Self {
            title,
            subtitle,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            object_type_colors: BTreeMap::new(),
        }
    }

    fn add_node(&mut self, label: &str, kind: &str, order: f64, count: usize) {
        self.add_node_with_style(label, kind, "rect", "#42635c", None, order, count);
    }

    fn add_object_boundary_node(
        &mut self,
        label: &str,
        kind: &str,
        object_type: &str,
        order: f64,
        count: usize,
    ) {
        let color = self.color_for_object_type(object_type);
        self.add_node_with_style(
            label,
            kind,
            "ellipse",
            &color,
            Some(object_type),
            order,
            count,
        );
    }

    fn add_node_with_style(
        &mut self,
        label: &str,
        kind: &str,
        shape: &str,
        color: &str,
        object_type: Option<&str>,
        order: f64,
        count: usize,
    ) {
        let entry = self
            .nodes
            .entry(label.to_owned())
            .or_insert_with(|| GraphNodeAccumulator {
                label: label.to_owned(),
                kind: kind.to_owned(),
                shape: shape.to_owned(),
                color: color.to_owned(),
                object_type: object_type.map(str::to_owned),
                count: 0,
                order_sum: 0.0,
                order_count: 0,
            });
        entry.count += count;
        entry.order_sum += order * count as f64;
        entry.order_count += count;
        if entry.kind != "state-change" && kind == "state-change" {
            entry.kind = kind.to_owned();
            entry.shape = shape.to_owned();
            entry.color = color.to_owned();
            entry.object_type = object_type.map(str::to_owned);
        }
    }

    fn add_edge(&mut self, source: &str, target: &str, object_type: &str, weight: usize) {
        let color = self.color_for_object_type(object_type);
        let entry = self
            .edges
            .entry((source.to_owned(), target.to_owned(), object_type.to_owned()))
            .or_insert_with(|| GraphEdgeAccumulator {
                source: source.to_owned(),
                target: target.to_owned(),
                object_type: object_type.to_owned(),
                color,
                weight: 0,
            });
        entry.weight += weight;
    }

    fn color_for_object_type(&mut self, object_type: &str) -> String {
        if let Some(color) = self.object_type_colors.get(object_type) {
            return color.clone();
        }

        let index = self.object_type_colors.len();
        let color = object_type_graph_color(index);
        self.object_type_colors
            .insert(object_type.to_owned(), color.clone());
        color
    }

    fn into_layout(self) -> OcelResult<String> {
        self.into_filtered_layout(GraphLayoutFilter::default())
    }

    fn into_filtered_layout(mut self, filter: GraphLayoutFilter) -> OcelResult<String> {
        self.apply_layout_filter(filter);
        let graph = layout_accumulated_graph(self);
        serde_json::to_string(&graph)
            .map_err(|err| OcelError::new(format!("could not serialize graph layout: {err}")))
    }

    fn apply_layout_filter(&mut self, filter: GraphLayoutFilter) {
        if filter.min_activity_frequency > 0 {
            self.nodes.retain(|_, node| {
                node.kind == "object-start"
                    || node.kind == "object-end"
                    || node.count >= filter.min_activity_frequency
            });
        }

        self.edges.retain(|_, edge| {
            edge.weight >= filter.min_path_frequency
                && self.nodes.contains_key(&edge.source)
                && self.nodes.contains_key(&edge.target)
        });

        let mut connected_boundary_nodes = BTreeSet::<String>::new();
        for edge in self.edges.values() {
            connected_boundary_nodes.insert(edge.source.clone());
            connected_boundary_nodes.insert(edge.target.clone());
        }

        self.nodes.retain(|_, node| {
            if node.kind == "object-start" || node.kind == "object-end" {
                connected_boundary_nodes.contains(&node.label)
            } else {
                true
            }
        });
    }
}

fn object_boundary_label(boundary: &str, object_type: &str) -> String {
    format!("{boundary}\n{object_type}")
}

fn object_type_graph_color(index: usize) -> String {
    let hue = (214 + index * 137) % 360;
    format!("hsl({hue} 68% 38%)")
}

fn layout_accumulated_graph(graph: GraphAccumulator) -> LayoutGraph {
    let mut node_items = graph
        .nodes
        .into_values()
        .map(|node| {
            let average_order = if node.order_count == 0 {
                0.0
            } else {
                node.order_sum / node.order_count as f64
            };
            (average_order, node)
        })
        .collect::<Vec<_>>();

    node_items.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.1.count.cmp(&left.1.count))
            .then_with(|| left.1.label.cmp(&right.1.label))
    });

    let mut layers = BTreeMap::<i32, Vec<(f64, GraphNodeAccumulator)>>::new();
    for (average_order, node) in node_items {
        layers
            .entry((average_order * 2.0).round() as i32)
            .or_default()
            .push((average_order, node));
    }
    let mut layers = layers.into_iter().collect::<Vec<_>>();
    let max_layer_rows = layers
        .iter()
        .map(|(_, nodes)| nodes.len())
        .max()
        .unwrap_or(1);

    let mut nodes = Vec::new();
    let mut node_positions = HashMap::<String, (String, f64, f64, f64, f64)>::new();
    let node_gap_x = 340.0;
    let node_gap_y = 158.0;
    let margin_x = 76.0;
    let margin_y = 76.0;
    let mut max_rows = 1usize;

    for (layer_index, (_layer, layer_nodes)) in layers.iter_mut().enumerate() {
        layer_nodes.sort_by(|left, right| {
            right
                .1
                .count
                .cmp(&left.1.count)
                .then_with(|| left.1.label.cmp(&right.1.label))
        });
        max_rows = max_rows.max(layer_nodes.len());
        let layer_offset_y =
            (max_layer_rows.saturating_sub(layer_nodes.len()) as f64 * node_gap_y) / 2.0;
        let wave_offset_y = if layer_index % 2 == 1 && layer_nodes.len() > 1 {
            node_gap_y * 0.12
        } else {
            0.0
        };
        for (row_index, (_average_order, node)) in layer_nodes.drain(..).enumerate() {
            let max_line_length = if node.shape == "ellipse" { 18 } else { 24 };
            let lines = wrap_label(&node.label, max_line_length, 4);
            let width = match node.kind.as_str() {
                "state-change" => 230.0,
                "object-start" | "object-end" => 168.0,
                _ => 215.0,
            };
            let height = (62.0 + (lines.len().saturating_sub(1) as f64 * 14.0)).max(72.0);
            let x = margin_x + layer_index as f64 * node_gap_x;
            let y = margin_y + layer_offset_y + wave_offset_y + row_index as f64 * node_gap_y;
            let id = format!("n{}", nodes.len() + 1);
            node_positions.insert(node.label.clone(), (id.clone(), x, y, width, height));
            nodes.push(LayoutNode {
                id,
                label: node.label,
                kind: node.kind,
                shape: node.shape,
                color: node.color,
                object_type: node.object_type,
                count: node.count,
                x,
                y,
                width,
                height,
                lines,
            });
        }
    }

    let width = nodes
        .iter()
        .map(|node| node.x + node.width + margin_x)
        .fold(720.0, f64::max);
    let height = (margin_y * 2.0 + max_rows as f64 * node_gap_y).max(320.0);

    let edge_items = graph.edges.into_values().collect::<Vec<_>>();
    let mut parallel_totals = BTreeMap::<(String, String), usize>::new();
    for edge in &edge_items {
        *parallel_totals
            .entry((edge.source.clone(), edge.target.clone()))
            .or_default() += 1;
    }
    let mut parallel_seen = BTreeMap::<(String, String), usize>::new();

    let mut edges = edge_items
        .into_iter()
        .filter_map(|edge| {
            let (source_id, source_x, source_y, source_width, source_height) =
                node_positions.get(&edge.source)?.clone();
            let (target_id, target_x, target_y, target_width, target_height) =
                node_positions.get(&edge.target)?.clone();
            let parallel_key = (edge.source.clone(), edge.target.clone());
            let parallel_total = *parallel_totals.get(&parallel_key).unwrap_or(&1);
            let parallel_index = parallel_seen.entry(parallel_key).or_default();
            let lane_offset = parallel_edge_offset(*parallel_index, parallel_total);
            *parallel_index += 1;
            let points = routed_edge_points(
                source_x,
                source_y,
                source_width,
                source_height,
                target_x,
                target_y,
                target_width,
                target_height,
                source_id == target_id,
                lane_offset,
            );
            let (label_x, label_y) = edge_label_position(&points);
            let path = curved_edge_path(&points);
            let object_type = edge.object_type;
            let object_types = vec![WeightedObjectType {
                object_type: object_type.clone(),
                weight: edge.weight,
            }];
            let title = format!("{object_type}: {}", edge.weight);
            Some(LayoutEdge {
                id: String::new(),
                source: source_id,
                target: target_id,
                kind: "df".to_owned(),
                path,
                label: edge.weight.to_string(),
                title,
                weight: edge.weight,
                object_type,
                color: edge.color,
                directed: true,
                points,
                label_x,
                label_y,
                object_types,
            })
        })
        .collect::<Vec<_>>();

    edges.sort_by(|left, right| {
        right
            .weight
            .cmp(&left.weight)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.object_type.cmp(&right.object_type))
    });
    for (index, edge) in edges.iter_mut().enumerate() {
        edge.id = format!("e{}", index + 1);
    }

    LayoutGraph {
        title: graph.title,
        subtitle: graph.subtitle,
        width,
        height,
        nodes,
        edges,
    }
}

fn parallel_edge_offset(index: usize, total: usize) -> f64 {
    if total <= 1 {
        return 0.0;
    }

    (index as f64 - (total as f64 - 1.0) / 2.0) * 30.0
}

fn routed_edge_points(
    source_x: f64,
    source_y: f64,
    source_width: f64,
    source_height: f64,
    target_x: f64,
    target_y: f64,
    target_width: f64,
    target_height: f64,
    self_loop: bool,
    lane_offset: f64,
) -> Vec<LayoutPoint> {
    let source_mid_y = source_y + source_height / 2.0;
    let target_mid_y = target_y + target_height / 2.0;
    if self_loop {
        let x1 = source_x + source_width;
        let y1 = source_mid_y;
        return vec![
            LayoutPoint { x: x1, y: y1 },
            LayoutPoint {
                x: x1 + 44.0,
                y: y1 - 42.0 + lane_offset,
            },
            LayoutPoint {
                x: source_x + source_width / 2.0,
                y: source_y - 28.0 + lane_offset,
            },
            LayoutPoint {
                x: source_x,
                y: y1 - 16.0,
            },
        ];
    }

    let starts_before_target = source_x + source_width <= target_x;
    if starts_before_target {
        let x1 = source_x + source_width;
        let x2 = target_x;
        let mid_x = (x1 + x2) / 2.0;
        vec![
            LayoutPoint {
                x: x1,
                y: source_mid_y,
            },
            LayoutPoint {
                x: mid_x,
                y: source_mid_y + lane_offset,
            },
            LayoutPoint {
                x: mid_x,
                y: target_mid_y + lane_offset,
            },
            LayoutPoint {
                x: x2,
                y: target_mid_y,
            },
        ]
    } else {
        let x1 = source_x;
        let x2 = target_x + target_width;
        let mid_x = (x1 + x2) / 2.0;
        vec![
            LayoutPoint {
                x: x1,
                y: source_mid_y,
            },
            LayoutPoint {
                x: mid_x,
                y: source_mid_y + lane_offset,
            },
            LayoutPoint {
                x: mid_x,
                y: target_mid_y + lane_offset,
            },
            LayoutPoint {
                x: x2,
                y: target_mid_y,
            },
        ]
    }
}

fn curved_edge_path(points: &[LayoutPoint]) -> String {
    match points {
        [] => String::new(),
        [start] => format!("M {:.1} {:.1}", start.x, start.y),
        [start, end] => format!(
            "M {:.1} {:.1} L {:.1} {:.1}",
            start.x, start.y, end.x, end.y
        ),
        [start, control, end] => format!(
            "M {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}",
            start.x, start.y, control.x, control.y, end.x, end.y
        ),
        [start, control_a, control_b, end, ..] => format!(
            "M {:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
            start.x, start.y, control_a.x, control_a.y, control_b.x, control_b.y, end.x, end.y
        ),
    }
}

fn edge_label_position(points: &[LayoutPoint]) -> (f64, f64) {
    if points.is_empty() {
        return (0.0, 0.0);
    }
    let middle = points.len() / 2;
    if points.len() % 2 == 0 {
        (
            (points[middle - 1].x + points[middle].x) / 2.0,
            (points[middle - 1].y + points[middle].y) / 2.0 - 6.0,
        )
    } else {
        (points[middle].x, points[middle].y - 6.0)
    }
}

fn wrap_label(label: &str, max_line_length: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for chunk in label.split('\n') {
        let mut current = String::new();
        for word in chunk.split_whitespace() {
            for part in split_label_word(word, max_line_length) {
                let candidate = if current.is_empty() {
                    part.clone()
                } else {
                    format!("{current} {part}")
                };
                if candidate.len() <= max_line_length {
                    current = candidate;
                } else {
                    if !current.is_empty() {
                        lines.push(current);
                    }
                    current = part;
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(label.to_owned());
    }
    if lines.len() <= max_lines {
        return lines;
    }
    let mut trimmed = lines.into_iter().take(max_lines).collect::<Vec<_>>();
    if let Some(last) = trimmed.last_mut() {
        last.truncate(max_line_length.saturating_sub(3));
        last.push_str("...");
    }
    trimmed
}

fn split_label_word(word: &str, max_line_length: usize) -> Vec<String> {
    if word.len() <= max_line_length {
        return vec![word.to_owned()];
    }
    let mut parts = Vec::new();
    let mut start = 0usize;
    while start < word.len() {
        let mut end = (start + max_line_length).min(word.len());
        while !word.is_char_boundary(end) {
            end -= 1;
        }
        parts.push(word[start..end].to_owned());
        start = end;
    }
    parts
}

fn feature_table_to_csv(table: &FeatureTableData) -> String {
    let mut output = String::new();
    output.push_str("object_id");
    for column in &table.columns {
        output.push(',');
        output.push_str(&csv_escape(column));
    }
    output.push('\n');

    for row in &table.rows {
        output.push_str(&csv_escape(&row.object_id));
        for value in &row.values {
            output.push(',');
            output.push_str(&format_numeric_feature(*value));
        }
        output.push('\n');
    }

    output
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CausalNodeRole {
    Observable,
    Latent,
    Outcome,
}

fn validate_causal_nodes<'a>(
    nodes: &'a [CausalModelNodeRequest],
    features: &[String],
) -> OcelResult<HashMap<&'a str, &'a CausalModelNodeRequest>> {
    let feature_set = features.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut node_by_id = HashMap::new();
    for node in nodes {
        if node.id.trim().is_empty() {
            return Err(OcelError::new("causal model nodes must have non-empty ids"));
        }
        if node_by_id.insert(node.id.as_str(), node).is_some() {
            return Err(OcelError::new(format!(
                "duplicate causal model node id '{}'",
                node.id
            )));
        }

        match causal_role(&node.role)? {
            CausalNodeRole::Observable | CausalNodeRole::Outcome => {
                let feature = node.feature.as_deref().ok_or_else(|| {
                    OcelError::new(format!("node '{}' must reference a feature", node.label))
                })?;
                if !feature_set.contains(feature) {
                    return Err(OcelError::new(format!(
                        "node '{}' references unknown feature '{feature}'",
                        node.label
                    )));
                }
                validate_causal_operation(&node.operation)?;
            }
            CausalNodeRole::Latent => {
                if node.feature.is_some() {
                    return Err(OcelError::new(format!(
                        "latent node '{}' cannot reference an observed feature",
                        node.label
                    )));
                }
            }
        }
    }
    Ok(node_by_id)
}

fn validate_causal_edges(
    edges: &[CausalModelEdgeRequest],
    node_by_id: &HashMap<&str, &CausalModelNodeRequest>,
) -> OcelResult<()> {
    let mut seen = HashSet::new();
    for edge in edges {
        if edge.source == edge.target {
            return Err(OcelError::new("causal model edges cannot be self-loops"));
        }
        if !seen.insert((edge.source.as_str(), edge.target.as_str())) {
            return Err(OcelError::new(format!(
                "duplicate causal edge '{} -> {}'",
                edge.source, edge.target
            )));
        }
        let source = node_by_id.get(edge.source.as_str()).ok_or_else(|| {
            OcelError::new(format!("unknown causal edge source '{}'", edge.source))
        })?;
        let target = node_by_id.get(edge.target.as_str()).ok_or_else(|| {
            OcelError::new(format!("unknown causal edge target '{}'", edge.target))
        })?;
        let source_role = causal_role(&source.role)?;
        let target_role = causal_role(&target.role)?;
        let valid = matches!(
            (source_role, target_role),
            (CausalNodeRole::Observable, CausalNodeRole::Latent)
                | (CausalNodeRole::Latent, CausalNodeRole::Latent)
                | (CausalNodeRole::Latent, CausalNodeRole::Outcome)
        );
        if !valid {
            return Err(OcelError::new(format!(
                "invalid causal edge '{} -> {}': use observable -> latent, latent -> latent, or latent -> outcome",
                source.label, target.label
            )));
        }
    }
    Ok(())
}

fn causal_topological_order(
    nodes: &[CausalModelNodeRequest],
    edges: &[CausalModelEdgeRequest],
) -> OcelResult<Vec<String>> {
    let mut indegree = nodes
        .iter()
        .map(|node| (node.id.as_str(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing = nodes
        .iter()
        .map(|node| (node.id.as_str(), Vec::<&str>::new()))
        .collect::<HashMap<_, _>>();
    for edge in edges {
        *indegree
            .get_mut(edge.target.as_str())
            .expect("edge target was validated") += 1;
        outgoing
            .get_mut(edge.source.as_str())
            .expect("edge source was validated")
            .push(edge.target.as_str());
    }

    let mut ready = nodes
        .iter()
        .filter_map(|node| (indegree[node.id.as_str()] == 0).then_some(node.id.as_str()))
        .collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(node_id) = ready.pop_front() {
        order.push(node_id.to_owned());
        for target in outgoing.get(node_id).into_iter().flatten() {
            let degree = indegree
                .get_mut(target)
                .expect("topological target was validated");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(target);
            }
        }
    }

    if order.len() != nodes.len() {
        return Err(OcelError::new(
            "causal model must be a DAG; remove latent-to-latent cycles before fitting",
        ));
    }
    Ok(order)
}

fn causal_role(role: &str) -> OcelResult<CausalNodeRole> {
    match role.trim().to_ascii_lowercase().as_str() {
        "observable" => Ok(CausalNodeRole::Observable),
        "latent" => Ok(CausalNodeRole::Latent),
        "outcome" => Ok(CausalNodeRole::Outcome),
        other => Err(OcelError::new(format!(
            "unknown causal node role '{other}'; expected observable, latent, or outcome"
        ))),
    }
}

fn validate_causal_operation(operation: &str) -> OcelResult<()> {
    match normalized_causal_operation(operation).as_str() {
        "identity" | "log10" | "log_e" | "sqrt" => Ok(()),
        other => Err(OcelError::new(format!(
            "unknown causal transform '{other}'; expected identity, log10, log_e, or sqrt"
        ))),
    }
}

fn normalized_causal_operation(operation: &str) -> String {
    match operation.trim().to_ascii_lowercase().as_str() {
        "" | "none" | "raw" | "identity" => "identity".to_owned(),
        "log_10" | "log10" => "log10".to_owned(),
        "ln" | "loge" | "log_e" => "log_e".to_owned(),
        "sqrt" | "square_root" => "sqrt".to_owned(),
        other => other.to_owned(),
    }
}

fn transform_causal_value(value: f64, operation: &str) -> f64 {
    match normalized_causal_operation(operation).as_str() {
        "log10" => {
            if value > 0.0 {
                value.log10()
            } else {
                0.0
            }
        }
        "log_e" => {
            if value > 0.0 {
                value.ln()
            } else {
                0.0
            }
        }
        "sqrt" => value.max(0.0).sqrt(),
        _ => {
            if value.is_finite() {
                value
            } else {
                0.0
            }
        }
    }
}

#[derive(Default)]
struct StateFeatureGroup {
    count: usize,
    sum: f64,
}

fn state_feature_correlation_row(
    feature: &str,
    column_index: usize,
    rows: &[FeatureRow],
    row_states: &[Option<String>],
) -> StateCorrelationRow {
    let mut total_count = 0usize;
    let mut total_sum = 0.0;
    let mut total_sum_squares = 0.0;
    let mut by_state = BTreeMap::<String, StateFeatureGroup>::new();

    for (row, state) in rows.iter().zip(row_states.iter()) {
        let Some(state) = state else {
            continue;
        };
        let Some(value) = row.values.get(column_index).copied() else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }

        total_count += 1;
        total_sum += value;
        total_sum_squares += value * value;
        let group = by_state.entry(state.clone()).or_default();
        group.count += 1;
        group.sum += value;
    }

    if total_count == 0 {
        return StateCorrelationRow {
            feature: feature.to_owned(),
            state: String::new(),
            correlation: 0.0,
            strength: 0.0,
            sample_count: 0,
            state_count: 0,
            mean_in_state: 0.0,
            mean_outside_state: 0.0,
        };
    }

    let total_mean = total_sum / total_count as f64;
    let variance = (total_sum_squares / total_count as f64 - total_mean * total_mean).max(0.0);
    let std_dev = variance.sqrt();
    let mut best_state = String::new();
    let mut best_correlation = 0.0;
    let mut best_strength = 0.0;
    let mut best_state_count = 0usize;
    let mut best_mean_in_state = total_mean;
    let mut best_mean_outside_state = 0.0;

    for (state, group) in &by_state {
        let outside_count = total_count.saturating_sub(group.count);
        let mean_in_state = group.sum / group.count as f64;
        let mean_outside_state = if outside_count > 0 {
            (total_sum - group.sum) / outside_count as f64
        } else {
            0.0
        };
        let correlation = if total_count >= 2 && outside_count > 0 && std_dev > f64::EPSILON {
            let p = group.count as f64 / total_count as f64;
            let q = outside_count as f64 / total_count as f64;
            ((mean_in_state - mean_outside_state) * (p * q).sqrt() / std_dev).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let strength = correlation.abs();
        if best_state.is_empty()
            || strength > best_strength
            || ((strength - best_strength).abs() <= f64::EPSILON && state < &best_state)
        {
            best_state = state.clone();
            best_correlation = correlation;
            best_strength = strength;
            best_state_count = group.count;
            best_mean_in_state = mean_in_state;
            best_mean_outside_state = mean_outside_state;
        }
    }

    StateCorrelationRow {
        feature: feature.to_owned(),
        state: best_state,
        correlation: round_f64(best_correlation),
        strength: round_f64(best_strength),
        sample_count: total_count,
        state_count: best_state_count,
        mean_in_state: round_f64(best_mean_in_state),
        mean_outside_state: round_f64(best_mean_outside_state),
    }
}

fn average_vectors(vectors: &[Vec<f64>], row_count: usize) -> Vec<f64> {
    if vectors.is_empty() {
        return vec![0.0; row_count];
    }
    let mut average = vec![0.0; row_count];
    for vector in vectors {
        for (index, value) in vector.iter().take(row_count).enumerate() {
            average[index] += *value / vectors.len() as f64;
        }
    }
    average
}

fn standardized_vector(values: &[f64]) -> Vec<f64> {
    let (mean, std_dev) = vector_mean_std(values);
    if std_dev <= f64::EPSILON {
        return vec![0.0; values.len()];
    }
    values
        .iter()
        .map(|value| {
            if value.is_finite() {
                (value - mean) / std_dev
            } else {
                0.0
            }
        })
        .collect()
}

fn vector_mean_std(values: &[f64]) -> (f64, f64) {
    let finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if finite.is_empty() {
        return (0.0, 0.0);
    }
    let mean = finite.iter().sum::<f64>() / finite.len() as f64;
    let variance = finite
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>()
        / finite.len().max(1) as f64;
    (mean, variance.sqrt())
}

fn pearson_correlation(left: &[f64], right: &[f64]) -> (f64, usize) {
    let pairs = left
        .iter()
        .zip(right.iter())
        .filter_map(|(left, right)| {
            (left.is_finite() && right.is_finite()).then_some((*left, *right))
        })
        .collect::<Vec<_>>();
    let sample_count = pairs.len();
    if sample_count < 2 {
        return (0.0, sample_count);
    }
    let left_mean = pairs.iter().map(|(left, _)| left).sum::<f64>() / sample_count as f64;
    let right_mean = pairs.iter().map(|(_, right)| right).sum::<f64>() / sample_count as f64;
    let mut covariance = 0.0;
    let mut left_variance = 0.0;
    let mut right_variance = 0.0;
    for (left, right) in pairs {
        let left_centered = left - left_mean;
        let right_centered = right - right_mean;
        covariance += left_centered * right_centered;
        left_variance += left_centered * left_centered;
        right_variance += right_centered * right_centered;
    }
    let denominator = (left_variance * right_variance).sqrt();
    if denominator <= f64::EPSILON {
        return (0.0, sample_count);
    }
    ((covariance / denominator).clamp(-1.0, 1.0), sample_count)
}

fn approximate_correlation_p_value(correlation: f64, sample_count: usize) -> f64 {
    if sample_count < 3 {
        return 1.0;
    }
    let denominator = (1.0 - correlation * correlation).max(1e-12);
    let t_score = correlation.abs() * (((sample_count - 2) as f64) / denominator).sqrt();
    (-0.5 * t_score * t_score).exp().clamp(0.0, 1.0)
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn format_numeric_feature(value: f64) -> String {
    if value.is_finite() && value.fract().abs() < 0.000_000_1 {
        (value as i64).to_string()
    } else {
        round_f64(value).to_string()
    }
}

fn cell_label(x: usize, y: usize) -> String {
    format!("S{}-{}", x + 1, y + 1)
}

fn attr_value_to_f64(value: &AttrValue) -> Option<f64> {
    match value {
        AttrValue::String(_) => None,
        AttrValue::Time(value) => Some(*value as f64),
        AttrValue::Integer(value) => Some(*value as f64),
        AttrValue::Float(value) if value.is_finite() => Some(*value),
        AttrValue::Float(_) => None,
        AttrValue::Boolean(value) => Some(usize::from(*value) as f64),
    }
}

fn pca_project(rows: &[Vec<f64>]) -> PcaProjection {
    let row_count = rows.len();
    let column_count = rows.first().map(Vec::len).unwrap_or_default();
    if row_count == 0 || column_count == 0 {
        return PcaProjection {
            points: Vec::new(),
            pc1_variance: 0.0,
            pc2_variance: 0.0,
            pc1_explained_ratio: 0.0,
            pc2_explained_ratio: 0.0,
        };
    }

    let mut means = vec![0.0; column_count];
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            means[index] += *value;
        }
    }
    for mean in &mut means {
        *mean /= row_count as f64;
    }

    let mut std_devs = vec![0.0; column_count];
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            let centered = value - means[index];
            std_devs[index] += centered * centered;
        }
    }
    for std_dev in &mut std_devs {
        *std_dev = (*std_dev / row_count.max(1) as f64).sqrt();
        if *std_dev <= f64::EPSILON {
            *std_dev = 1.0;
        }
    }

    let standardized = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(index, value)| (value - means[index]) / std_devs[index])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let divisor = row_count.saturating_sub(1).max(1) as f64;
    let mut covariance = vec![vec![0.0; column_count]; column_count];
    for row in &standardized {
        for left in 0..column_count {
            for right in left..column_count {
                covariance[left][right] += row[left] * row[right] / divisor;
            }
        }
    }
    for left in 0..column_count {
        for right in 0..left {
            covariance[left][right] = covariance[right][left];
        }
    }

    let total_variance = covariance
        .iter()
        .enumerate()
        .map(|(index, row)| row[index])
        .sum::<f64>()
        .max(0.0);
    let pc1 = power_iteration(&covariance, 80);
    let pc1_variance = rayleigh_quotient(&covariance, &pc1).max(0.0);
    let mut deflated = covariance.clone();
    for row in 0..column_count {
        for column in 0..column_count {
            deflated[row][column] -= pc1_variance * pc1[row] * pc1[column];
        }
    }
    let pc2 = if column_count > 1 {
        power_iteration(&deflated, 80)
    } else {
        vec![0.0; column_count]
    };
    let pc2_variance = if column_count > 1 {
        rayleigh_quotient(&covariance, &pc2).max(0.0)
    } else {
        0.0
    };

    let points = standardized
        .iter()
        .map(|row| (dot(row, &pc1), dot(row, &pc2)))
        .collect();

    PcaProjection {
        points,
        pc1_variance,
        pc2_variance,
        pc1_explained_ratio: if total_variance > f64::EPSILON {
            pc1_variance / total_variance
        } else {
            0.0
        },
        pc2_explained_ratio: if total_variance > f64::EPSILON {
            pc2_variance / total_variance
        } else {
            0.0
        },
    }
}

fn power_iteration(matrix: &[Vec<f64>], iterations: usize) -> Vec<f64> {
    let size = matrix.len();
    if size == 0 {
        return Vec::new();
    }

    let mut vector = (0..size)
        .map(|index| (index + 1) as f64 / size as f64)
        .collect::<Vec<_>>();
    normalize_vector(&mut vector);
    for _ in 0..iterations {
        let mut next = vec![0.0; size];
        for row in 0..size {
            for (column, value) in vector.iter().enumerate() {
                next[row] += matrix[row][column] * value;
            }
        }
        if vector_norm(&next) <= f64::EPSILON {
            break;
        }
        normalize_vector(&mut next);
        vector = next;
    }
    vector
}

fn rayleigh_quotient(matrix: &[Vec<f64>], vector: &[f64]) -> f64 {
    if matrix.is_empty() || vector.is_empty() {
        return 0.0;
    }
    let multiplied = matrix
        .iter()
        .map(|row| dot(row, vector))
        .collect::<Vec<_>>();
    dot(vector, &multiplied)
}

fn normalize_vector(vector: &mut [f64]) {
    let norm = vector_norm(vector);
    if norm <= f64::EPSILON {
        return;
    }
    for value in vector {
        *value /= norm;
    }
}

fn vector_norm(vector: &[f64]) -> f64 {
    vector.iter().map(|value| value * value).sum::<f64>().sqrt()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}

fn default_som_dimensions(
    point_count: usize,
    requested_width: Option<usize>,
    requested_height: Option<usize>,
) -> (usize, usize) {
    let fallback = ((point_count as f64).sqrt().ceil() as usize).clamp(3, 8);
    (
        requested_width.unwrap_or(fallback).clamp(2, 12),
        requested_height.unwrap_or(fallback).clamp(2, 12),
    )
}

fn train_som(points: &[(f64, f64)], width: usize, height: usize, epochs: usize) -> SomModel {
    let (min_x, max_x, min_y, max_y) = point_bounds(points);
    let mut weights = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let fx = if width <= 1 {
                0.5
            } else {
                x as f64 / (width - 1) as f64
            };
            let fy = if height <= 1 {
                0.5
            } else {
                y as f64 / (height - 1) as f64
            };
            weights.push((min_x + (max_x - min_x) * fx, min_y + (max_y - min_y) * fy));
        }
    }

    let max_radius = (width.max(height) as f64 / 2.0).max(1.0);
    for epoch in 0..epochs {
        let progress = if epochs <= 1 {
            1.0
        } else {
            epoch as f64 / (epochs - 1) as f64
        };
        let learning_rate = 0.5 * (1.0 - progress) + 0.05 * progress;
        let radius = max_radius * (1.0 - progress) + 0.75 * progress;
        let radius_sq = (radius * radius).max(0.01);
        for point in points {
            let (bmu_x, bmu_y) = best_matching_unit(point, &weights, width);
            for y in 0..height {
                for x in 0..width {
                    let grid_distance_sq =
                        (x.abs_diff(bmu_x).pow(2) + y.abs_diff(bmu_y).pow(2)) as f64;
                    let influence = (-grid_distance_sq / (2.0 * radius_sq)).exp();
                    let index = y * width + x;
                    weights[index].0 += learning_rate * influence * (point.0 - weights[index].0);
                    weights[index].1 += learning_rate * influence * (point.1 - weights[index].1);
                }
            }
        }
    }

    let assignments = points
        .iter()
        .map(|point| best_matching_unit(point, &weights, width))
        .collect();

    SomModel {
        width,
        assignments,
        weights,
    }
}

fn point_bounds(points: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in points {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }
    if !min_x.is_finite() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    if (max_x - min_x).abs() <= f64::EPSILON {
        min_x -= 0.5;
        max_x += 0.5;
    }
    if (max_y - min_y).abs() <= f64::EPSILON {
        min_y -= 0.5;
        max_y += 0.5;
    }
    (min_x, max_x, min_y, max_y)
}

fn best_matching_unit(point: &(f64, f64), weights: &[(f64, f64)], width: usize) -> (usize, usize) {
    let mut best_index = 0usize;
    let mut best_distance = f64::INFINITY;
    for (index, weight) in weights.iter().enumerate() {
        let distance = squared_distance(*point, *weight);
        if distance < best_distance {
            best_distance = distance;
            best_index = index;
        }
    }
    (best_index % width, best_index / width)
}

fn squared_distance(left: (f64, f64), right: (f64, f64)) -> f64 {
    let dx = left.0 - right.0;
    let dy = left.1 - right.1;
    dx * dx + dy * dy
}

fn duration_stats(mut durations: Vec<i64>) -> DurationStats {
    durations.sort_unstable();
    let sample_count = durations.len();
    let avg_duration_ms = if durations.is_empty() {
        None
    } else {
        Some(round_f64(
            durations
                .iter()
                .map(|duration| *duration as f64)
                .sum::<f64>()
                / durations.len() as f64,
        ))
    };

    DurationStats {
        sample_count,
        min_duration_ms: durations.first().copied(),
        median_duration_ms: durations.get(durations.len() / 2).copied(),
        avg_duration_ms,
        max_duration_ms: durations.last().copied(),
    }
}

fn is_recovery_transition(from_state: &str, to_state: &str) -> bool {
    if from_state == to_state {
        return false;
    }
    let to = to_state.to_ascii_lowercase();
    to.contains("normal") || to.contains("available") || to.contains("standard")
}

fn lifecycle_state_bands(events: &[LifecycleEventDetail]) -> Vec<LifecycleStateBand> {
    let mut bands = Vec::new();
    let mut start_index = None::<usize>;
    let mut current_state = None::<String>;

    for (index, event) in events.iter().enumerate() {
        let Some(state) = &event.state else {
            if let Some(start) = start_index.take() {
                if let Some(state) = current_state.take() {
                    bands.push(lifecycle_state_band(&events[start..index], state));
                }
            }
            continue;
        };

        if current_state.as_ref() != Some(state) {
            if let Some(start) = start_index.replace(index) {
                if let Some(previous_state) = current_state.replace(state.clone()) {
                    bands.push(lifecycle_state_band(&events[start..index], previous_state));
                }
            } else {
                current_state = Some(state.clone());
            }
        }
    }

    if let Some(start) = start_index {
        if let Some(state) = current_state {
            bands.push(lifecycle_state_band(&events[start..], state));
        }
    }

    bands
}

fn lifecycle_state_band(events: &[LifecycleEventDetail], state: String) -> LifecycleStateBand {
    let first = events
        .first()
        .expect("state band cannot be built from an empty event slice");
    let last = events
        .last()
        .expect("state band cannot be built from an empty event slice");
    LifecycleStateBand {
        state,
        start_time_ms: first.time_ms,
        end_time_ms: last.time_ms,
        event_count: events.len(),
        start_event_id: first.event_id.clone(),
        end_event_id: last.event_id.clone(),
    }
}

fn numeric_attr_value(value: &AttrValue) -> Option<f64> {
    match value {
        AttrValue::Integer(number) => Some(*number as f64),
        AttrValue::Float(number) if number.is_finite() => Some(*number),
        _ => None,
    }
}

fn round_f64(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum PatternFamily {
    Intra,
    Inter,
}

impl PatternFamily {
    fn as_str(self) -> &'static str {
        match self {
            Self::Intra => "intra",
            Self::Inter => "inter",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct PatternKey {
    family: PatternFamily,
    leading_object_type: String,
    state: String,
    to_state: Option<String>,
    sequence: Vec<String>,
    eo_pairs: Vec<(String, String)>,
    oo_pairs: Vec<(String, String)>,
}

#[derive(Debug)]
struct PatternInstance {
    family: PatternFamily,
    leading_object_type: String,
    state: String,
    to_state: Option<String>,
    sequence: Vec<String>,
    object_types: BTreeSet<String>,
    df_edges: BTreeMap<(String, String), usize>,
    eo_edges: BTreeMap<(String, String), usize>,
    oo_edges: BTreeMap<(String, String), usize>,
}

impl PatternInstance {
    fn key(&self) -> PatternKey {
        PatternKey {
            family: self.family,
            leading_object_type: self.leading_object_type.clone(),
            state: self.state.clone(),
            to_state: self.to_state.clone(),
            sequence: self.sequence.clone(),
            eo_pairs: self.eo_edges.keys().cloned().collect(),
            oo_pairs: self.oo_edges.keys().cloned().collect(),
        }
    }
}

#[derive(Debug)]
struct PatternAccumulator {
    key: PatternKey,
    support: usize,
    mass: usize,
    object_types: BTreeSet<String>,
    df_edges: BTreeMap<(String, String), usize>,
    eo_edges: BTreeMap<(String, String), usize>,
    oo_edges: BTreeMap<(String, String), usize>,
}

impl PatternAccumulator {
    fn new(key: PatternKey) -> Self {
        Self {
            key,
            support: 0,
            mass: 0,
            object_types: BTreeSet::new(),
            df_edges: BTreeMap::new(),
            eo_edges: BTreeMap::new(),
            oo_edges: BTreeMap::new(),
        }
    }

    fn add(&mut self, instance: PatternInstance) {
        self.support += 1;
        self.mass += instance.sequence.len().saturating_sub(1);
        self.object_types.extend(instance.object_types);
        merge_weighted_edges(&mut self.df_edges, instance.df_edges);
        merge_weighted_edges(&mut self.eo_edges, instance.eo_edges);
        merge_weighted_edges(&mut self.oo_edges, instance.oo_edges);
    }

    fn into_summary(self, index: usize) -> PatternSummary {
        let family = self.key.family.as_str();
        let label = match &self.key.to_state {
            Some(to_state) => format!(
                "{} -> {} on {}",
                self.key.state, to_state, self.key.leading_object_type
            ),
            None => format!("{} on {}", self.key.state, self.key.leading_object_type),
        };

        PatternSummary {
            id: format!("{family}-{index}"),
            family,
            label,
            leading_object_type: self.key.leading_object_type,
            state: (self.key.family == PatternFamily::Intra).then_some(self.key.state.clone()),
            from_state: (self.key.family == PatternFamily::Inter).then_some(self.key.state),
            to_state: self.key.to_state,
            support: self.support,
            mass: self.mass,
            sequence: self.key.sequence,
            object_types: self.object_types.into_iter().collect(),
            df_edges: edge_map_to_vec(self.df_edges),
            eo_edges: edge_map_to_vec(self.eo_edges),
            oo_edges: edge_map_to_vec(self.oo_edges),
        }
    }
}

#[derive(Debug)]
struct StateEpisode {
    state: String,
    start: usize,
    end: usize,
}

fn state_episodes(state_lifecycle: &[(usize, String)]) -> Vec<StateEpisode> {
    if state_lifecycle.is_empty() {
        return Vec::new();
    }

    let mut episodes = Vec::new();
    let mut start = 0usize;
    let mut current_state = state_lifecycle[0].1.clone();

    for (index, (_, state)) in state_lifecycle.iter().enumerate().skip(1) {
        if *state != current_state {
            episodes.push(StateEpisode {
                state: current_state,
                start,
                end: index - 1,
            });
            start = index;
            current_state = state.clone();
        }
    }

    episodes.push(StateEpisode {
        state: current_state,
        start,
        end: state_lifecycle.len() - 1,
    });
    episodes
}

fn insert_pattern_instance(
    patterns: &mut HashMap<PatternKey, PatternAccumulator>,
    instance: PatternInstance,
) {
    let key = instance.key();
    patterns
        .entry(key.clone())
        .or_insert_with(|| PatternAccumulator::new(key))
        .add(instance);
}

fn summarize_patterns(mut accumulators: Vec<PatternAccumulator>) -> Vec<PatternSummary> {
    accumulators.sort_by(|left, right| {
        right
            .support
            .cmp(&left.support)
            .then_with(|| right.mass.cmp(&left.mass))
            .then_with(|| pattern_sort_label(&left.key).cmp(&pattern_sort_label(&right.key)))
    });

    accumulators
        .into_iter()
        .enumerate()
        .map(|(index, accumulator)| accumulator.into_summary(index + 1))
        .collect()
}

fn pattern_sort_label(key: &PatternKey) -> String {
    match &key.to_state {
        Some(to_state) => format!("{} -> {} {}", key.state, to_state, key.leading_object_type),
        None => format!("{} {}", key.state, key.leading_object_type),
    }
}

fn pattern_filter_matches_instance(filter: &PatternFilter, instance: &PatternInstance) -> bool {
    let expected_family = match filter.family.as_str() {
        "intra" => PatternFamily::Intra,
        "inter" => PatternFamily::Inter,
        _ => return false,
    };
    if instance.family != expected_family
        || instance.leading_object_type != filter.leading_object_type
        || instance.sequence != filter.sequence
    {
        return false;
    }

    let state_matches = match expected_family {
        PatternFamily::Intra => filter
            .state
            .as_deref()
            .is_some_and(|state| state == instance.state.as_str()),
        PatternFamily::Inter => {
            let from_state = filter.from_state.as_deref().or(filter.state.as_deref());
            from_state.is_some_and(|state| state == instance.state.as_str())
                && filter.to_state.as_deref() == instance.to_state.as_deref()
        }
    };
    if !state_matches {
        return false;
    }

    pattern_edge_filter_pairs(&filter.eo_edges) == pattern_instance_edge_pairs(&instance.eo_edges)
        && pattern_edge_filter_pairs(&filter.oo_edges)
            == pattern_instance_edge_pairs(&instance.oo_edges)
}

fn pattern_edge_filter_pairs(edges: &[PatternEdgeFilter]) -> Vec<(String, String)> {
    let mut pairs = edges
        .iter()
        .map(|edge| (edge.source.clone(), edge.target.clone()))
        .collect::<Vec<_>>();
    pairs.sort();
    pairs.dedup();
    pairs
}

fn pattern_instance_edge_pairs(edges: &BTreeMap<(String, String), usize>) -> Vec<(String, String)> {
    edges.keys().cloned().collect()
}

fn merge_weighted_edges(
    target: &mut BTreeMap<(String, String), usize>,
    source: BTreeMap<(String, String), usize>,
) {
    for (edge, weight) in source {
        *target.entry(edge).or_default() += weight;
    }
}

fn edge_map_to_vec(edges: BTreeMap<(String, String), usize>) -> Vec<PatternEdge> {
    edges
        .into_iter()
        .map(|((source, target), weight)| PatternEdge {
            source,
            target,
            weight,
        })
        .collect()
}

fn unordered_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_owned(), right.to_owned())
    } else {
        (right.to_owned(), left.to_owned())
    }
}
