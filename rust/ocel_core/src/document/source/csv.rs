#[derive(Debug, Clone, PartialEq)]
enum CsvScalar {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug)]
struct CsvPendingEvent {
    id: String,
    type_name: String,
    time_micros: i64,
    attributes: Vec<(String, CsvScalar)>,
    relationships: Vec<SourceRelationship>,
}

#[derive(Debug)]
struct CsvPendingAssignment {
    object_id: String,
    name: String,
    time_micros: i64,
    value: CsvScalar,
    sequence: usize,
}

#[derive(Debug)]
struct CsvParsedReference {
    object_id: String,
    qualifier: Option<String>,
    attributes: Vec<(String, Option<CsvScalar>)>,
}

fn parse_csv(input: &str) -> OcelResult<SourceLog> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(input.as_bytes());
    let headers = reader
        .headers()
        .map_err(|err| OcelError::new(format!("invalid OCEL CSV header: {err}")))?
        .clone();
    if headers.is_empty() {
        return Err(OcelError::new("OCEL CSV must contain a header row"));
    }
    if headers.get(0) != Some("id")
        || headers.get(1) != Some("activity")
        || headers.get(2) != Some("timestamp")
    {
        return Err(OcelError::new(
            "OCEL CSV must start with the columns 'id', 'activity', and 'timestamp' in that order",
        ));
    }

    let mut header_indexes = HashMap::new();
    for (index, header) in headers.iter().enumerate() {
        if header_indexes.insert(header.to_owned(), index).is_some() {
            return Err(OcelError::new(format!(
                "duplicate OCEL CSV column '{header}'"
            )));
        }
    }
    let id_index = csv_required_column(&header_indexes, "id")?;
    let activity_index = csv_required_column(&header_indexes, "activity")?;
    let timestamp_index = csv_required_column(&header_indexes, "timestamp")?;

    let object_columns = headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| {
            header
                .strip_prefix("ot:")
                .map(|type_name| (index, type_name.to_owned()))
        })
        .collect::<Vec<_>>();
    if object_columns
        .iter()
        .any(|(_, type_name)| type_name.is_empty())
    {
        return Err(OcelError::new(
            "OCEL CSV object type columns must use a non-empty 'ot:<type>' name",
        ));
    }
    let event_attribute_columns = headers
        .iter()
        .enumerate()
        .filter(|(index, header)| {
            *index != id_index
                && *index != activity_index
                && *index != timestamp_index
                && !header.starts_with("ot:")
        })
        .map(|(index, header)| (index, header.to_owned()))
        .collect::<Vec<_>>();

    let mut event_type_order = Vec::new();
    let mut seen_event_types = HashSet::new();
    let object_type_order = object_columns
        .iter()
        .map(|(_, type_name)| type_name.clone())
        .collect::<Vec<_>>();
    let mut object_type_by_id = HashMap::<String, String>::new();
    let mut object_order = Vec::<String>::new();
    let mut events = Vec::<CsvPendingEvent>::new();
    let mut object_relationships = HashMap::<String, Vec<SourceRelationship>>::new();
    let mut assignments = Vec::<CsvPendingAssignment>::new();
    let mut assignment_values = HashMap::<(String, String, i64), CsvScalar>::new();
    let mut object_attribute_order = HashMap::<String, Vec<String>>::new();
    let mut assignment_sequence = 0usize;

    for (record_index, record_result) in reader.records().enumerate() {
        let row_number = record_index + 2;
        let record = record_result.map_err(|err| {
            OcelError::new(format!("invalid OCEL CSV row {row_number}: {err}"))
        })?;
        let id = record.get(id_index).unwrap_or("").trim().to_owned();
        let activity = record
            .get(activity_index)
            .unwrap_or("")
            .trim()
            .to_owned();
        let timestamp = record
            .get(timestamp_index)
            .unwrap_or("")
            .trim()
            .to_owned();

        let is_o2o = activity.eq_ignore_ascii_case("o2o");
        let row_kind = if !id.is_empty()
            && !activity.is_empty()
            && !timestamp.is_empty()
            && !is_o2o
        {
            CsvRowKind::Event
        } else if !id.is_empty() && is_o2o {
            CsvRowKind::ObjectRelationship
        } else if id.is_empty() && activity.is_empty() && timestamp.is_empty() {
            CsvRowKind::ObjectDeclaration
        } else if id.is_empty() && activity.is_empty() && !timestamp.is_empty() {
            CsvRowKind::ObjectAttribute
        } else {
            return Err(OcelError::new(format!(
                "invalid OCEL CSV row {row_number}: unsupported id/activity/timestamp combination"
            )));
        };

        let time_micros = match row_kind {
            CsvRowKind::Event | CsvRowKind::ObjectAttribute => {
                parse_csv_timestamp(&timestamp, row_number)?
            }
            CsvRowKind::ObjectRelationship if !timestamp.is_empty() => {
                parse_csv_timestamp(&timestamp, row_number)?
            }
            CsvRowKind::ObjectRelationship | CsvRowKind::ObjectDeclaration => 0,
        };

        if row_kind == CsvRowKind::ObjectRelationship
            && !object_type_by_id.contains_key(&id)
        {
            return Err(OcelError::new(format!(
                "invalid OCEL CSV row {row_number}: source object '{id}' has not been declared by an earlier object reference"
            )));
        }

        let mut parsed_cells = Vec::with_capacity(object_columns.len());
        for (column_index, type_name) in &object_columns {
            let cell = record.get(*column_index).unwrap_or("");
            let references = parse_csv_reference_cell(cell).map_err(|err| {
                OcelError::new(format!(
                    "invalid OCEL CSV row {row_number}, object type '{type_name}': {err}"
                ))
            })?;
            parsed_cells.push((type_name.clone(), references));
        }

        let mut event_relationships = Vec::new();
        for (type_name, references) in parsed_cells {
            for reference in references {
                csv_declare_object(
                    &reference.object_id,
                    &type_name,
                    &mut object_type_by_id,
                    &mut object_order,
                    row_number,
                )?;

                match row_kind {
                    CsvRowKind::Event => event_relationships.push(SourceRelationship {
                        object_id: reference.object_id.clone(),
                        qualifier: reference.qualifier.clone().unwrap_or_default(),
                    }),
                    CsvRowKind::ObjectRelationship => {
                        object_relationships
                            .entry(id.clone())
                            .or_default()
                            .push(SourceRelationship {
                                object_id: reference.object_id.clone(),
                                qualifier: reference.qualifier.clone().unwrap_or_default(),
                            });
                    }
                    CsvRowKind::ObjectDeclaration => {
                        if reference.qualifier.is_some() {
                            return Err(OcelError::new(format!(
                                "invalid OCEL CSV row {row_number}: object declarations cannot contain qualifiers"
                            )));
                        }
                    }
                    CsvRowKind::ObjectAttribute => {}
                }

                let has_attributes = reference
                    .attributes
                    .iter()
                    .any(|(_, value)| value.is_some());
                if row_kind == CsvRowKind::ObjectAttribute && !has_attributes {
                    return Err(OcelError::new(format!(
                        "invalid OCEL CSV row {row_number}: object attribute references must contain JSON attributes"
                    )));
                }
                if row_kind == CsvRowKind::ObjectRelationship
                    && has_attributes
                    && timestamp.is_empty()
                {
                    return Err(OcelError::new(format!(
                        "invalid OCEL CSV row {row_number}: object-to-object rows carrying attributes require a timestamp"
                    )));
                }

                for (name, value) in reference.attributes {
                    let Some(value) = value else {
                        continue;
                    };
                    let attribute_time = if row_kind == CsvRowKind::ObjectDeclaration {
                        0
                    } else {
                        time_micros
                    };
                    csv_add_assignment(
                        &reference.object_id,
                        &type_name,
                        name,
                        value,
                        attribute_time,
                        assignment_sequence,
                        &mut assignments,
                        &mut assignment_values,
                        &mut object_attribute_order,
                        row_number,
                    )?;
                    assignment_sequence += 1;
                }
            }
        }

        if row_kind == CsvRowKind::Event {
            if activity.eq_ignore_ascii_case("o2o") {
                return Err(OcelError::new(
                    "the event type 'o2o' is not representable in OCEL CSV",
                ));
            }
            if seen_event_types.insert(activity.clone()) {
                event_type_order.push(activity.clone());
            }
            let attributes = event_attribute_columns
                .iter()
                .filter_map(|(column_index, name)| {
                    let value = record.get(*column_index).unwrap_or("");
                    (!value.is_empty()).then(|| (name.clone(), CsvScalar::Text(value.to_owned())))
                })
                .collect();
            events.push(CsvPendingEvent {
                id,
                type_name: activity,
                time_micros,
                attributes,
                relationships: event_relationships,
            });
        } else if !event_attribute_columns
            .iter()
            .all(|(column_index, _)| record.get(*column_index).unwrap_or("").is_empty())
        {
            return Err(OcelError::new(format!(
                "invalid OCEL CSV row {row_number}: event attribute columns are only valid on event rows"
            )));
        }
    }

    let mut event_attr_types = HashMap::<(String, String), AttrType>::new();
    let event_types = event_type_order
        .iter()
        .map(|type_name| {
            let attributes = event_attribute_columns
                .iter()
                .filter_map(|(_, name)| {
                    let values = events
                        .iter()
                        .filter(|event| &event.type_name == type_name)
                        .flat_map(|event| event.attributes.iter())
                        .filter(|(attribute_name, _)| attribute_name == name)
                        .map(|(_, value)| value)
                        .collect::<Vec<_>>();
                    if values.is_empty() {
                        return None;
                    }
                    let attr_type = infer_csv_attr_type(&values);
                    event_attr_types.insert((type_name.clone(), name.clone()), attr_type);
                    Some(SourceAttributeDef {
                        name: name.clone(),
                        attr_type: attr_type.as_str().to_owned(),
                    })
                })
                .collect();
            SourceType {
                name: type_name.clone(),
                attributes,
            }
        })
        .collect::<Vec<_>>();

    let mut object_attr_types = HashMap::<(String, String), AttrType>::new();
    let object_types = object_type_order
        .iter()
        .map(|type_name| {
            let attributes = object_attribute_order
                .get(type_name)
                .into_iter()
                .flatten()
                .map(|name| {
                    let values = assignments
                        .iter()
                        .filter(|assignment| {
                            object_type_by_id.get(&assignment.object_id) == Some(type_name)
                                && &assignment.name == name
                        })
                        .map(|assignment| &assignment.value)
                        .collect::<Vec<_>>();
                    let attr_type = infer_csv_attr_type(&values);
                    object_attr_types.insert((type_name.clone(), name.clone()), attr_type);
                    SourceAttributeDef {
                        name: name.clone(),
                        attr_type: attr_type.as_str().to_owned(),
                    }
                })
                .collect();
            SourceType {
                name: type_name.clone(),
                attributes,
            }
        })
        .collect::<Vec<_>>();

    assignments.sort_by_key(|assignment| (assignment.time_micros, assignment.sequence));
    let source_events = events
        .into_iter()
        .map(|event| {
            let type_name = event.type_name.clone();
            SourceEvent {
                id: event.id,
                type_name: event.type_name,
                time: format_timestamp_micros(event.time_micros)
                    .expect("validated CSV timestamp remains representable"),
                attributes: event
                    .attributes
                    .into_iter()
                    .map(|(name, value)| {
                        let attr_type = event_attr_types[&(type_name.clone(), name.clone())];
                        SourceAttribute {
                            name,
                            value: csv_scalar_to_source(value, attr_type),
                        }
                    })
                    .collect(),
                relationships: event.relationships,
            }
        })
        .collect();
    let source_objects = object_order
        .into_iter()
        .map(|object_id| {
            let type_name = object_type_by_id[&object_id].clone();
            SourceObject {
                id: object_id.clone(),
                type_name: type_name.clone(),
                attributes: assignments
                    .iter()
                    .filter(|assignment| assignment.object_id == object_id)
                    .map(|assignment| {
                        let attr_type =
                            object_attr_types[&(type_name.clone(), assignment.name.clone())];
                        SourceTimedAttribute {
                            name: assignment.name.clone(),
                            time: format_timestamp_micros(assignment.time_micros)
                                .expect("validated CSV timestamp remains representable"),
                            value: csv_scalar_to_source(assignment.value.clone(), attr_type),
                        }
                    })
                    .collect(),
                relationships: object_relationships.remove(&object_id).unwrap_or_default(),
            }
        })
        .collect();

    Ok(SourceLog {
        event_types,
        object_types,
        events: source_events,
        objects: source_objects,
    })
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CsvRowKind {
    Event,
    ObjectRelationship,
    ObjectDeclaration,
    ObjectAttribute,
}

fn csv_required_column(indexes: &HashMap<String, usize>, name: &str) -> OcelResult<usize> {
    indexes
        .get(name)
        .copied()
        .ok_or_else(|| OcelError::new(format!("OCEL CSV is missing required column '{name}'")))
}

fn parse_csv_timestamp(value: &str, row_number: usize) -> OcelResult<i64> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.timestamp_micros())
        .map_err(|err| {
            OcelError::new(format!(
                "invalid OCEL CSV timestamp on row {row_number}: '{value}' ({err})"
            ))
        })
}

fn csv_declare_object(
    object_id: &str,
    type_name: &str,
    object_type_by_id: &mut HashMap<String, String>,
    object_order: &mut Vec<String>,
    row_number: usize,
) -> OcelResult<()> {
    if let Some(existing_type) = object_type_by_id.get(object_id) {
        if existing_type != type_name {
            return Err(OcelError::new(format!(
                "invalid OCEL CSV row {row_number}: object '{object_id}' appears as both '{existing_type}' and '{type_name}'"
            )));
        }
    } else {
        object_type_by_id.insert(object_id.to_owned(), type_name.to_owned());
        object_order.push(object_id.to_owned());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn csv_add_assignment(
    object_id: &str,
    type_name: &str,
    name: String,
    value: CsvScalar,
    time_micros: i64,
    sequence: usize,
    assignments: &mut Vec<CsvPendingAssignment>,
    assignment_values: &mut HashMap<(String, String, i64), CsvScalar>,
    object_attribute_order: &mut HashMap<String, Vec<String>>,
    row_number: usize,
) -> OcelResult<()> {
    let key = (object_id.to_owned(), name.clone(), time_micros);
    if let Some(existing) = assignment_values.get(&key) {
        if existing != &value {
            return Err(OcelError::new(format!(
                "invalid OCEL CSV row {row_number}: conflicting values for object '{object_id}' attribute '{name}' at the same timestamp"
            )));
        }
        return Ok(());
    }
    assignment_values.insert(key, value.clone());
    let names = object_attribute_order
        .entry(type_name.to_owned())
        .or_default();
    if !names.contains(&name) {
        names.push(name.clone());
    }
    assignments.push(CsvPendingAssignment {
        object_id: object_id.to_owned(),
        name,
        time_micros,
        value,
        sequence,
    });
    Ok(())
}

fn parse_csv_reference_cell(cell: &str) -> OcelResult<Vec<CsvParsedReference>> {
    if cell.is_empty() {
        return Ok(Vec::new());
    }
    split_csv_references(cell)?
        .into_iter()
        .map(parse_csv_reference)
        .collect()
}

fn split_csv_references(cell: &str) -> OcelResult<Vec<&str>> {
    let mut references = Vec::new();
    let mut start = 0usize;
    let mut escaped = false;
    let mut json_depth = 0i32;
    let mut json_string = false;
    let mut json_escaped = false;

    for (index, character) in cell.char_indices() {
        if json_depth > 0 {
            if json_string {
                if json_escaped {
                    json_escaped = false;
                } else if character == '\\' {
                    json_escaped = true;
                } else if character == '"' {
                    json_string = false;
                }
            } else {
                match character {
                    '"' => json_string = true,
                    '{' | '[' => json_depth += 1,
                    '}' | ']' => json_depth -= 1,
                    _ => {}
                }
            }
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '{' => json_depth = 1,
            '/' => {
                if index == start {
                    return Err(OcelError::new("empty object reference"));
                }
                references.push(&cell[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if json_depth != 0 || json_string {
        return Err(OcelError::new("unterminated JSON object in object reference"));
    }
    if start == cell.len() {
        return Err(OcelError::new("empty object reference"));
    }
    references.push(&cell[start..]);
    Ok(references)
}

fn parse_csv_reference(reference: &str) -> OcelResult<CsvParsedReference> {
    let json_index = first_unescaped(reference, '{');
    let prefix_end = json_index.unwrap_or(reference.len());
    let prefix = &reference[..prefix_end];
    let qualifier_index = first_unescaped(prefix, '#');
    let (raw_object_id, raw_qualifier) = qualifier_index.map_or((prefix, None), |index| {
        (&prefix[..index], Some(&prefix[index + 1..]))
    });
    let object_id = unescape_csv_reference_part(raw_object_id)?.trim().to_owned();
    if object_id.is_empty() {
        return Err(OcelError::new("object reference has an empty object id"));
    }
    let qualifier = raw_qualifier
        .map(unescape_csv_reference_part)
        .transpose()?
        .map(|value| value.trim().to_owned());
    if qualifier.as_deref() == Some("") {
        return Err(OcelError::new(
            "object reference qualifier cannot be empty after '#'",
        ));
    }

    let attributes = if let Some(index) = json_index {
        let value = serde_json::from_str::<Value>(&reference[index..])
            .map_err(|err| OcelError::new(format!("invalid object attribute JSON: {err}")))?;
        let Value::Object(attributes) = value else {
            return Err(OcelError::new("object reference attributes must be a JSON object"));
        };
        attributes
            .into_iter()
            .map(|(name, value)| {
                let value = match value {
                    Value::Null => None,
                    Value::String(value) => Some(CsvScalar::Text(value)),
                    Value::Bool(value) => Some(CsvScalar::Boolean(value)),
                    Value::Number(value) => {
                        if let Some(value) = value.as_i64() {
                            Some(CsvScalar::Integer(value))
                        } else if let Some(value) = value.as_f64().filter(|value| value.is_finite()) {
                            Some(CsvScalar::Float(value))
                        } else {
                            return Err(OcelError::new(
                                "object attributes must contain finite JSON numbers",
                            ));
                        }
                    }
                    Value::Array(_) | Value::Object(_) => {
                        return Err(OcelError::new(
                            "object attribute values must be scalar JSON values",
                        ));
                    }
                };
                Ok((name, value))
            })
            .collect::<OcelResult<Vec<_>>>()?
    } else {
        Vec::new()
    };

    Ok(CsvParsedReference {
        object_id,
        qualifier,
        attributes,
    })
}

fn first_unescaped(value: &str, needle: char) -> Option<usize> {
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == needle {
            return Some(index);
        }
    }
    None
}

fn unescape_csv_reference_part(value: &str) -> OcelResult<String> {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters
            .next()
            .ok_or_else(|| OcelError::new("trailing backslash in object reference"))?;
        if !matches!(escaped, '/' | '#' | '{' | '\\') {
            return Err(OcelError::new(format!(
                "invalid object reference escape '\\{escaped}'"
            )));
        }
        output.push(escaped);
    }
    Ok(output)
}

fn infer_csv_attr_type(values: &[&CsvScalar]) -> AttrType {
    if values.iter().all(|value| csv_scalar_as_integer(value).is_some()) {
        return AttrType::Integer;
    }
    if values.iter().all(|value| csv_scalar_as_float(value).is_some()) {
        return AttrType::Float;
    }
    if values.iter().all(|value| csv_scalar_as_boolean(value).is_some()) {
        return AttrType::Boolean;
    }
    if values.iter().all(|value| csv_scalar_as_time(value).is_some()) {
        return AttrType::Time;
    }
    AttrType::String
}

fn csv_scalar_as_integer(value: &CsvScalar) -> Option<i64> {
    match value {
        CsvScalar::Integer(value) => Some(*value),
        CsvScalar::Text(value) if canonical_integer_text(value) => value.parse().ok(),
        CsvScalar::Text(_) | CsvScalar::Float(_) | CsvScalar::Boolean(_) => None,
    }
}

fn canonical_integer_text(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let digits = if bytes[0] == b'-' {
        if bytes.len() == 1 {
            return false;
        }
        &bytes[1..]
    } else {
        bytes
    };
    digits.iter().all(u8::is_ascii_digit)
        && (digits.len() == 1 || digits[0] != b'0')
        && value.parse::<i64>().is_ok()
}

fn csv_scalar_as_float(value: &CsvScalar) -> Option<f64> {
    let parsed = match value {
        CsvScalar::Integer(value) => {
            let parsed = *value as f64;
            (parsed as i64 == *value).then_some(parsed)?
        }
        CsvScalar::Float(value) => *value,
        CsvScalar::Text(value) if canonical_integer_text(value) => {
            let integer = value.parse::<i64>().ok()?;
            let parsed = integer as f64;
            (parsed as i64 == integer).then_some(parsed)?
        }
        CsvScalar::Text(value) => {
            serde_json::from_str::<Number>(value)
                .ok()?
                .as_f64()
                .filter(|value| value.is_finite())?
        }
        CsvScalar::Boolean(_) => return None,
    };
    parsed.is_finite().then_some(parsed)
}

fn csv_scalar_as_boolean(value: &CsvScalar) -> Option<bool> {
    match value {
        CsvScalar::Boolean(value) => Some(*value),
        CsvScalar::Text(value) if value.eq_ignore_ascii_case("true") => Some(true),
        CsvScalar::Text(value) if value.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

fn csv_scalar_as_time(value: &CsvScalar) -> Option<String> {
    let CsvScalar::Text(value) = value else {
        return None;
    };
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|_| value.clone())
}

fn csv_scalar_to_source(value: CsvScalar, attr_type: AttrType) -> SourceValue {
    match attr_type {
        AttrType::String => SourceValue::String(match value {
            CsvScalar::Text(value) => value,
            CsvScalar::Integer(value) => value.to_string(),
            CsvScalar::Float(value) => value.to_string(),
            CsvScalar::Boolean(value) => value.to_string(),
        }),
        AttrType::Time => SourceValue::String(csv_scalar_as_time(&value).unwrap()),
        AttrType::Integer => SourceValue::Integer(csv_scalar_as_integer(&value).unwrap()),
        AttrType::Float => SourceValue::Float(csv_scalar_as_float(&value).unwrap()),
        AttrType::Boolean => SourceValue::Boolean(csv_scalar_as_boolean(&value).unwrap()),
    }
}
