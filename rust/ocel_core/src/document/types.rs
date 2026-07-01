#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OcelFormat {
    Json,
    Xml,
}

impl OcelFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct Symbol(u32);

#[derive(Debug, Default, Clone)]
struct StringPool {
    values: Vec<String>,
    index: HashMap<String, Symbol>,
}

impl StringPool {
    fn intern(&mut self, value: &str) -> Symbol {
        if self.index.is_empty() && !self.values.is_empty() {
            self.rebuild_index();
        }

        if let Some(symbol) = self.index.get(value) {
            return *symbol;
        }

        let symbol = Symbol(self.values.len() as u32);
        let owned = value.to_owned();
        self.values.push(owned.clone());
        self.index.insert(owned, symbol);
        symbol
    }

    fn resolve(&self, symbol: Symbol) -> &str {
        &self.values[symbol.0 as usize]
    }

    fn finish(mut self) -> Self {
        self.index.clear();
        self.index.shrink_to_fit();
        self
    }

    fn rebuild_index(&mut self) {
        self.index = self
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| (value.clone(), Symbol(index as u32)))
            .collect();
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum AttrType {
    String,
    Time,
    Integer,
    Float,
    Boolean,
}

impl AttrType {
    fn parse(value: &str) -> OcelResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "string" => Ok(Self::String),
            "time" => Ok(Self::Time),
            "integer" => Ok(Self::Integer),
            "float" => Ok(Self::Float),
            "boolean" => Ok(Self::Boolean),
            other => Err(OcelError::new(format!(
                "unsupported OCEL attribute type '{other}'"
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Time => "time",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
        }
    }
}

#[derive(Debug, Clone)]
struct AttributeDef {
    name: Symbol,
    attr_type: AttrType,
}

#[derive(Debug, Clone)]
struct TypeDef {
    name: Symbol,
    attributes: Vec<AttributeDef>,
}

#[derive(Debug, Clone)]
enum AttrValue {
    String(Symbol),
    Time(i64),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug, Clone)]
struct Attribute {
    name: Symbol,
    value: AttrValue,
}

#[derive(Debug, Clone)]
struct TimedAttribute {
    name: Symbol,
    time_ms: i64,
    value: AttrValue,
}

#[derive(Debug, Clone)]
struct Relationship {
    object_id: Symbol,
    qualifier: Symbol,
}

#[derive(Debug, Clone)]
struct Event {
    id: Symbol,
    type_name: Symbol,
    time_ms: i64,
    attributes: Vec<Attribute>,
    relationships: Vec<Relationship>,
}

#[derive(Debug, Clone)]
struct Object {
    id: Symbol,
    type_name: Symbol,
    attributes: Vec<TimedAttribute>,
    relationships: Vec<Relationship>,
    lifecycle: Vec<usize>,
}

#[derive(Debug, Clone)]
struct CompactOcelLog {
    format: OcelFormat,
    pool: StringPool,
    event_types: Vec<TypeDef>,
    object_types: Vec<TypeDef>,
    events: Vec<Event>,
    objects: Vec<Object>,
    object_index: HashMap<Symbol, usize>,
    state_leading_object_type: Option<Symbol>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct OcelSummary {
    source_format: &'static str,
    event_types: usize,
    object_types: usize,
    events: usize,
    objects: usize,
    e2o_relationships: usize,
    o2o_relationships: usize,
    interned_strings: usize,
    objects_with_lifecycle: usize,
    stateful_events: usize,
}

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
    color_attribute: Option<String>,
}

#[derive(Deserialize)]
struct StateDetectionCellRequest {
    object_type: String,
    window_size: Option<usize>,
    som_width: Option<usize>,
    som_height: Option<usize>,
    epochs: Option<usize>,
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

impl OcelFilterRequest {
    fn all_for(log: &CompactOcelLog) -> Self {
        let options = log.filter_options();
        Self {
            event_types: options.event_types,
            object_types: options.object_types,
            time_range: None,
            df_nodes: Vec::new(),
            df_edges: Vec::new(),
            text_attributes: Vec::new(),
            patterns: Vec::new(),
        }
    }

    fn has_object_predicates(&self) -> bool {
        !self.df_nodes.is_empty()
            || !self.df_edges.is_empty()
            || self
                .text_attributes
                .iter()
                .any(|attribute| !attribute.values.is_empty())
            || !self.patterns.is_empty()
    }

    fn has_time_predicate(&self) -> bool {
        self.time_range
            .as_ref()
            .is_some_and(|range| range.start_ms.is_some() || range.end_ms.is_some())
    }

    fn accepts_time(&self, time_ms: i64) -> bool {
        let Some(range) = &self.time_range else {
            return true;
        };
        if range.start_ms.is_some_and(|start_ms| time_ms < start_ms) {
            return false;
        }
        if range.end_ms.is_some_and(|end_ms| time_ms > end_ms) {
            return false;
        }
        true
    }
}

fn default_text_attribute_scope() -> String {
    "event".to_owned()
}

impl GraphFilterRequest {
    fn layout_filter(&self) -> GraphLayoutFilter {
        GraphLayoutFilter {
            min_activity_frequency: self.min_activity_frequency.unwrap_or_default(),
            min_path_frequency: self.min_path_frequency.unwrap_or_default(),
        }
    }
}
