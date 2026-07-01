impl CompactOcelLog {

    fn evaluate_state_query(
        &self,
        query: &StateQuery,
        eval_index: &StateEvalIndex,
        event_index: usize,
    ) -> Option<String> {
        let event = &self.events[event_index];
        let leading_type_symbol = self.object_type_symbol(&query.leading_object_type)?;
        let related_objects = event
            .relationships
            .iter()
            .filter_map(|relationship| self.object_index.get(&relationship.object_id).copied())
            .filter(|object_index| self.objects[*object_index].type_name == leading_type_symbol)
            .collect::<Vec<_>>();
        if related_objects.is_empty() {
            return None;
        }

        for branch in &query.branches {
            if branch.condition.references_object() {
                for object_index in &related_objects {
                    let context = EvalContext {
                        log: self,
                        eval_index,
                        event_index,
                        object_index: Some(*object_index),
                    };
                    if context.eval_condition(&branch.condition) {
                        return context.eval_state_value(&branch.value);
                    }
                }
            } else {
                let context = EvalContext {
                    log: self,
                    eval_index,
                    event_index,
                    object_index: None,
                };
                if context.eval_condition(&branch.condition) {
                    return context.eval_state_value(&branch.value);
                }
            }
        }

        query.else_value.as_ref().and_then(|value| {
            if value.references_object() {
                related_objects.first().and_then(|object_index| {
                    EvalContext {
                        log: self,
                        eval_index,
                        event_index,
                        object_index: Some(*object_index),
                    }
                    .eval_state_value(value)
                })
            } else {
                EvalContext {
                    log: self,
                    eval_index,
                    event_index,
                    object_index: None,
                }
                .eval_state_value(value)
            }
        })
    }

    fn export_json(&self) -> OcelResult<String> {
        let mut top = Map::new();
        top.insert(
            "eventTypes".to_owned(),
            Value::Array(
                self.event_types
                    .iter()
                    .map(|type_def| self.type_def_to_json(type_def))
                    .collect(),
            ),
        );
        top.insert(
            "objectTypes".to_owned(),
            Value::Array(
                self.object_types
                    .iter()
                    .map(|type_def| self.type_def_to_json(type_def))
                    .collect(),
            ),
        );
        top.insert(
            "events".to_owned(),
            Value::Array(
                self.events
                    .iter()
                    .map(|event| self.event_to_json(event))
                    .collect::<OcelResult<Vec<_>>>()?,
            ),
        );
        top.insert(
            "objects".to_owned(),
            Value::Array(
                self.objects
                    .iter()
                    .map(|object| self.object_to_json(object))
                    .collect::<OcelResult<Vec<_>>>()?,
            ),
        );

        serde_json::to_string_pretty(&Value::Object(top))
            .map_err(|err| OcelError::new(format!("could not export JSON: {err}")))
    }

    fn type_def_to_json(&self, type_def: &TypeDef) -> Value {
        json!({
            "name": self.pool.resolve(type_def.name),
            "attributes": type_def.attributes.iter().map(|attribute| {
                json!({
                    "name": self.pool.resolve(attribute.name),
                    "type": attribute.attr_type.as_str(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    fn event_to_json(&self, event: &Event) -> OcelResult<Value> {
        Ok(json!({
            "id": self.pool.resolve(event.id),
            "type": self.pool.resolve(event.type_name),
            "time": format_timestamp_ms(event.time_ms)?,
            "attributes": event.attributes.iter().map(|attribute| {
                self.attribute_to_json(attribute)
            }).collect::<OcelResult<Vec<_>>>()?,
            "relationships": self.relationships_to_json(&event.relationships),
        }))
    }

    fn object_to_json(&self, object: &Object) -> OcelResult<Value> {
        Ok(json!({
            "id": self.pool.resolve(object.id),
            "type": self.pool.resolve(object.type_name),
            "attributes": object.attributes.iter().map(|attribute| {
                self.timed_attribute_to_json(attribute)
            }).collect::<OcelResult<Vec<_>>>()?,
            "relationships": self.relationships_to_json(&object.relationships),
        }))
    }

    fn attribute_to_json(&self, attribute: &Attribute) -> OcelResult<Value> {
        Ok(json!({
            "name": self.pool.resolve(attribute.name),
            "value": self.attr_value_to_json(&attribute.value)?,
        }))
    }

    fn timed_attribute_to_json(&self, attribute: &TimedAttribute) -> OcelResult<Value> {
        Ok(json!({
            "name": self.pool.resolve(attribute.name),
            "time": format_timestamp_ms(attribute.time_ms)?,
            "value": self.attr_value_to_json(&attribute.value)?,
        }))
    }

    fn relationships_to_json(&self, relationships: &[Relationship]) -> Value {
        Value::Array(
            relationships
                .iter()
                .map(|relationship| {
                    json!({
                        "objectId": self.pool.resolve(relationship.object_id),
                        "qualifier": self.pool.resolve(relationship.qualifier),
                    })
                })
                .collect(),
        )
    }

    fn attr_value_to_json(&self, value: &AttrValue) -> OcelResult<Value> {
        match value {
            AttrValue::String(symbol) => Ok(Value::String(self.pool.resolve(*symbol).to_owned())),
            AttrValue::Time(ms) => Ok(Value::String(format_timestamp_ms(*ms)?)),
            AttrValue::Integer(number) => Ok(Value::Number(Number::from(*number))),
            AttrValue::Float(number) => Number::from_f64(*number)
                .map(Value::Number)
                .ok_or_else(|| OcelError::new("cannot export non-finite float value")),
            AttrValue::Boolean(value) => Ok(Value::Bool(*value)),
        }
    }
}
