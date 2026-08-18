impl CompactOcelLog {
    fn export_sqlite(&self) -> OcelResult<Vec<u8>> {
        self.validate_sqlite_attribute_names()?;
        let mut connection = rusqlite::Connection::open_in_memory().map_err(|err| {
            OcelError::new(format!("could not create OCEL SQLite database: {err}"))
        })?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE \"event_map_type\" (
                   \"ocel_type\" TEXT PRIMARY KEY,
                   \"ocel_type_map\" TEXT NOT NULL UNIQUE
                 );
                 CREATE TABLE \"object_map_type\" (
                   \"ocel_type\" TEXT PRIMARY KEY,
                   \"ocel_type_map\" TEXT NOT NULL UNIQUE
                 );
                 CREATE TABLE \"event\" (
                   \"ocel_id\" TEXT PRIMARY KEY,
                   \"ocel_type\" TEXT NOT NULL,
                   FOREIGN KEY (\"ocel_type\") REFERENCES \"event_map_type\" (\"ocel_type\")
                 );
                 CREATE TABLE \"object\" (
                   \"ocel_id\" TEXT PRIMARY KEY,
                   \"ocel_type\" TEXT NOT NULL,
                   FOREIGN KEY (\"ocel_type\") REFERENCES \"object_map_type\" (\"ocel_type\")
                 );
                 CREATE TABLE \"event_object\" (
                   \"ocel_event_id\" TEXT NOT NULL,
                   \"ocel_object_id\" TEXT NOT NULL,
                   \"ocel_qualifier\" TEXT NOT NULL,
                   PRIMARY KEY (\"ocel_event_id\", \"ocel_object_id\", \"ocel_qualifier\"),
                   FOREIGN KEY (\"ocel_event_id\") REFERENCES \"event\" (\"ocel_id\"),
                   FOREIGN KEY (\"ocel_object_id\") REFERENCES \"object\" (\"ocel_id\")
                 );
                 CREATE TABLE \"object_object\" (
                   \"ocel_source_id\" TEXT NOT NULL,
                   \"ocel_target_id\" TEXT NOT NULL,
                   \"ocel_qualifier\" TEXT NOT NULL,
                   PRIMARY KEY (\"ocel_source_id\", \"ocel_target_id\", \"ocel_qualifier\"),
                   FOREIGN KEY (\"ocel_source_id\") REFERENCES \"object\" (\"ocel_id\"),
                   FOREIGN KEY (\"ocel_target_id\") REFERENCES \"object\" (\"ocel_id\")
                 );",
            )
            .map_err(sqlite_export_error)?;

        let event_mappings = self
            .event_types
            .iter()
            .map(|type_def| {
                (
                    self.pool.resolve(type_def.name).to_owned(),
                    sqlite_safe_type_mapping(self.pool.resolve(type_def.name)),
                )
            })
            .collect::<Vec<_>>();
        let object_mappings = self
            .object_types
            .iter()
            .map(|type_def| {
                (
                    self.pool.resolve(type_def.name).to_owned(),
                    sqlite_safe_type_mapping(self.pool.resolve(type_def.name)),
                )
            })
            .collect::<Vec<_>>();

        let transaction = connection.transaction().map_err(sqlite_export_error)?;
        for ((type_name, mapped_name), type_def) in
            event_mappings.iter().zip(&self.event_types)
        {
            transaction
                .execute(
                    "INSERT INTO \"event_map_type\" (\"ocel_type\", \"ocel_type_map\") VALUES (?1, ?2)",
                    rusqlite::params![type_name, mapped_name],
                )
                .map_err(sqlite_export_error)?;
            let table_name = format!("event_{mapped_name}");
            let mut column_sql = vec![
                "\"ocel_id\" TEXT PRIMARY KEY".to_owned(),
                "\"ocel_time\" TIMESTAMP NOT NULL".to_owned(),
            ];
            for attribute in &type_def.attributes {
                column_sql.push(format!(
                    "{} {}",
                    quote_sql_identifier(self.pool.resolve(attribute.name))?,
                    sqlite_type_name(attribute.attr_type)
                ));
            }
            column_sql.push(
                "FOREIGN KEY (\"ocel_id\") REFERENCES \"event\" (\"ocel_id\")".to_owned(),
            );
            transaction
                .execute_batch(&format!(
                    "CREATE TABLE {} ({})",
                    quote_sql_identifier(&table_name)?,
                    column_sql.join(", ")
                ))
                .map_err(sqlite_export_error)?;
        }
        for ((type_name, mapped_name), type_def) in
            object_mappings.iter().zip(&self.object_types)
        {
            transaction
                .execute(
                    "INSERT INTO \"object_map_type\" (\"ocel_type\", \"ocel_type_map\") VALUES (?1, ?2)",
                    rusqlite::params![type_name, mapped_name],
                )
                .map_err(sqlite_export_error)?;
            let table_name = format!("object_{mapped_name}");
            let mut column_sql = vec![
                "\"ocel_id\" TEXT NOT NULL".to_owned(),
                "\"ocel_time\" TIMESTAMP NOT NULL".to_owned(),
                "\"ocel_changed_field\" TEXT".to_owned(),
            ];
            for attribute in &type_def.attributes {
                column_sql.push(format!(
                    "{} {}",
                    quote_sql_identifier(self.pool.resolve(attribute.name))?,
                    sqlite_type_name(attribute.attr_type)
                ));
            }
            column_sql.push(
                "FOREIGN KEY (\"ocel_id\") REFERENCES \"object\" (\"ocel_id\")".to_owned(),
            );
            transaction
                .execute_batch(&format!(
                    "CREATE TABLE {} ({})",
                    quote_sql_identifier(&table_name)?,
                    column_sql.join(", ")
                ))
                .map_err(sqlite_export_error)?;
        }

        for object in &self.objects {
            transaction
                .execute(
                    "INSERT INTO \"object\" (\"ocel_id\", \"ocel_type\") VALUES (?1, ?2)",
                    rusqlite::params![
                        self.pool.resolve(object.id),
                        self.pool.resolve(object.type_name)
                    ],
                )
                .map_err(sqlite_export_error)?;
        }
        for event in &self.events {
            transaction
                .execute(
                    "INSERT INTO \"event\" (\"ocel_id\", \"ocel_type\") VALUES (?1, ?2)",
                    rusqlite::params![
                        self.pool.resolve(event.id),
                        self.pool.resolve(event.type_name)
                    ],
                )
                .map_err(sqlite_export_error)?;
        }

        let event_type_positions = self
            .event_types
            .iter()
            .enumerate()
            .map(|(index, type_def)| (type_def.name, index))
            .collect::<HashMap<_, _>>();
        for event in &self.events {
            let type_position = event_type_positions[&event.type_name];
            let type_def = &self.event_types[type_position];
            let table_name = format!("event_{}", event_mappings[type_position].1);
            let mut columns = vec!["ocel_id".to_owned(), "ocel_time".to_owned()];
            columns.extend(
                type_def
                    .attributes
                    .iter()
                    .map(|attribute| self.pool.resolve(attribute.name).to_owned()),
            );
            let sql = sqlite_insert_sql(&table_name, &columns)?;
            let event_attributes = event
                .attributes
                .iter()
                .map(|attribute| (attribute.name, &attribute.value))
                .collect::<HashMap<_, _>>();
            let mut values = vec![
                rusqlite::types::Value::Text(self.pool.resolve(event.id).to_owned()),
                rusqlite::types::Value::Text(format_timestamp_micros(event.time_micros)?),
            ];
            for attribute in &type_def.attributes {
                values.push(match event_attributes.get(&attribute.name) {
                    Some(value) => self.attr_value_to_sqlite(value)?,
                    None => rusqlite::types::Value::Null,
                });
            }
            transaction
                .execute(&sql, rusqlite::params_from_iter(values.iter()))
                .map_err(sqlite_export_error)?;
            for relationship in &event.relationships {
                transaction
                    .execute(
                        "INSERT INTO \"event_object\" (\"ocel_event_id\", \"ocel_object_id\", \"ocel_qualifier\") VALUES (?1, ?2, ?3)",
                        rusqlite::params![
                            self.pool.resolve(event.id),
                            self.pool.resolve(relationship.object_id),
                            self.pool.resolve(relationship.qualifier)
                        ],
                    )
                    .map_err(sqlite_export_error)?;
            }
        }

        let object_type_positions = self
            .object_types
            .iter()
            .enumerate()
            .map(|(index, type_def)| (type_def.name, index))
            .collect::<HashMap<_, _>>();
        for object in &self.objects {
            let type_position = object_type_positions[&object.type_name];
            let type_def = &self.object_types[type_position];
            let table_name = format!("object_{}", object_mappings[type_position].1);
            let mut columns = vec![
                "ocel_id".to_owned(),
                "ocel_time".to_owned(),
                "ocel_changed_field".to_owned(),
            ];
            columns.extend(
                type_def
                    .attributes
                    .iter()
                    .map(|attribute| self.pool.resolve(attribute.name).to_owned()),
            );
            let sql = sqlite_insert_sql(&table_name, &columns)?;

            let initial_values = self.sqlite_object_values_at(object, 0)?;
            let mut values = vec![
                rusqlite::types::Value::Text(self.pool.resolve(object.id).to_owned()),
                rusqlite::types::Value::Text("1970-01-01T00:00:00Z".to_owned()),
                rusqlite::types::Value::Null,
            ];
            for attribute in &type_def.attributes {
                values.push(
                    initial_values
                        .get(&attribute.name)
                        .cloned()
                        .unwrap_or(rusqlite::types::Value::Null),
                );
            }
            transaction
                .execute(&sql, rusqlite::params_from_iter(values.iter()))
                .map_err(sqlite_export_error)?;

            let mut seen_changes = HashMap::<(Symbol, i64), rusqlite::types::Value>::new();
            for attribute in &object.attributes {
                if attribute.time_micros == 0 {
                    continue;
                }
                let value = self.attr_value_to_sqlite(&attribute.value)?;
                let key = (attribute.name, attribute.time_micros);
                if let Some(existing) = seen_changes.get(&key) {
                    if existing != &value {
                        return Err(OcelError::new(format!(
                            "object '{}' has conflicting values for attribute '{}' at the same timestamp",
                            self.pool.resolve(object.id),
                            self.pool.resolve(attribute.name)
                        )));
                    }
                    continue;
                }
                seen_changes.insert(key, value.clone());
                let mut values = vec![
                    rusqlite::types::Value::Text(self.pool.resolve(object.id).to_owned()),
                    rusqlite::types::Value::Text(format_timestamp_micros(
                        attribute.time_micros,
                    )?),
                    rusqlite::types::Value::Text(
                        self.pool.resolve(attribute.name).to_owned(),
                    ),
                ];
                for type_attribute in &type_def.attributes {
                    values.push(if type_attribute.name == attribute.name {
                        value.clone()
                    } else {
                        rusqlite::types::Value::Null
                    });
                }
                transaction
                    .execute(&sql, rusqlite::params_from_iter(values.iter()))
                    .map_err(sqlite_export_error)?;
            }

            for relationship in &object.relationships {
                transaction
                    .execute(
                        "INSERT INTO \"object_object\" (\"ocel_source_id\", \"ocel_target_id\", \"ocel_qualifier\") VALUES (?1, ?2, ?3)",
                        rusqlite::params![
                            self.pool.resolve(object.id),
                            self.pool.resolve(relationship.object_id),
                            self.pool.resolve(relationship.qualifier)
                        ],
                    )
                    .map_err(sqlite_export_error)?;
            }
        }
        transaction.commit().map_err(sqlite_export_error)?;
        let serialized = connection
            .serialize("main")
            .map_err(sqlite_export_error)?;
        Ok(serialized.to_vec())
    }

    fn validate_sqlite_attribute_names(&self) -> OcelResult<()> {
        for type_def in &self.event_types {
            for attribute in &type_def.attributes {
                let name = self.pool.resolve(attribute.name);
                if matches!(name, "ocel_id" | "ocel_time") {
                    return Err(OcelError::new(format!(
                        "event attribute '{name}' conflicts with a required SQLite column"
                    )));
                }
                quote_sql_identifier(name)?;
            }
        }
        for type_def in &self.object_types {
            for attribute in &type_def.attributes {
                let name = self.pool.resolve(attribute.name);
                if matches!(name, "ocel_id" | "ocel_time" | "ocel_changed_field") {
                    return Err(OcelError::new(format!(
                        "object attribute '{name}' conflicts with a required SQLite column"
                    )));
                }
                quote_sql_identifier(name)?;
            }
        }
        Ok(())
    }

    fn attr_value_to_sqlite(&self, value: &AttrValue) -> OcelResult<rusqlite::types::Value> {
        match value {
            AttrValue::String(symbol) => Ok(rusqlite::types::Value::Text(
                self.pool.resolve(*symbol).to_owned(),
            )),
            AttrValue::Time(micros) => Ok(rusqlite::types::Value::Text(
                format_timestamp_micros(*micros)?,
            )),
            AttrValue::Integer(value) => Ok(rusqlite::types::Value::Integer(*value)),
            AttrValue::Float(value) if value.is_finite() => {
                Ok(rusqlite::types::Value::Real(*value))
            }
            AttrValue::Float(_) => Err(OcelError::new(
                "cannot export non-finite float to OCEL SQLite",
            )),
            AttrValue::Boolean(value) => {
                Ok(rusqlite::types::Value::Integer(i64::from(*value)))
            }
        }
    }

    fn sqlite_object_values_at(
        &self,
        object: &Object,
        time_micros: i64,
    ) -> OcelResult<HashMap<Symbol, rusqlite::types::Value>> {
        let mut result = HashMap::new();
        for attribute in &object.attributes {
            if attribute.time_micros != time_micros {
                continue;
            }
            let value = self.attr_value_to_sqlite(&attribute.value)?;
            if let Some(existing) = result.insert(attribute.name, value.clone()) {
                if existing != value {
                    return Err(OcelError::new(format!(
                        "object '{}' has conflicting values for attribute '{}' at the same timestamp",
                        self.pool.resolve(object.id),
                        self.pool.resolve(attribute.name)
                    )));
                }
            }
        }
        Ok(result)
    }
}

fn sqlite_safe_type_mapping(type_name: &str) -> String {
    let mut mapped = String::from("T");
    for byte in type_name.as_bytes() {
        write!(mapped, "{byte:02X}").expect("writing to String cannot fail");
    }
    mapped
}

fn sqlite_type_name(attr_type: AttrType) -> &'static str {
    match attr_type {
        AttrType::String => "TEXT",
        AttrType::Time => "TIMESTAMP",
        AttrType::Integer => "INTEGER",
        AttrType::Float => "REAL",
        AttrType::Boolean => "BOOLEAN",
    }
}

fn sqlite_insert_sql(table_name: &str, columns: &[String]) -> OcelResult<String> {
    let column_count = columns.len();
    let columns = columns
        .iter()
        .map(|column| quote_sql_identifier(column))
        .collect::<OcelResult<Vec<_>>>()?
        .join(", ");
    let placeholders = (1..=column_count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "INSERT INTO {} ({columns}) VALUES ({placeholders})",
        quote_sql_identifier(table_name)?
    ))
}

fn sqlite_export_error(error: rusqlite::Error) -> OcelError {
    OcelError::new(format!("could not export OCEL SQLite database: {error}"))
}
