const COMBINED_STATE: &str = "combined_state";
const STATE_ACTIVITY: &str = "state_activity";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateComposition {
    leading_object_type: String,
    state_expression: String,
    activity_expression: String,
}

#[derive(Clone, Serialize)]
struct StateDefinition {
    attribute: String,
    leading_object_type: String,
    query: String,
}

impl StateComposition {
    fn for_attribute(attribute: &str, leading_object_type: &str) -> Self {
        Self {
            leading_object_type: leading_object_type.to_owned(),
            state_expression: format!("event.\"{}\"", attribute.replace('"', "\"\"")),
            activity_expression: "CONCAT(event.type, ' [', event.combined_state, ']')".to_owned(),
        }
    }
}

impl CompactOcelLog {
    fn state_attribute_symbol(&self) -> Option<Symbol> {
        [COMBINED_STATE, "state"].iter().find_map(|name| {
            self.event_types
                .iter()
                .flat_map(|kind| &kind.attributes)
                .find(|attribute| self.pool.resolve(attribute.name) == *name)
                .map(|attribute| attribute.name)
        })
    }

    fn parse_composition_expression(
        &self,
        expression: &str,
        allow_combined: bool,
    ) -> OcelResult<ValueExpr> {
        let mut parser = QueryParser {
            tokens: tokenize(expression)?,
            position: 0,
        };
        let value = parser.parse_value_expr()?;
        if !parser.is_done() {
            return Err(OcelError::new(
                "unexpected tokens after composition expression",
            ));
        }
        let mut fields = ReferencedFields::default();
        value.collect_fields(&mut fields);
        if value.references_object() {
            return Err(OcelError::new("composition uses event attributes; define object states separately with a STATE query"));
        }
        for field in &fields.event_attributes {
            if field == STATE_ACTIVITY || (!allow_combined && field == COMBINED_STATE) {
                return Err(OcelError::new(format!(
                    "composition cannot reference its output '{field}'"
                )));
            }
            if allow_combined && field == COMBINED_STATE {
                continue;
            }
            if !self.event_types.iter().any(|kind| {
                kind.attributes
                    .iter()
                    .any(|attr| self.pool.resolve(attr.name).eq_ignore_ascii_case(field))
            }) {
                return Err(OcelError::new(format!(
                    "unknown event attribute '{field}' in composition"
                )));
            }
        }
        Ok(value)
    }

    fn evaluate_event_expression(&self, value: &ValueExpr) -> Vec<Option<String>> {
        let mut fields = ReferencedFields::default();
        value.collect_fields(&mut fields);
        let eval_index = StateEvalIndex::for_fields(self, fields);
        (0..self.events.len())
            .map(|event_index| {
                EvalContext {
                    log: self,
                    eval_index: &eval_index,
                    event_index,
                    object_index: None,
                }
                .eval_state_value(value)
            })
            .collect()
    }

    fn write_event_strings(&mut self, name: &str, values: Vec<Option<String>>) {
        let attribute = self.pool.intern(name);
        self.ensure_event_attribute(attribute, AttrType::String);
        for (event, value) in self.events.iter_mut().zip(values) {
            event.attributes.retain(|item| item.name != attribute);
            if let Some(value) = value {
                event.attributes.push(Attribute {
                    name: attribute,
                    value: AttrValue::String(self.pool.intern(&value)),
                });
            }
        }
    }

    fn apply_state_composition(&mut self, composition: &StateComposition) -> OcelResult<()> {
        let leading_type = self
            .object_type_symbol(&composition.leading_object_type)
            .ok_or_else(|| {
                OcelError::new(format!(
                    "unknown leading object type '{}'",
                    composition.leading_object_type
                ))
            })?;
        // Validate both expressions before writing either output.
        let state = self.parse_composition_expression(&composition.state_expression, false)?;
        let activity = self.parse_composition_expression(&composition.activity_expression, true)?;
        let values = self.evaluate_event_expression(&state);
        self.write_event_strings(COMBINED_STATE, values);
        let labels = self.evaluate_event_expression(&activity);
        // A composed state must have an activity label; missing labels are explicit errors.
        let state_attribute = self
            .symbol_for_value(COMBINED_STATE)
            .expect("written state");
        if self.events.iter().zip(&labels).any(|(event, label)| {
            self.event_state(event, state_attribute).is_some()
                && label.as_ref().is_none_or(|label| label.is_empty())
        }) {
            return Err(OcelError::new("activity expression produced a missing or empty label for a stateful event; use COALESCE to provide a fallback"));
        }
        self.write_event_strings(STATE_ACTIVITY, labels);
        self.state_leading_object_type = Some(leading_type);
        Ok(())
    }
}
