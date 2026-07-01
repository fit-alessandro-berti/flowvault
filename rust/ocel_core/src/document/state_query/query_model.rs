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

#[derive(Debug)]
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
}

impl StateEvalIndex {
    fn build(log: &CompactOcelLog, query: &StateQuery) -> Self {
        let fields = query.referenced_fields();
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
}

impl ValueExpr {
    fn references_object(&self) -> bool {
        matches!(self, Self::Field(FieldRef::Object(_)))
    }

    fn collect_fields(&self, fields: &mut ReferencedFields) {
        match self {
            Self::Field(FieldRef::Event(field)) => fields.add_event_field(field),
            Self::Field(FieldRef::Object(field)) => fields.add_object_field(field),
            Self::Literal(_) => {}
        }
    }
}

#[derive(Debug, Clone)]
enum FieldRef {
    Event(String),
    Object(String),
}
