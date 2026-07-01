
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
