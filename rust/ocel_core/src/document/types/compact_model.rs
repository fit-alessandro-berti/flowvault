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
