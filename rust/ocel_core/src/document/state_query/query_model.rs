#[derive(Serialize)]
struct StateQueryResult {
    attribute: String,
    leading_object_type: String,
    assigned_events: usize,
    total_events: usize,
}

#[derive(Debug)]
struct StateQuery {
    attribute_name: String,
    leading_object_type: String,
    branches: Vec<StateBranch>,
    else_value: Option<ValueExpr>,
}

#[derive(Debug, Clone)]
struct StateBranch {
    condition: Expr,
    value: ValueExpr,
}

impl StateQuery {
    fn parse(query: &str) -> OcelResult<Self> {
        let tokens = tokenize(query)?;
        let mut parser = QueryParser {
            tokens,
            position: 0,
        };
        parser.parse_state_query()
    }

    fn referenced_fields(&self) -> ReferencedFields {
        let mut fields = ReferencedFields::default();
        for branch in &self.branches {
            branch.condition.collect_fields(&mut fields);
            branch.value.collect_fields(&mut fields);
        }
        if let Some(value) = &self.else_value {
            value.collect_fields(&mut fields);
        }
        fields
    }
}

#[derive(Default)]
struct ReferencedFields {
    event_attributes: HashSet<String>,
    object_attributes: HashSet<String>,
    quantile_fields: HashSet<FieldRef>,
}

impl ReferencedFields {
    fn add_event_field(&mut self, field: &str) {
        if !matches!(field, "id" | "type" | "activity" | "time" | "timestamp") {
            self.event_attributes.insert(field.to_owned());
        }
    }

    fn add_object_field(&mut self, field: &str) {
        if !matches!(field, "id" | "type") {
            self.object_attributes.insert(field.to_owned());
        }
    }
}

struct StateEvalIndex {
    event_attributes: Vec<HashMap<String, usize>>,
    object_attributes: Vec<HashMap<String, Vec<usize>>>,
    quantile_values: HashMap<FieldRef, Vec<f64>>,
}

impl StateEvalIndex {
    fn build(log: &CompactOcelLog, query: &StateQuery) -> OcelResult<Self> {
        let leading_type = log
            .object_type_symbol(&query.leading_object_type)
            .ok_or_else(|| OcelError::new("unknown leading object type for quantiles"))?;
        Self::with_quantiles(log, query.referenced_fields(), leading_type)
    }

    fn for_fields(log: &CompactOcelLog, fields: ReferencedFields) -> Self {
        let event_attributes = log
            .events
            .iter()
            .map(|event| {
                event
                    .attributes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, attribute)| {
                        let name = log.pool.resolve(attribute.name).to_ascii_lowercase();
                        fields
                            .event_attributes
                            .contains(&name)
                            .then_some((name, index))
                    })
                    .collect::<HashMap<_, _>>()
            })
            .collect();
        let object_attributes = log
            .objects
            .iter()
            .map(|object| {
                let mut attributes = HashMap::<String, Vec<usize>>::new();
                for (index, attribute) in object.attributes.iter().enumerate() {
                    let name = log.pool.resolve(attribute.name).to_ascii_lowercase();
                    if fields.object_attributes.contains(&name) {
                        attributes.entry(name).or_default().push(index);
                    }
                }
                for indexes in attributes.values_mut() {
                    indexes.sort_by_key(|index| object.attributes[*index].time_ms);
                }
                attributes
            })
            .collect();

        Self {
            event_attributes,
            object_attributes,
            quantile_values: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare {
        left: ValueExpr,
        op: CompareOp,
        right: ValueExpr,
    },
    IsNull {
        value: ValueExpr,
        negated: bool,
    },
}

impl Expr {
    fn references_object(&self) -> bool {
        match self {
            Self::Or(left, right) | Self::And(left, right) => {
                left.references_object() || right.references_object()
            }
            Self::Not(expr) => expr.references_object(),
            Self::Compare { left, right, .. } => {
                left.references_object() || right.references_object()
            }
            Self::IsNull { value, .. } => value.references_object(),
        }
    }

    fn collect_fields(&self, fields: &mut ReferencedFields) {
        match self {
            Self::Or(left, right) | Self::And(left, right) => {
                left.collect_fields(fields);
                right.collect_fields(fields);
            }
            Self::Not(expr) => expr.collect_fields(fields),
            Self::Compare { left, right, .. } => {
                left.collect_fields(fields);
                right.collect_fields(fields);
            }
            Self::IsNull { value, .. } => value.collect_fields(fields),
        }
    }
}

#[derive(Debug, Clone)]
enum ValueExpr {
    Field(FieldRef),
    Literal(QueryValue),
    Concat(Vec<ValueExpr>),
    Coalesce(Vec<ValueExpr>),
    Case(Vec<StateBranch>, Option<Box<ValueExpr>>),
    Null,
    Quantile { field: FieldRef, probability: f64 },
}

impl ValueExpr {
    fn references_object(&self) -> bool {
        match self {
            Self::Field(FieldRef::Object(_)) => true,
            Self::Concat(values) | Self::Coalesce(values) => {
                values.iter().any(Self::references_object)
            }
            Self::Case(branches, fallback) => {
                branches.iter().any(|branch| {
                    branch.condition.references_object() || branch.value.references_object()
                }) || fallback
                    .as_ref()
                    .is_some_and(|value| value.references_object())
            }
            _ => false,
        }
    }

    fn collect_fields(&self, fields: &mut ReferencedFields) {
        match self {
            Self::Field(FieldRef::Event(field)) => fields.add_event_field(field),
            Self::Field(FieldRef::Object(field)) => fields.add_object_field(field),
            Self::Concat(values) | Self::Coalesce(values) => {
                for value in values {
                    value.collect_fields(fields);
                }
            }
            Self::Case(branches, fallback) => {
                for branch in branches {
                    branch.condition.collect_fields(fields);
                    branch.value.collect_fields(fields);
                }
                if let Some(value) = fallback {
                    value.collect_fields(fields);
                }
            }
            Self::Quantile { field, .. } => {
                fields.quantile_fields.insert(field.clone());
                Self::Field(field.clone()).collect_fields(fields);
            }
            Self::Literal(_) | Self::Null => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FieldRef {
    Event(String),
    Object(String),
}
