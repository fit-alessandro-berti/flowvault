
#[derive(Debug, Clone)]
enum QueryValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl QueryValue {
    fn as_state_string(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => {
                if value.fract() == 0.0 {
                    (*value as i64).to_string()
                } else {
                    value.to_string()
                }
            }
            Self::Boolean(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Like,
}

struct EvalContext<'a> {
    log: &'a CompactOcelLog,
    eval_index: &'a StateEvalIndex,
    event_index: usize,
    object_index: Option<usize>,
}

impl EvalContext<'_> {
    fn eval_condition(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Or(left, right) => self.eval_condition(left) || self.eval_condition(right),
            Expr::And(left, right) => self.eval_condition(left) && self.eval_condition(right),
            Expr::Not(expr) => !self.eval_condition(expr),
            Expr::Compare { left, op, right } => {
                let Some(left_value) = self.eval_value(left) else {
                    return false;
                };
                let Some(right_value) = self.eval_value(right) else {
                    return false;
                };
                compare_query_values(&left_value, *op, &right_value)
            }
            Expr::IsNull { value, negated } => {
                let is_null = self.eval_value(value).is_none();
                if *negated {
                    !is_null
                } else {
                    is_null
                }
            }
        }
    }

    fn eval_state_value(&self, value: &ValueExpr) -> Option<String> {
        self.eval_value(value).map(|value| value.as_state_string())
    }

    fn eval_value(&self, value: &ValueExpr) -> Option<QueryValue> {
        match value {
            ValueExpr::Literal(value) => Some(value.clone()),
            ValueExpr::Field(field) => self.eval_field(field),
        }
    }

    fn eval_field(&self, field: &FieldRef) -> Option<QueryValue> {
        match field {
            FieldRef::Event(field_name) => self.eval_event_field(field_name),
            FieldRef::Object(field_name) => self.eval_object_field(field_name),
        }
    }

    fn eval_event_field(&self, field_name: &str) -> Option<QueryValue> {
        let event = &self.log.events[self.event_index];
        match field_name.to_ascii_lowercase().as_str() {
            "id" => Some(QueryValue::String(
                self.log.pool.resolve(event.id).to_owned(),
            )),
            "type" | "activity" => Some(QueryValue::String(
                self.log.pool.resolve(event.type_name).to_owned(),
            )),
            "time" | "timestamp" => Some(QueryValue::Number(event.time_ms as f64)),
            other => self.eval_index.event_attributes[self.event_index]
                .get(other)
                .map(|attribute_index| {
                    self.attr_value_to_query_value(&event.attributes[*attribute_index].value)
                }),
        }
    }

    fn eval_object_field(&self, field_name: &str) -> Option<QueryValue> {
        let object = &self.log.objects[self.object_index?];
        match field_name.to_ascii_lowercase().as_str() {
            "id" => Some(QueryValue::String(
                self.log.pool.resolve(object.id).to_owned(),
            )),
            "type" => Some(QueryValue::String(
                self.log.pool.resolve(object.type_name).to_owned(),
            )),
            other => self.object_attribute_at_event_time(self.object_index?, object, other),
        }
    }

    fn object_attribute_at_event_time(
        &self,
        object_index: usize,
        object: &Object,
        attribute_name: &str,
    ) -> Option<QueryValue> {
        let event_time = self.log.events[self.event_index].time_ms;
        let attribute_indexes =
            self.eval_index.object_attributes[object_index].get(attribute_name)?;
        let partition = attribute_indexes.partition_point(|attribute_index| {
            object.attributes[*attribute_index].time_ms <= event_time
        });
        if partition == 0 {
            return None;
        }
        let attribute = &object.attributes[attribute_indexes[partition - 1]];
        Some(self.attr_value_to_query_value(&attribute.value))
    }

    fn attr_value_to_query_value(&self, value: &AttrValue) -> QueryValue {
        match value {
            AttrValue::String(symbol) => {
                QueryValue::String(self.log.pool.resolve(*symbol).to_owned())
            }
            AttrValue::Time(ms) => QueryValue::Number(*ms as f64),
            AttrValue::Integer(value) => QueryValue::Number(*value as f64),
            AttrValue::Float(value) => QueryValue::Number(*value),
            AttrValue::Boolean(value) => QueryValue::Boolean(*value),
        }
    }
}

fn compare_query_values(left: &QueryValue, op: CompareOp, right: &QueryValue) -> bool {
    match op {
        CompareOp::Eq => query_values_equal(left, right),
        CompareOp::Ne => !query_values_equal(left, right),
        CompareOp::Lt => compare_ordered(left, right).is_some_and(|ordering| ordering < 0),
        CompareOp::Le => compare_ordered(left, right).is_some_and(|ordering| ordering <= 0),
        CompareOp::Gt => compare_ordered(left, right).is_some_and(|ordering| ordering > 0),
        CompareOp::Ge => compare_ordered(left, right).is_some_and(|ordering| ordering >= 0),
        CompareOp::Like => sql_like(
            &left.as_state_string().to_ascii_lowercase(),
            &right.as_state_string().to_ascii_lowercase(),
        ),
    }
}

fn query_values_equal(left: &QueryValue, right: &QueryValue) -> bool {
    match (left, right) {
        (QueryValue::Boolean(left), QueryValue::Boolean(right)) => left == right,
        _ if query_value_as_number(left).is_some() && query_value_as_number(right).is_some() => {
            (query_value_as_number(left).expect("checked numeric left")
                - query_value_as_number(right).expect("checked numeric right"))
            .abs()
                < f64::EPSILON
        }
        _ => left.as_state_string() == right.as_state_string(),
    }
}
