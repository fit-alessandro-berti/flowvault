fn parse_sqlite(input: &[u8]) -> OcelResult<SourceLog> {
    if !input.starts_with(b"SQLite format 3\0") {
        return Err(OcelError::new(
            "invalid OCEL SQLite file: missing SQLite database header",
        ));
    }
    let mut connection = rusqlite::Connection::open_in_memory()
        .map_err(|err| OcelError::new(format!("could not open in-memory SQLite database: {err}")))?;
    connection
        .deserialize_read_exact("main", input, input.len(), true)
        .map_err(|err| OcelError::new(format!("could not read OCEL SQLite database: {err}")))?;
    parse_sqlite_connection(&connection)
}

fn parse_sqlite_connection(connection: &rusqlite::Connection) -> OcelResult<SourceLog> {
    let event_maps = sqlite_type_maps(connection, "event_map_type")?;
    let object_maps = sqlite_type_maps(connection, "object_map_type")?;
    let event_map_lookup = event_maps.iter().cloned().collect::<HashMap<_, _>>();
    let object_map_lookup = object_maps.iter().cloned().collect::<HashMap<_, _>>();

    let event_types = event_maps
        .iter()
        .map(|(type_name, mapped_name)| {
            sqlite_source_type(connection, "event", type_name, mapped_name)
        })
        .collect::<OcelResult<Vec<_>>>()?;
    let object_types = object_maps
        .iter()
        .map(|(type_name, mapped_name)| {
            sqlite_source_type(connection, "object", type_name, mapped_name)
        })
        .collect::<OcelResult<Vec<_>>>()?;

    let base_events = sqlite_base_entities(connection, "event")?;
    let base_objects = sqlite_base_entities(connection, "object")?;
    let event_ids = base_events
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<HashSet<_>>();
    let object_ids = base_objects
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<HashSet<_>>();

    let event_relationships =
        sqlite_event_relationships(connection, &event_ids, &object_ids)?;
    let object_relationships = sqlite_object_relationships(connection, &object_ids)?;

    let event_type_defs = event_types
        .iter()
        .map(|type_def| (type_def.name.clone(), type_def))
        .collect::<HashMap<_, _>>();
    let mut event_details = HashMap::<String, (String, Vec<SourceAttribute>)>::new();
    for (type_name, mapped_name) in &event_maps {
        let type_def = event_type_defs[type_name];
        let table_name = format!("event_{mapped_name}");
        let mut selected_columns = vec!["ocel_id".to_owned(), "ocel_time".to_owned()];
        selected_columns.extend(type_def.attributes.iter().map(|attribute| attribute.name.clone()));
        let sql = sqlite_select_columns(&table_name, &selected_columns)?;
        let mut statement = connection.prepare(&sql).map_err(|err| {
            OcelError::new(format!(
                "invalid OCEL SQLite event table '{table_name}': {err}"
            ))
        })?;
        let mut rows = statement.query([]).map_err(sqlite_import_error)?;
        while let Some(row) = rows.next().map_err(sqlite_import_error)? {
            let id = sqlite_required_text(row.get_ref(0).map_err(sqlite_import_error)?, "ocel_id")?;
            let time = sqlite_required_timestamp(
                row.get_ref(1).map_err(sqlite_import_error)?,
                "ocel_time",
            )?;
            let mut attributes = Vec::new();
            for (attribute_index, attribute) in type_def.attributes.iter().enumerate() {
                let attr_type = AttrType::parse(&attribute.attr_type)?;
                if let Some(value) = sqlite_source_value(
                    row.get_ref(attribute_index + 2)
                        .map_err(sqlite_import_error)?,
                    attr_type,
                    &attribute.name,
                )? {
                    attributes.push(SourceAttribute {
                        name: attribute.name.clone(),
                        value,
                    });
                }
            }
            if event_details
                .insert(id.clone(), (time, attributes))
                .is_some()
            {
                return Err(OcelError::new(format!(
                    "duplicate event id '{id}' in OCEL SQLite type tables"
                )));
            }
        }
    }

    let events = base_events
        .into_iter()
        .map(|(id, type_name)| {
            if !event_map_lookup.contains_key(&type_name) {
                return Err(OcelError::new(format!(
                    "event '{id}' references unknown event type '{type_name}'"
                )));
            }
            let (time, attributes) = event_details.remove(&id).ok_or_else(|| {
                OcelError::new(format!(
                    "event '{id}' is missing from its OCEL SQLite type table"
                ))
            })?;
            Ok(SourceEvent {
                id: id.clone(),
                type_name,
                time,
                attributes,
                relationships: event_relationships.get(&id).cloned().unwrap_or_default(),
            })
        })
        .collect::<OcelResult<Vec<_>>>()?;
    if let Some(extra_id) = event_details.keys().next() {
        return Err(OcelError::new(format!(
            "event '{extra_id}' exists in a type table but not in the general event table"
        )));
    }

    let object_type_defs = object_types
        .iter()
        .map(|type_def| (type_def.name.clone(), type_def))
        .collect::<HashMap<_, _>>();
    let base_object_types = base_objects.iter().cloned().collect::<HashMap<_, _>>();
    let mut object_attributes = HashMap::<String, Vec<(i64, usize, SourceTimedAttribute)>>::new();
    let mut object_assignment_values =
        HashMap::<(String, String, i64), SourceValue>::new();
    let mut sequence = 0usize;

    for (type_name, mapped_name) in &object_maps {
        let type_def = object_type_defs[type_name];
        let table_name = format!("object_{mapped_name}");
        let table_columns = sqlite_table_columns(connection, &table_name)?;
        let has_time = table_columns.iter().any(|column| column.name == "ocel_time");
        let has_changed = table_columns
            .iter()
            .any(|column| column.name == "ocel_changed_field");
        let mut selected_columns = vec!["ocel_id".to_owned()];
        if has_time {
            selected_columns.push("ocel_time".to_owned());
        }
        if has_changed {
            selected_columns.push("ocel_changed_field".to_owned());
        }
        selected_columns.extend(type_def.attributes.iter().map(|attribute| attribute.name.clone()));
        let sql = sqlite_select_columns(&table_name, &selected_columns)?;
        let mut statement = connection.prepare(&sql).map_err(|err| {
            OcelError::new(format!(
                "invalid OCEL SQLite object table '{table_name}': {err}"
            ))
        })?;
        let mut rows = statement.query([]).map_err(sqlite_import_error)?;
        while let Some(row) = rows.next().map_err(sqlite_import_error)? {
            let id = sqlite_required_text(row.get_ref(0).map_err(sqlite_import_error)?, "ocel_id")?;
            let Some(base_type) = base_object_types.get(&id) else {
                return Err(OcelError::new(format!(
                    "object '{id}' exists in '{table_name}' but not in the general object table"
                )));
            };
            if base_type != type_name {
                return Err(OcelError::new(format!(
                    "object '{id}' is stored in object type table '{type_name}' but declared as '{base_type}'"
                )));
            }

            let mut column_index = 1usize;
            let time = if has_time {
                let value = row.get_ref(column_index).map_err(sqlite_import_error)?;
                column_index += 1;
                sqlite_optional_timestamp(value, "ocel_time")?
            } else {
                None
            };
            let changed_field = if has_changed {
                let value = row.get_ref(column_index).map_err(sqlite_import_error)?;
                column_index += 1;
                sqlite_optional_text(value, "ocel_changed_field")?
            } else {
                None
            };

            // Some widespread exporters add a null-time snapshot row in addition to the
            // standard history rows. It carries no new assignment and is ignored.
            let Some(time) = time else {
                continue;
            };
            let time_micros = parse_timestamp_micros(&time)?;
            let mut row_values = Vec::new();
            for (attribute_index, attribute) in type_def.attributes.iter().enumerate() {
                let attr_type = AttrType::parse(&attribute.attr_type)?;
                let value = sqlite_source_value(
                    row.get_ref(column_index + attribute_index)
                        .map_err(sqlite_import_error)?,
                    attr_type,
                    &attribute.name,
                )?;
                row_values.push((attribute, value));
            }

            if let Some(changed_field) = changed_field {
                let Some((attribute, Some(value))) = row_values
                    .iter()
                    .find(|(attribute, _)| attribute.name == changed_field)
                else {
                    return Err(OcelError::new(format!(
                        "object '{id}' change row names unknown or null field '{changed_field}'"
                    )));
                };
                sqlite_add_object_assignment(
                    &id,
                    &attribute.name,
                    time_micros,
                    (*value).clone(),
                    sequence,
                    &mut object_attributes,
                    &mut object_assignment_values,
                )?;
                sequence += 1;
            } else {
                if time_micros != 0 && row_values.iter().any(|(_, value)| value.is_some()) {
                    return Err(OcelError::new(format!(
                        "object '{id}' has a non-initial attribute row without ocel_changed_field"
                    )));
                }
                for (attribute, value) in row_values {
                    if let Some(value) = value {
                        sqlite_add_object_assignment(
                            &id,
                            &attribute.name,
                            time_micros,
                            value,
                            sequence,
                            &mut object_attributes,
                            &mut object_assignment_values,
                        )?;
                        sequence += 1;
                    }
                }
            }
        }
    }

    for attributes in object_attributes.values_mut() {
        attributes.sort_by_key(|(time, sequence, _)| (*time, *sequence));
    }
    let objects = base_objects
        .into_iter()
        .map(|(id, type_name)| {
            if !object_map_lookup.contains_key(&type_name) {
                return Err(OcelError::new(format!(
                    "object '{id}' references unknown object type '{type_name}'"
                )));
            }
            Ok(SourceObject {
                id: id.clone(),
                type_name,
                attributes: object_attributes
                    .remove(&id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(_, _, attribute)| attribute)
                    .collect(),
                relationships: object_relationships.get(&id).cloned().unwrap_or_default(),
            })
        })
        .collect::<OcelResult<Vec<_>>>()?;

    Ok(SourceLog {
        event_types,
        object_types,
        events,
        objects,
    })
}

#[derive(Debug)]
struct SqliteColumn {
    name: String,
    declaration: String,
}

fn sqlite_type_maps(
    connection: &rusqlite::Connection,
    table_name: &str,
) -> OcelResult<Vec<(String, String)>> {
    let sql = format!(
        "SELECT \"ocel_type\", \"ocel_type_map\" FROM {}",
        quote_sql_identifier(table_name)?
    );
    let mut statement = connection.prepare(&sql).map_err(|err| {
        OcelError::new(format!(
            "invalid OCEL SQLite database: cannot read '{table_name}': {err}"
        ))
    })?;
    let mut rows = statement.query([]).map_err(sqlite_import_error)?;
    let mut result = Vec::new();
    let mut types = HashSet::new();
    let mut mappings = HashSet::new();
    while let Some(row) = rows.next().map_err(sqlite_import_error)? {
        let type_name = sqlite_required_text(row.get_ref(0).map_err(sqlite_import_error)?, "ocel_type")?;
        let mapped_name = sqlite_required_text(
            row.get_ref(1).map_err(sqlite_import_error)?,
            "ocel_type_map",
        )?;
        if !types.insert(type_name.clone()) {
            return Err(OcelError::new(format!(
                "duplicate OCEL SQLite type '{type_name}' in '{table_name}'"
            )));
        }
        if !mappings.insert(mapped_name.clone()) {
            return Err(OcelError::new(format!(
                "duplicate OCEL SQLite type mapping '{mapped_name}' in '{table_name}'"
            )));
        }
        result.push((type_name, mapped_name));
    }
    Ok(result)
}

fn sqlite_source_type(
    connection: &rusqlite::Connection,
    prefix: &str,
    type_name: &str,
    mapped_name: &str,
) -> OcelResult<SourceType> {
    let table_name = format!("{prefix}_{mapped_name}");
    let columns = sqlite_table_columns(connection, &table_name)?;
    if !columns.iter().any(|column| column.name == "ocel_id") {
        return Err(OcelError::new(format!(
            "OCEL SQLite table '{table_name}' is missing required column 'ocel_id'"
        )));
    }
    if prefix == "event" && !columns.iter().any(|column| column.name == "ocel_time") {
        return Err(OcelError::new(format!(
            "OCEL SQLite event table '{table_name}' is missing required column 'ocel_time'"
        )));
    }
    let attributes = columns
        .into_iter()
        .filter(|column| {
            !matches!(
                column.name.as_str(),
                "ocel_id" | "ocel_time" | "ocel_changed_field" | "ocel:activity"
            )
        })
        .map(|column| {
            let attr_type = sqlite_attribute_type(&column.declaration).map_err(|message| {
                OcelError::new(format!(
                    "invalid type for OCEL SQLite column '{table_name}.{}': {message}",
                    column.name
                ))
            })?;
            Ok(SourceAttributeDef {
                name: column.name,
                attr_type: attr_type.as_str().to_owned(),
            })
        })
        .collect::<OcelResult<Vec<_>>>()?;
    Ok(SourceType {
        name: type_name.to_owned(),
        attributes,
    })
}

fn sqlite_table_columns(
    connection: &rusqlite::Connection,
    table_name: &str,
) -> OcelResult<Vec<SqliteColumn>> {
    let sql = format!("PRAGMA table_info({})", quote_sql_identifier(table_name)?);
    let mut statement = connection.prepare(&sql).map_err(sqlite_import_error)?;
    let mut rows = statement.query([]).map_err(sqlite_import_error)?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next().map_err(sqlite_import_error)? {
        columns.push(SqliteColumn {
            name: row.get(1).map_err(sqlite_import_error)?,
            declaration: row.get(2).map_err(sqlite_import_error)?,
        });
    }
    if columns.is_empty() {
        return Err(OcelError::new(format!(
            "OCEL SQLite database is missing table '{table_name}'"
        )));
    }
    Ok(columns)
}

fn sqlite_attribute_type(declaration: &str) -> Result<AttrType, String> {
    match declaration.trim().to_ascii_uppercase().as_str() {
        "TEXT" | "VARCHAR" | "CHAR" | "CLOB" => Ok(AttrType::String),
        "TIMESTAMP" | "DATETIME" => Ok(AttrType::Time),
        "INTEGER" | "INT" | "BIGINT" => Ok(AttrType::Integer),
        "REAL" | "DOUBLE" | "FLOAT" => Ok(AttrType::Float),
        "BOOLEAN" | "BOOL" => Ok(AttrType::Boolean),
        other => Err(format!("unsupported declared type '{other}'")),
    }
}

fn sqlite_base_entities(
    connection: &rusqlite::Connection,
    table_name: &str,
) -> OcelResult<Vec<(String, String)>> {
    let sql = format!(
        "SELECT \"ocel_id\", \"ocel_type\" FROM {}",
        quote_sql_identifier(table_name)?
    );
    let mut statement = connection.prepare(&sql).map_err(|err| {
        OcelError::new(format!(
            "invalid OCEL SQLite database: cannot read '{table_name}': {err}"
        ))
    })?;
    let mut rows = statement.query([]).map_err(sqlite_import_error)?;
    let mut result = Vec::new();
    let mut ids = HashSet::new();
    while let Some(row) = rows.next().map_err(sqlite_import_error)? {
        let id = sqlite_required_text(row.get_ref(0).map_err(sqlite_import_error)?, "ocel_id")?;
        let type_name = sqlite_required_text(row.get_ref(1).map_err(sqlite_import_error)?, "ocel_type")?;
        if !ids.insert(id.clone()) {
            return Err(OcelError::new(format!(
                "duplicate {table_name} id '{id}' in OCEL SQLite database"
            )));
        }
        result.push((id, type_name));
    }
    Ok(result)
}

fn sqlite_event_relationships(
    connection: &rusqlite::Connection,
    event_ids: &HashSet<String>,
    object_ids: &HashSet<String>,
) -> OcelResult<HashMap<String, Vec<SourceRelationship>>> {
    let mut statement = connection
        .prepare(
            "SELECT \"ocel_event_id\", \"ocel_object_id\", \"ocel_qualifier\" FROM \"event_object\"",
        )
        .map_err(sqlite_import_error)?;
    let mut rows = statement.query([]).map_err(sqlite_import_error)?;
    let mut relationships = HashMap::<String, Vec<SourceRelationship>>::new();
    while let Some(row) = rows.next().map_err(sqlite_import_error)? {
        let event_id = sqlite_required_text(row.get_ref(0).map_err(sqlite_import_error)?, "ocel_event_id")?;
        let object_id = sqlite_required_text(row.get_ref(1).map_err(sqlite_import_error)?, "ocel_object_id")?;
        let qualifier = sqlite_required_text(row.get_ref(2).map_err(sqlite_import_error)?, "ocel_qualifier")?;
        if !event_ids.contains(&event_id) {
            return Err(OcelError::new(format!(
                "event-object relationship references unknown event '{event_id}'"
            )));
        }
        if !object_ids.contains(&object_id) {
            return Err(OcelError::new(format!(
                "event-object relationship references unknown object '{object_id}'"
            )));
        }
        relationships
            .entry(event_id)
            .or_default()
            .push(SourceRelationship {
                object_id,
                qualifier,
            });
    }
    Ok(relationships)
}

fn sqlite_object_relationships(
    connection: &rusqlite::Connection,
    object_ids: &HashSet<String>,
) -> OcelResult<HashMap<String, Vec<SourceRelationship>>> {
    let mut statement = connection
        .prepare(
            "SELECT \"ocel_source_id\", \"ocel_target_id\", \"ocel_qualifier\" FROM \"object_object\"",
        )
        .map_err(sqlite_import_error)?;
    let mut rows = statement.query([]).map_err(sqlite_import_error)?;
    let mut relationships = HashMap::<String, Vec<SourceRelationship>>::new();
    while let Some(row) = rows.next().map_err(sqlite_import_error)? {
        let source_id = sqlite_required_text(row.get_ref(0).map_err(sqlite_import_error)?, "ocel_source_id")?;
        let target_id = sqlite_required_text(row.get_ref(1).map_err(sqlite_import_error)?, "ocel_target_id")?;
        let qualifier = sqlite_required_text(row.get_ref(2).map_err(sqlite_import_error)?, "ocel_qualifier")?;
        if !object_ids.contains(&source_id) || !object_ids.contains(&target_id) {
            return Err(OcelError::new(format!(
                "object-object relationship '{source_id}' -> '{target_id}' references an unknown object"
            )));
        }
        relationships
            .entry(source_id)
            .or_default()
            .push(SourceRelationship {
                object_id: target_id,
                qualifier,
            });
    }
    Ok(relationships)
}

fn sqlite_select_columns(table_name: &str, columns: &[String]) -> OcelResult<String> {
    let columns = columns
        .iter()
        .map(|column| quote_sql_identifier(column))
        .collect::<OcelResult<Vec<_>>>()?
        .join(", ");
    Ok(format!(
        "SELECT {columns} FROM {}",
        quote_sql_identifier(table_name)?
    ))
}

fn quote_sql_identifier(value: &str) -> OcelResult<String> {
    if value.contains('\0') {
        return Err(OcelError::new("SQLite identifiers cannot contain NUL bytes"));
    }
    Ok(format!("\"{}\"", value.replace('"', "\"\"")))
}

fn sqlite_import_error(error: rusqlite::Error) -> OcelError {
    OcelError::new(format!("invalid OCEL SQLite database: {error}"))
}

fn sqlite_required_text(value: rusqlite::types::ValueRef<'_>, column: &str) -> OcelResult<String> {
    sqlite_optional_text(value, column)?.ok_or_else(|| {
        OcelError::new(format!(
            "OCEL SQLite column '{column}' contains an unexpected NULL"
        ))
    })
}

fn sqlite_optional_text(
    value: rusqlite::types::ValueRef<'_>,
    column: &str,
) -> OcelResult<Option<String>> {
    match value {
        rusqlite::types::ValueRef::Null => Ok(None),
        rusqlite::types::ValueRef::Text(value) => std::str::from_utf8(value)
            .map(|value| Some(value.to_owned()))
            .map_err(|err| {
                OcelError::new(format!(
                    "OCEL SQLite column '{column}' is not valid UTF-8: {err}"
                ))
            }),
        _ => Err(OcelError::new(format!(
            "OCEL SQLite column '{column}' must contain TEXT values"
        ))),
    }
}

fn sqlite_required_timestamp(
    value: rusqlite::types::ValueRef<'_>,
    column: &str,
) -> OcelResult<String> {
    sqlite_optional_timestamp(value, column)?.ok_or_else(|| {
        OcelError::new(format!(
            "OCEL SQLite timestamp column '{column}' contains an unexpected NULL"
        ))
    })
}

fn sqlite_optional_timestamp(
    value: rusqlite::types::ValueRef<'_>,
    column: &str,
) -> OcelResult<Option<String>> {
    let Some(value) = sqlite_optional_text(value, column)? else {
        return Ok(None);
    };
    parse_timestamp_micros(&value)?;
    Ok(Some(value))
}

fn sqlite_source_value(
    value: rusqlite::types::ValueRef<'_>,
    attr_type: AttrType,
    column: &str,
) -> OcelResult<Option<SourceValue>> {
    use rusqlite::types::ValueRef;
    if matches!(value, ValueRef::Null) {
        return Ok(None);
    }
    let text = |value: &[u8]| {
        std::str::from_utf8(value)
            .map(str::to_owned)
            .map_err(|err| {
                OcelError::new(format!(
                    "OCEL SQLite column '{column}' is not valid UTF-8: {err}"
                ))
            })
    };
    let value = match (attr_type, value) {
        (AttrType::String, ValueRef::Text(value)) => SourceValue::String(text(value)?),
        (AttrType::Time, ValueRef::Text(value)) => {
            let value = text(value)?;
            parse_timestamp_micros(&value)?;
            SourceValue::String(value)
        }
        (AttrType::Integer, ValueRef::Integer(value)) => SourceValue::Integer(value),
        (AttrType::Float, ValueRef::Integer(value)) => SourceValue::Integer(value),
        (AttrType::Float, ValueRef::Real(value)) if value.is_finite() => {
            SourceValue::Float(value)
        }
        (AttrType::Boolean, ValueRef::Integer(0)) => SourceValue::Boolean(false),
        (AttrType::Boolean, ValueRef::Integer(1)) => SourceValue::Boolean(true),
        (AttrType::Float, ValueRef::Real(_)) => {
            return Err(OcelError::new(format!(
                "OCEL SQLite column '{column}' contains a non-finite float"
            )));
        }
        (AttrType::Boolean, ValueRef::Integer(value)) => {
            return Err(OcelError::new(format!(
                "OCEL SQLite BOOLEAN column '{column}' contains {value}; expected 0 or 1"
            )));
        }
        (expected, _) => {
            return Err(OcelError::new(format!(
                "OCEL SQLite column '{column}' contains a value incompatible with declared type '{}'",
                expected.as_str()
            )));
        }
    };
    Ok(Some(value))
}

#[allow(clippy::too_many_arguments)]
fn sqlite_add_object_assignment(
    object_id: &str,
    name: &str,
    time_micros: i64,
    value: SourceValue,
    sequence: usize,
    object_attributes: &mut HashMap<String, Vec<(i64, usize, SourceTimedAttribute)>>,
    assignment_values: &mut HashMap<(String, String, i64), SourceValue>,
) -> OcelResult<()> {
    let key = (object_id.to_owned(), name.to_owned(), time_micros);
    if let Some(existing) = assignment_values.get(&key) {
        if existing != &value {
            return Err(OcelError::new(format!(
                "object '{object_id}' has conflicting values for attribute '{name}' at the same timestamp"
            )));
        }
        return Ok(());
    }
    assignment_values.insert(key, value.clone());
    object_attributes
        .entry(object_id.to_owned())
        .or_default()
        .push((
            time_micros,
            sequence,
            SourceTimedAttribute {
                name: name.to_owned(),
                time: format_timestamp_micros(time_micros)?,
                value,
            },
        ));
    Ok(())
}
