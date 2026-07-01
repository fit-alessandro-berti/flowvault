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

fn compare_ordered(left: &QueryValue, right: &QueryValue) -> Option<i8> {
    match (query_value_as_number(left), query_value_as_number(right)) {
        (Some(left), Some(right)) => {
            if left < right {
                Some(-1)
            } else if left > right {
                Some(1)
            } else {
                Some(0)
            }
        }
        _ => {
            let left = left.as_state_string();
            let right = right.as_state_string();
            match left.cmp(&right) {
                std::cmp::Ordering::Less => Some(-1),
                std::cmp::Ordering::Equal => Some(0),
                std::cmp::Ordering::Greater => Some(1),
            }
        }
    }
}

fn query_value_as_number(value: &QueryValue) -> Option<f64> {
    match value {
        QueryValue::Number(value) => Some(*value),
        QueryValue::String(value) => value.trim().parse::<f64>().ok(),
        QueryValue::Boolean(_) => None,
    }
}

fn sql_like(value: &str, pattern: &str) -> bool {
    if pattern == "%" {
        return true;
    }

    let parts = pattern.split('%').collect::<Vec<_>>();
    if parts.len() == 1 {
        return value == pattern;
    }

    let mut remainder = value;
    let starts_with_wildcard = pattern.starts_with('%');
    let ends_with_wildcard = pattern.ends_with('%');

    for (index, part) in parts.iter().filter(|part| !part.is_empty()).enumerate() {
        if index == 0 && !starts_with_wildcard {
            if !remainder.starts_with(part) {
                return false;
            }
            remainder = &remainder[part.len()..];
            continue;
        }

        let Some(position) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[position + part.len()..];
    }

    ends_with_wildcard
        || parts
            .last()
            .is_none_or(|last| last.is_empty() || remainder.is_empty())
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Identifier(String),
    String(String),
    Number(f64),
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Dot,
    LParen,
    RParen,
    Semicolon,
}

fn tokenize(input: &str) -> OcelResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((index, character)) = chars.next() {
        match character {
            character if character.is_whitespace() => {}
            '-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
                for (_, next) in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '\'' | '"' => {
                let quote = character;
                let mut value = String::new();
                let mut closed = false;
                while let Some((_, next)) = chars.next() {
                    if next == quote {
                        if chars.peek().is_some_and(|(_, repeated)| *repeated == quote) {
                            chars.next();
                            value.push(quote);
                        } else {
                            closed = true;
                            break;
                        }
                    } else {
                        value.push(next);
                    }
                }
                if !closed {
                    return Err(OcelError::new("unterminated string literal in state query"));
                }
                tokens.push(Token::String(value));
            }
            '=' => tokens.push(Token::Eq),
            '!' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Ne);
            }
            '<' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Le);
            }
            '<' if chars.peek().is_some_and(|(_, next)| *next == '>') => {
                chars.next();
                tokens.push(Token::Ne);
            }
            '<' => tokens.push(Token::Lt),
            '>' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Ge);
            }
            '>' => tokens.push(Token::Gt),
            '.' => tokens.push(Token::Dot),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ';' => tokens.push(Token::Semicolon),
            character if character.is_ascii_digit() || character == '-' => {
                let mut literal = character.to_string();
                while let Some((_, next)) = chars.peek() {
                    if next.is_ascii_digit() || *next == '.' {
                        literal.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let number = literal.parse::<f64>().map_err(|err| {
                    OcelError::new(format!("invalid numeric literal '{literal}': {err}"))
                })?;
                tokens.push(Token::Number(number));
            }
            character if is_identifier_start(character) => {
                let mut identifier = character.to_string();
                while let Some((_, next)) = chars.peek() {
                    if is_identifier_part(*next) {
                        identifier.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Identifier(identifier));
            }
            other => {
                return Err(OcelError::new(format!(
                    "unexpected character '{other}' at byte {index} in state query"
                )));
            }
        }
    }

    Ok(tokens)
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_part(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

struct QueryParser {
    tokens: Vec<Token>,
    position: usize,
}

impl QueryParser {
    fn parse_state_query(&mut self) -> OcelResult<StateQuery> {
        self.expect_keyword("STATE")?;
        let attribute_name = self.parse_identifier()?;
        self.expect_keyword("FOR")?;
        self.expect_keyword("LEADING")?;
        self.expect_keyword("OBJECT")?;
        self.expect_keyword("TYPE")?;
        let leading_object_type = self.parse_type_name()?;
        self.expect_keyword("AS")?;
        self.expect_keyword("CASE")?;

        let mut branches = Vec::new();
        while self.consume_keyword("WHEN") {
            let condition = self.parse_expr()?;
            self.expect_keyword("THEN")?;
            let value = self.parse_value_expr()?;
            branches.push(StateBranch { condition, value });
        }

        if branches.is_empty() {
            return Err(OcelError::new(
                "state query must contain at least one WHEN branch",
            ));
        }

        let else_value = if self.consume_keyword("ELSE") {
            Some(self.parse_value_expr()?)
        } else {
            None
        };

        self.expect_keyword("END")?;
        self.consume_token(&Token::Semicolon);
        if !self.is_done() {
            return Err(OcelError::new("unexpected tokens after END in state query"));
        }

        Ok(StateQuery {
            attribute_name,
            leading_object_type,
            branches,
            else_value,
        })
    }

    fn parse_expr(&mut self) -> OcelResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> OcelResult<Expr> {
        let mut expr = self.parse_and()?;
        while self.consume_keyword("OR") {
            let right = self.parse_and()?;
            expr = Expr::Or(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> OcelResult<Expr> {
        let mut expr = self.parse_not()?;
        while self.consume_keyword("AND") {
            let right = self.parse_not()?;
            expr = Expr::And(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    fn parse_not(&mut self) -> OcelResult<Expr> {
        if self.consume_keyword("NOT") {
            Ok(Expr::Not(Box::new(self.parse_not()?)))
        } else {
            self.parse_predicate()
        }
    }

    fn parse_predicate(&mut self) -> OcelResult<Expr> {
        if self.consume_token(&Token::LParen) {
            let expr = self.parse_expr()?;
            self.expect_token(&Token::RParen)?;
            return Ok(expr);
        }

        let left = self.parse_value_expr()?;

        if self.consume_keyword("IS") {
            let negated = self.consume_keyword("NOT");
            self.expect_keyword("NULL")?;
            return Ok(Expr::IsNull {
                value: left,
                negated,
            });
        }

        let op = self.parse_compare_op()?;
        let right = self.parse_value_expr()?;
        Ok(Expr::Compare { left, op, right })
    }

    fn parse_compare_op(&mut self) -> OcelResult<CompareOp> {
        if self.consume_token(&Token::Eq) {
            Ok(CompareOp::Eq)
        } else if self.consume_token(&Token::Ne) {
            Ok(CompareOp::Ne)
        } else if self.consume_token(&Token::Le) {
            Ok(CompareOp::Le)
        } else if self.consume_token(&Token::Lt) {
            Ok(CompareOp::Lt)
        } else if self.consume_token(&Token::Ge) {
            Ok(CompareOp::Ge)
        } else if self.consume_token(&Token::Gt) {
            Ok(CompareOp::Gt)
        } else if self.consume_keyword("LIKE") {
            Ok(CompareOp::Like)
        } else {
            Err(OcelError::new(
                "expected comparison operator in state query predicate",
            ))
        }
    }

    fn parse_value_expr(&mut self) -> OcelResult<ValueExpr> {
        match self.next().cloned() {
            Some(Token::String(value)) => Ok(ValueExpr::Literal(QueryValue::String(value))),
            Some(Token::Number(value)) => Ok(ValueExpr::Literal(QueryValue::Number(value))),
            Some(Token::Identifier(identifier)) => {
                if identifier.eq_ignore_ascii_case("TRUE") {
                    return Ok(ValueExpr::Literal(QueryValue::Boolean(true)));
                }
                if identifier.eq_ignore_ascii_case("FALSE") {
                    return Ok(ValueExpr::Literal(QueryValue::Boolean(false)));
                }
                if self.consume_token(&Token::Dot) {
                    let field = self.parse_field_name()?;
                    if identifier.eq_ignore_ascii_case("EVENT") {
                        Ok(ValueExpr::Field(FieldRef::Event(
                            field.to_ascii_lowercase(),
                        )))
                    } else if identifier.eq_ignore_ascii_case("OBJECT") {
                        Ok(ValueExpr::Field(FieldRef::Object(
                            field.to_ascii_lowercase(),
                        )))
                    } else {
                        Err(OcelError::new(format!(
                            "unsupported field namespace '{identifier}', expected event or object"
                        )))
                    }
                } else {
                    Ok(ValueExpr::Literal(QueryValue::String(identifier)))
                }
            }
            other => Err(OcelError::new(format!(
                "expected field or literal in state query, found {other:?}"
            ))),
        }
    }

    fn parse_field_name(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) | Some(Token::String(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected field name in state query, found {other:?}"
            ))),
        }
    }

    fn parse_identifier(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected identifier in state query, found {other:?}"
            ))),
        }
    }

    fn parse_type_name(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) | Some(Token::String(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected leading object type name in state query, found {other:?}"
            ))),
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> OcelResult<()> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(OcelError::new(format!(
                "expected keyword {keyword} in state query"
            )))
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        match self.tokens.get(self.position) {
            Some(Token::Identifier(identifier)) if identifier.eq_ignore_ascii_case(keyword) => {
                self.position += 1;
                true
            }
            _ => false,
        }
    }

    fn expect_token(&mut self, token: &Token) -> OcelResult<()> {
        if self.consume_token(token) {
            Ok(())
        } else {
            Err(OcelError::new(format!(
                "expected token {token:?} in state query"
            )))
        }
    }

    fn consume_token(&mut self, token: &Token) -> bool {
        if self.tokens.get(self.position) == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    fn is_done(&self) -> bool {
        self.position >= self.tokens.len()
    }
}
