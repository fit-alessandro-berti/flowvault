impl StateEvalIndex {
    fn with_quantiles(
        log: &CompactOcelLog,
        fields: ReferencedFields,
        leading_type: Symbol,
    ) -> OcelResult<Self> {
        let quantile_fields = fields.quantile_fields.clone();
        let mut index = Self::for_fields(log, fields);
        for field in quantile_fields {
            index.validate_quantile_field(log, &field, leading_type)?;
            let mut samples = Vec::new();
            for (event_index, event) in log.events.iter().enumerate() {
                // A repeated E2O link with another qualifier is still one observation.
                let mut seen = HashSet::new();
                for relationship in &event.relationships {
                    let Some(&object_index) = log.object_index.get(&relationship.object_id) else {
                        continue;
                    };
                    if log.objects[object_index].type_name != leading_type
                        || !seen.insert(object_index)
                    {
                        continue;
                    }
                    let context = EvalContext {
                        log,
                        eval_index: &index,
                        event_index,
                        object_index: Some(object_index),
                    };
                    if let Some(number) = context
                        .eval_field(&field)
                        .as_ref()
                        .and_then(query_value_as_number)
                    {
                        samples.push(number);
                    }
                    // Event fields get one sample per eligible event, not per relationship.
                    if matches!(field, FieldRef::Event(_)) {
                        break;
                    }
                }
            }
            samples.sort_by(f64::total_cmp);
            index.quantile_values.insert(field, samples);
        }
        Ok(index)
    }

    fn validate_quantile_field(
        &self,
        log: &CompactOcelLog,
        field: &FieldRef,
        leading_type: Symbol,
    ) -> OcelResult<()> {
        let (namespace, name, exists) = match field {
            FieldRef::Event(name) => (
                "event",
                name,
                matches!(
                    name.as_str(),
                    "id" | "type" | "activity" | "time" | "timestamp"
                ) || log.event_types.iter().any(|kind| {
                    kind.attributes
                        .iter()
                        .any(|attr| log.pool.resolve(attr.name).eq_ignore_ascii_case(name))
                }),
            ),
            FieldRef::Object(name) => (
                "object",
                name,
                matches!(name.as_str(), "id" | "type")
                    || log
                        .object_types
                        .iter()
                        .filter(|kind| kind.name == leading_type)
                        .any(|kind| {
                            kind.attributes
                                .iter()
                                .any(|attr| log.pool.resolve(attr.name).eq_ignore_ascii_case(name))
                        }),
            ),
        };
        if exists {
            Ok(())
        } else {
            Err(OcelError::new(format!(
                "unknown {namespace} field '{name}' in QUANTILE"
            )))
        }
    }
}

/// Linear interpolation between order statistics at position (n - 1) * p.
/// Empty numeric populations are missing, just like an absent attribute.
fn interpolated_quantile(sorted: &[f64], probability: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let position = (sorted.len() - 1) as f64 * probability;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position.fract();
    if lower == upper || sorted[lower] == sorted[upper] {
        return Some(sorted[lower]);
    }
    // Weighted endpoints avoid overflowing high - low for large opposite signs.
    Some(sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction)
}
