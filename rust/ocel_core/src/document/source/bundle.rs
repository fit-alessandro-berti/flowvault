#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleMetadata {
    ocel_version: String,
    bundle_format_version: String,
    storage_format: String,
    event_types: BTreeMap<String, BundleTypeMetadata>,
    object_types: BTreeMap<String, BundleTypeMetadata>,
    relations: BundleRelationMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleTypeMetadata {
    file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    changes_file: Option<String>,
    #[serde(default)]
    attributes: Vec<BundleAttributeMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleAttributeMetadata {
    name: String,
    #[serde(rename = "type")]
    attr_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleRelationMetadata {
    e2o: String,
    o2o: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BundleColumnType {
    String,
    Integer,
    Float,
    Boolean,
    Time,
}

impl BundleColumnType {
    fn from_attr_type(attr_type: AttrType) -> Self {
        match attr_type {
            AttrType::String => Self::String,
            AttrType::Integer => Self::Integer,
            AttrType::Float => Self::Float,
            AttrType::Boolean => Self::Boolean,
            AttrType::Time => Self::Time,
        }
    }
}

#[derive(Debug, Clone)]
struct BundleColumnSpec {
    name: String,
    column_type: BundleColumnType,
    required: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum BundleCell {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Time(i64),
}

type BundleRow = BTreeMap<String, Option<BundleCell>>;

fn parse_bundle(input: &[u8]) -> OcelResult<SourceLog> {
    if !input.starts_with(b"PK") {
        return Err(OcelError::new(
            "invalid OCEL bundle: missing ZIP archive header",
        ));
    }
    let cursor = std::io::Cursor::new(input);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|err| OcelError::new(format!("invalid OCEL bundle ZIP archive: {err}")))?;
    validate_bundle_archive_entries(&mut archive)?;
    let metadata_bytes = read_bundle_entry(&mut archive, "ocel-meta.json")?;
    let metadata: BundleMetadata = serde_json::from_slice(&metadata_bytes)
        .map_err(|err| OcelError::new(format!("invalid OCEL bundle metadata: {err}")))?;
    validate_bundle_metadata(&metadata)?;

    let mut event_types = Vec::with_capacity(metadata.event_types.len());
    let mut object_types = Vec::with_capacity(metadata.object_types.len());
    let mut events = Vec::<SourceEvent>::new();
    let mut objects = Vec::<SourceObject>::new();
    let mut event_ids = HashSet::new();
    let mut object_ids = HashSet::new();

    for (type_name, type_metadata) in &metadata.event_types {
        let attributes = bundle_source_attributes(&type_metadata.attributes)?;
        event_types.push(SourceType {
            name: type_name.clone(),
            attributes: attributes.clone(),
        });
        let mut columns = vec![
            bundle_required_column("ocel_id", BundleColumnType::String),
            bundle_required_column("ocel_time", BundleColumnType::Time),
        ];
        columns.extend(bundle_attribute_columns(&attributes)?);
        let table = read_bundle_parquet(&mut archive, &type_metadata.file, &columns)?;
        for row in table {
            let id = bundle_required_string(&row, "ocel_id")?;
            if !event_ids.insert(id.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate event id '{id}' in OCEL bundle"
                )));
            }
            let time_micros = bundle_required_time(&row, "ocel_time")?;
            events.push(SourceEvent {
                id,
                type_name: type_name.clone(),
                time: format_timestamp_micros(time_micros)?,
                attributes: bundle_event_attributes(&row, &attributes)?,
                relationships: Vec::new(),
            });
        }
    }

    for (type_name, type_metadata) in &metadata.object_types {
        let attributes = bundle_source_attributes(&type_metadata.attributes)?;
        object_types.push(SourceType {
            name: type_name.clone(),
            attributes: attributes.clone(),
        });
        let mut object_columns = vec![bundle_required_column(
            "ocel_id",
            BundleColumnType::String,
        )];
        object_columns.extend(bundle_attribute_columns(&attributes)?);
        let object_table =
            read_bundle_parquet(&mut archive, &type_metadata.file, &object_columns)?;

        let changes_file = type_metadata.changes_file.as_ref().ok_or_else(|| {
            OcelError::new(format!(
                "OCEL bundle object type '{type_name}' is missing changesFile"
            ))
        })?;
        let mut change_columns = vec![
            bundle_required_column("ocel_id", BundleColumnType::String),
            bundle_required_column("ocel_time", BundleColumnType::Time),
            bundle_required_column("ocel_changed_field", BundleColumnType::String),
        ];
        change_columns.extend(bundle_attribute_columns(&attributes)?);
        let change_table = read_bundle_parquet(&mut archive, changes_file, &change_columns)?;
        let mut changes_by_object = HashMap::<String, Vec<(usize, SourceTimedAttribute)>>::new();
        let mut change_assignments = HashMap::<(String, String, i64), SourceValue>::new();
        for (sequence, row) in change_table.into_iter().enumerate() {
            let id = bundle_required_string(&row, "ocel_id")?;
            let time_micros = bundle_required_time(&row, "ocel_time")?;
            if time_micros == 0 {
                return Err(OcelError::new(format!(
                    "OCEL bundle object change for '{id}' uses the Unix epoch; initial values belong in the object table"
                )));
            }
            let changed_field = bundle_required_string(&row, "ocel_changed_field")?;
            let attribute = attributes
                .iter()
                .find(|attribute| attribute.name == changed_field)
                .ok_or_else(|| {
                    OcelError::new(format!(
                        "OCEL bundle object change for '{id}' names unknown field '{changed_field}'"
                    ))
                })?;
            let value = bundle_attribute_source_value(&row, attribute)?.ok_or_else(|| {
                OcelError::new(format!(
                    "OCEL bundle object change for '{id}' has a null value for '{changed_field}'"
                ))
            })?;
            for other_attribute in &attributes {
                if other_attribute.name != changed_field
                    && row
                        .get(&other_attribute.name)
                        .and_then(Option::as_ref)
                        .is_some()
                {
                    return Err(OcelError::new(format!(
                        "OCEL bundle object change for '{id}' contains a value for unchanged field '{}'",
                        other_attribute.name
                    )));
                }
            }
            let assignment_key = (id.clone(), changed_field.clone(), time_micros);
            if let Some(existing) = change_assignments.get(&assignment_key) {
                if existing != &value {
                    return Err(OcelError::new(format!(
                        "OCEL bundle has conflicting changes for object '{id}' field '{changed_field}' at the same timestamp"
                    )));
                }
                continue;
            }
            change_assignments.insert(assignment_key, value.clone());
            changes_by_object.entry(id).or_default().push((
                sequence,
                SourceTimedAttribute {
                    name: changed_field,
                    time: format_timestamp_micros(time_micros)?,
                    value,
                },
            ));
        }

        for row in object_table {
            let id = bundle_required_string(&row, "ocel_id")?;
            if !object_ids.insert(id.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate object id '{id}' in OCEL bundle"
                )));
            }
            let mut timed_attributes = Vec::new();
            for attribute in &attributes {
                if let Some(value) = bundle_attribute_source_value(&row, attribute)? {
                    timed_attributes.push(SourceTimedAttribute {
                        name: attribute.name.clone(),
                        time: "1970-01-01T00:00:00Z".to_owned(),
                        value,
                    });
                }
            }
            if let Some(mut changes) = changes_by_object.remove(&id) {
                changes.sort_by_key(|(sequence, attribute)| {
                    (
                        parse_timestamp_micros(&attribute.time).unwrap_or_default(),
                        *sequence,
                    )
                });
                timed_attributes.extend(changes.into_iter().map(|(_, attribute)| attribute));
            }
            objects.push(SourceObject {
                id,
                type_name: type_name.clone(),
                attributes: timed_attributes,
                relationships: Vec::new(),
            });
        }
        if let Some((unknown_id, _)) = changes_by_object.into_iter().next() {
            return Err(OcelError::new(format!(
                "OCEL bundle change table references unknown object '{unknown_id}'"
            )));
        }
    }

    let event_positions = events
        .iter()
        .enumerate()
        .map(|(index, event)| (event.id.clone(), index))
        .collect::<HashMap<_, _>>();
    let object_positions = objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.id.clone(), index))
        .collect::<HashMap<_, _>>();
    let e2o_columns = vec![
        bundle_required_column("ocel_event_id", BundleColumnType::String),
        bundle_required_column("ocel_object_id", BundleColumnType::String),
        bundle_required_column("ocel_qualifier", BundleColumnType::String),
    ];
    for row in read_bundle_parquet(&mut archive, &metadata.relations.e2o, &e2o_columns)? {
        let event_id = bundle_required_string(&row, "ocel_event_id")?;
        let object_id = bundle_required_string(&row, "ocel_object_id")?;
        let qualifier = bundle_required_string(&row, "ocel_qualifier")?;
        let event_position = event_positions.get(&event_id).ok_or_else(|| {
            OcelError::new(format!(
                "OCEL bundle event-object relationship references unknown event '{event_id}'"
            ))
        })?;
        if !object_positions.contains_key(&object_id) {
            return Err(OcelError::new(format!(
                "OCEL bundle event-object relationship references unknown object '{object_id}'"
            )));
        }
        events[*event_position]
            .relationships
            .push(SourceRelationship {
                object_id,
                qualifier,
            });
    }
    let o2o_columns = vec![
        bundle_required_column("ocel_source_id", BundleColumnType::String),
        bundle_required_column("ocel_target_id", BundleColumnType::String),
        bundle_required_column("ocel_qualifier", BundleColumnType::String),
    ];
    for row in read_bundle_parquet(&mut archive, &metadata.relations.o2o, &o2o_columns)? {
        let source_id = bundle_required_string(&row, "ocel_source_id")?;
        let target_id = bundle_required_string(&row, "ocel_target_id")?;
        let qualifier = bundle_required_string(&row, "ocel_qualifier")?;
        let source_position = object_positions.get(&source_id).ok_or_else(|| {
            OcelError::new(format!(
                "OCEL bundle object-object relationship references unknown source '{source_id}'"
            ))
        })?;
        if !object_positions.contains_key(&target_id) {
            return Err(OcelError::new(format!(
                "OCEL bundle object-object relationship references unknown target '{target_id}'"
            )));
        }
        objects[*source_position]
            .relationships
            .push(SourceRelationship {
                object_id: target_id,
                qualifier,
            });
    }

    Ok(SourceLog {
        event_types,
        object_types,
        events,
        objects,
    })
}

fn validate_bundle_archive_entries<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> OcelResult<()> {
    let mut names = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| OcelError::new(format!("invalid OCEL bundle ZIP entry: {err}")))?;
        let name = entry.name().to_owned();
        let validated_name = if entry.is_dir() {
            name.strip_suffix('/').unwrap_or(&name)
        } else {
            &name
        };
        validate_bundle_path(validated_name)?;
        if !names.insert(name.clone()) {
            return Err(OcelError::new(format!(
                "OCEL bundle contains duplicate ZIP entry '{name}'"
            )));
        }
    }
    Ok(())
}

fn validate_bundle_metadata(metadata: &BundleMetadata) -> OcelResult<()> {
    if metadata.ocel_version != "2.0" {
        return Err(OcelError::new(format!(
            "unsupported OCEL bundle OCEL version '{}'; expected '2.0'",
            metadata.ocel_version
        )));
    }
    if metadata.bundle_format_version != "1.0" {
        return Err(OcelError::new(format!(
            "unsupported OCEL bundle format version '{}'; expected '1.0'",
            metadata.bundle_format_version
        )));
    }
    if metadata.storage_format != "parquet" {
        return Err(OcelError::new(format!(
            "unsupported OCEL bundle storage format '{}'; this importer expects 'parquet'",
            metadata.storage_format
        )));
    }
    let mut paths = HashSet::new();
    validate_unique_bundle_path(&metadata.relations.e2o, &mut paths)?;
    validate_unique_bundle_path(&metadata.relations.o2o, &mut paths)?;
    for (type_name, type_metadata) in &metadata.event_types {
        validate_unique_bundle_path(&type_metadata.file, &mut paths)?;
        validate_bundle_attribute_metadata(type_name, &type_metadata.attributes)?;
    }
    for (type_name, type_metadata) in &metadata.object_types {
        validate_unique_bundle_path(&type_metadata.file, &mut paths)?;
        validate_unique_bundle_path(
            type_metadata.changes_file.as_deref().ok_or_else(|| {
                OcelError::new(format!(
                    "OCEL bundle object type '{type_name}' is missing changesFile"
                ))
            })?,
            &mut paths,
        )?;
        validate_bundle_attribute_metadata(type_name, &type_metadata.attributes)?;
    }
    Ok(())
}

fn validate_bundle_attribute_metadata(
    type_name: &str,
    attributes: &[BundleAttributeMetadata],
) -> OcelResult<()> {
    let mut names = HashSet::new();
    for attribute in attributes {
        if !names.insert(&attribute.name) {
            return Err(OcelError::new(format!(
                "duplicate attribute '{}' in OCEL bundle type '{type_name}'",
                attribute.name
            )));
        }
        AttrType::parse(&attribute.attr_type)?;
    }
    Ok(())
}

fn validate_bundle_path(path: &str) -> OcelResult<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || path.contains('\0')
    {
        return Err(OcelError::new(format!(
            "invalid relative path '{path}' in OCEL bundle"
        )));
    }
    Ok(())
}

fn validate_unique_bundle_path(path: &str, paths: &mut HashSet<String>) -> OcelResult<()> {
    validate_bundle_path(path)?;
    if !paths.insert(path.to_owned()) {
        return Err(OcelError::new(format!(
            "OCEL bundle metadata declares path '{path}' more than once"
        )));
    }
    Ok(())
}

fn read_bundle_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
) -> OcelResult<Vec<u8>> {
    validate_bundle_path(path)?;
    let mut entry = archive.by_name(path).map_err(|err| {
        OcelError::new(format!("OCEL bundle is missing declared entry '{path}': {err}"))
    })?;
    let mut output = Vec::with_capacity(entry.size().min(usize::MAX as u64) as usize);
    entry.read_to_end(&mut output).map_err(|err| {
        OcelError::new(format!("could not read OCEL bundle entry '{path}': {err}"))
    })?;
    Ok(output)
}

fn bundle_source_attributes(
    attributes: &[BundleAttributeMetadata],
) -> OcelResult<Vec<SourceAttributeDef>> {
    attributes
        .iter()
        .map(|attribute| {
            AttrType::parse(&attribute.attr_type)?;
            Ok(SourceAttributeDef {
                name: attribute.name.clone(),
                attr_type: attribute.attr_type.clone(),
            })
        })
        .collect()
}

fn bundle_required_column(name: &str, column_type: BundleColumnType) -> BundleColumnSpec {
    BundleColumnSpec {
        name: name.to_owned(),
        column_type,
        required: true,
    }
}

fn bundle_attribute_columns(attributes: &[SourceAttributeDef]) -> OcelResult<Vec<BundleColumnSpec>> {
    attributes
        .iter()
        .map(|attribute| {
            Ok(BundleColumnSpec {
                name: attribute.name.clone(),
                column_type: BundleColumnType::from_attr_type(AttrType::parse(
                    &attribute.attr_type,
                )?),
                required: false,
            })
        })
        .collect()
}

fn read_bundle_parquet<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
    columns: &[BundleColumnSpec],
) -> OcelResult<Vec<BundleRow>> {
    let input = read_bundle_entry(archive, path)?;
    let reader = parquet::file::reader::SerializedFileReader::new(bytes::Bytes::from(input))
        .map_err(|err| OcelError::new(format!("invalid Parquet table '{path}': {err}")))?;
    validate_bundle_parquet_schema(&reader, path, columns)?;
    let rows = parquet::file::reader::FileReader::get_row_iter(&reader, None)
        .map_err(|err| OcelError::new(format!("could not read Parquet table '{path}': {err}")))?;
    rows.map(|row| {
        let row = row
            .map_err(|err| OcelError::new(format!("invalid row in Parquet table '{path}': {err}")))?;
        row.get_column_iter()
            .map(|(name, field)| {
                bundle_cell_from_parquet(field).map(|value| (name.clone(), value))
            })
            .collect::<OcelResult<BundleRow>>()
    })
    .collect()
}

fn validate_bundle_parquet_schema<T: parquet::file::reader::ChunkReader + 'static>(
    reader: &parquet::file::reader::SerializedFileReader<T>,
    path: &str,
    expected: &[BundleColumnSpec],
) -> OcelResult<()> {
    let schema = parquet::file::reader::FileReader::metadata(reader)
        .file_metadata()
        .schema_descr();
    let actual = schema.columns();
    if actual.len() != expected.len() {
        return Err(OcelError::new(format!(
            "Parquet table '{path}' has {} columns; expected {}",
            actual.len(),
            expected.len()
        )));
    }
    let expected_by_name = expected
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect::<HashMap<_, _>>();
    let mut actual_names = HashSet::new();
    for column in actual {
        if !actual_names.insert(column.name()) {
            return Err(OcelError::new(format!(
                "Parquet table '{path}' contains duplicate column '{}'",
                column.name()
            )));
        }
        let Some(expected_column) = expected_by_name.get(column.name()) else {
            return Err(OcelError::new(format!(
                "Parquet table '{path}' contains unexpected column '{}'",
                column.name()
            )));
        };
        let expected_def_level = if expected_column.required { 0 } else { 1 };
        if column.max_def_level() != expected_def_level {
            return Err(OcelError::new(format!(
                "Parquet column '{path}.{}' has incorrect nullability",
                column.name()
            )));
        }
        if column.max_rep_level() != 0 {
            return Err(OcelError::new(format!(
                "Parquet column '{path}.{}' must not be repeated",
                column.name()
            )));
        }
        let valid_type = match expected_column.column_type {
            BundleColumnType::String => {
                column.physical_type() == parquet::basic::Type::BYTE_ARRAY
                    && matches!(
                        column.logical_type_ref(),
                        Some(parquet::basic::LogicalType::String)
                    )
            }
            BundleColumnType::Integer => {
                column.physical_type() == parquet::basic::Type::INT64
                    && column.logical_type_ref().is_none()
            }
            BundleColumnType::Float => {
                column.physical_type() == parquet::basic::Type::DOUBLE
                    && column.logical_type_ref().is_none()
            }
            BundleColumnType::Boolean => {
                column.physical_type() == parquet::basic::Type::BOOLEAN
                    && column.logical_type_ref().is_none()
            }
            BundleColumnType::Time => {
                column.physical_type() == parquet::basic::Type::INT64
                    && matches!(
                        column.logical_type_ref(),
                        Some(parquet::basic::LogicalType::Timestamp(timestamp))
                            if timestamp.is_adjusted_to_u_t_c
                                && timestamp.unit == parquet::basic::TimeUnit::MICROS
                    )
            }
        };
        if !valid_type {
            return Err(OcelError::new(format!(
                "Parquet column '{path}.{}' has a physical/logical type that does not match OCEL metadata",
                column.name()
            )));
        }
    }
    if let Some(missing) = expected
        .iter()
        .find(|column| !actual_names.contains(column.name.as_str()))
    {
        return Err(OcelError::new(format!(
            "Parquet table '{path}' is missing column '{}'",
            missing.name
        )));
    }
    Ok(())
}

fn bundle_cell_from_parquet(field: &parquet::record::Field) -> OcelResult<Option<BundleCell>> {
    match field {
        parquet::record::Field::Null => Ok(None),
        parquet::record::Field::Str(value) => Ok(Some(BundleCell::String(value.clone()))),
        parquet::record::Field::Long(value) => Ok(Some(BundleCell::Integer(*value))),
        parquet::record::Field::Double(value) if value.is_finite() => {
            Ok(Some(BundleCell::Float(*value)))
        }
        parquet::record::Field::Bool(value) => Ok(Some(BundleCell::Boolean(*value))),
        parquet::record::Field::TimestampMicros(value) => Ok(Some(BundleCell::Time(*value))),
        other => Err(OcelError::new(format!(
            "unsupported Parquet value in OCEL bundle: {other:?}"
        ))),
    }
}

fn bundle_required_string(row: &BundleRow, column: &str) -> OcelResult<String> {
    match row.get(column).and_then(Option::as_ref) {
        Some(BundleCell::String(value)) => Ok(value.clone()),
        _ => Err(OcelError::new(format!(
            "OCEL bundle required string column '{column}' contains a null or invalid value"
        ))),
    }
}

fn bundle_required_time(row: &BundleRow, column: &str) -> OcelResult<i64> {
    match row.get(column).and_then(Option::as_ref) {
        Some(BundleCell::Time(value)) => Ok(*value),
        _ => Err(OcelError::new(format!(
            "OCEL bundle required timestamp column '{column}' contains a null or invalid value"
        ))),
    }
}

fn bundle_event_attributes(
    row: &BundleRow,
    attributes: &[SourceAttributeDef],
) -> OcelResult<Vec<SourceAttribute>> {
    attributes
        .iter()
        .filter_map(|attribute| {
            bundle_attribute_source_value(row, attribute)
                .transpose()
                .map(|value| {
                    value.map(|value| SourceAttribute {
                        name: attribute.name.clone(),
                        value,
                    })
                })
        })
        .collect()
}

fn bundle_attribute_source_value(
    row: &BundleRow,
    attribute: &SourceAttributeDef,
) -> OcelResult<Option<SourceValue>> {
    let Some(cell) = row.get(&attribute.name).and_then(Option::as_ref) else {
        return Ok(None);
    };
    let attr_type = AttrType::parse(&attribute.attr_type)?;
    let value = match (attr_type, cell) {
        (AttrType::String, BundleCell::String(value)) => SourceValue::String(value.clone()),
        (AttrType::Integer, BundleCell::Integer(value)) => SourceValue::Integer(*value),
        (AttrType::Float, BundleCell::Float(value)) => SourceValue::Float(*value),
        (AttrType::Boolean, BundleCell::Boolean(value)) => SourceValue::Boolean(*value),
        (AttrType::Time, BundleCell::Time(value)) => {
            SourceValue::String(format_timestamp_micros(*value)?)
        }
        _ => {
            return Err(OcelError::new(format!(
                "OCEL bundle value for attribute '{}' does not match declared type '{}'",
                attribute.name, attribute.attr_type
            )));
        }
    };
    Ok(Some(value))
}

fn write_bundle_parquet(columns: &[BundleColumnSpec], rows: &[BundleRow]) -> OcelResult<Vec<u8>> {
    for column in columns.iter().filter(|column| column.required) {
        if rows
            .iter()
            .any(|row| row.get(&column.name).and_then(Option::as_ref).is_none())
        {
            return Err(OcelError::new(format!(
                "required Parquet column '{}' contains a null value",
                column.name
            )));
        }
    }
    let fields = columns
        .iter()
        .map(|column| {
            let physical_type = match column.column_type {
                BundleColumnType::String => parquet::basic::Type::BYTE_ARRAY,
                BundleColumnType::Integer | BundleColumnType::Time => parquet::basic::Type::INT64,
                BundleColumnType::Float => parquet::basic::Type::DOUBLE,
                BundleColumnType::Boolean => parquet::basic::Type::BOOLEAN,
            };
            let logical_type = match column.column_type {
                BundleColumnType::String => Some(parquet::basic::LogicalType::String),
                BundleColumnType::Time => Some(parquet::basic::LogicalType::timestamp(
                    true,
                    parquet::basic::TimeUnit::MICROS,
                )),
                BundleColumnType::Integer
                | BundleColumnType::Float
                | BundleColumnType::Boolean => None,
            };
            parquet::schema::types::Type::primitive_type_builder(&column.name, physical_type)
                .with_repetition(if column.required {
                    parquet::basic::Repetition::REQUIRED
                } else {
                    parquet::basic::Repetition::OPTIONAL
                })
                .with_logical_type(logical_type)
                .build()
                .map(std::sync::Arc::new)
                .map_err(|err| OcelError::new(format!("could not build Parquet schema: {err}")))
        })
        .collect::<OcelResult<Vec<_>>>()?;
    let schema = parquet::schema::types::Type::group_type_builder("ocel")
        .with_fields(fields)
        .build()
        .map(std::sync::Arc::new)
        .map_err(|err| OcelError::new(format!("could not build Parquet schema: {err}")))?;
    let properties = parquet::file::properties::WriterProperties::builder()
        .set_compression(parquet::basic::Compression::SNAPPY)
        .build();
    let mut output = Vec::new();
    let mut writer = parquet::file::writer::SerializedFileWriter::new(
        &mut output,
        schema,
        std::sync::Arc::new(properties),
    )
    .map_err(|err| OcelError::new(format!("could not create Parquet writer: {err}")))?;
    if !rows.is_empty() {
        let mut row_group = writer
            .next_row_group()
            .map_err(|err| OcelError::new(format!("could not create Parquet row group: {err}")))?;
        for column in columns {
            let mut column_writer = row_group
                .next_column()
                .map_err(|err| OcelError::new(format!("could not create Parquet column: {err}")))?
                .ok_or_else(|| OcelError::new("Parquet writer returned too few columns"))?;
            let definition_levels = (!column.required).then(|| {
                rows.iter()
                    .map(|row| {
                        if row.get(&column.name).and_then(Option::as_ref).is_some() {
                            1
                        } else {
                            0
                        }
                    })
                    .collect::<Vec<i16>>()
            });
            match column.column_type {
                BundleColumnType::String => {
                    let values = rows
                        .iter()
                        .filter_map(|row| row.get(&column.name).and_then(Option::as_ref))
                        .map(|value| match value {
                            BundleCell::String(value) => {
                                Ok(parquet::data_type::ByteArray::from(value.as_str()))
                            }
                            _ => Err(OcelError::new(format!(
                                "invalid string value for Parquet column '{}'",
                                column.name
                            ))),
                        })
                        .collect::<OcelResult<Vec<_>>>()?;
                    column_writer
                        .typed::<parquet::data_type::ByteArrayType>()
                        .write_batch(&values, definition_levels.as_deref(), None)
                }
                BundleColumnType::Integer | BundleColumnType::Time => {
                    let values = rows
                        .iter()
                        .filter_map(|row| row.get(&column.name).and_then(Option::as_ref))
                        .map(|value| match (column.column_type, value) {
                            (BundleColumnType::Integer, BundleCell::Integer(value))
                            | (BundleColumnType::Time, BundleCell::Time(value)) => Ok(*value),
                            _ => Err(OcelError::new(format!(
                                "invalid integer/timestamp value for Parquet column '{}'",
                                column.name
                            ))),
                        })
                        .collect::<OcelResult<Vec<_>>>()?;
                    column_writer
                        .typed::<parquet::data_type::Int64Type>()
                        .write_batch(&values, definition_levels.as_deref(), None)
                }
                BundleColumnType::Float => {
                    let values = rows
                        .iter()
                        .filter_map(|row| row.get(&column.name).and_then(Option::as_ref))
                        .map(|value| match value {
                            BundleCell::Float(value) => Ok(*value),
                            _ => Err(OcelError::new(format!(
                                "invalid float value for Parquet column '{}'",
                                column.name
                            ))),
                        })
                        .collect::<OcelResult<Vec<_>>>()?;
                    column_writer
                        .typed::<parquet::data_type::DoubleType>()
                        .write_batch(&values, definition_levels.as_deref(), None)
                }
                BundleColumnType::Boolean => {
                    let values = rows
                        .iter()
                        .filter_map(|row| row.get(&column.name).and_then(Option::as_ref))
                        .map(|value| match value {
                            BundleCell::Boolean(value) => Ok(*value),
                            _ => Err(OcelError::new(format!(
                                "invalid boolean value for Parquet column '{}'",
                                column.name
                            ))),
                        })
                        .collect::<OcelResult<Vec<_>>>()?;
                    column_writer
                        .typed::<parquet::data_type::BoolType>()
                        .write_batch(&values, definition_levels.as_deref(), None)
                }
            }
            .map_err(|err| OcelError::new(format!("could not write Parquet column: {err}")))?;
            column_writer
                .close()
                .map_err(|err| OcelError::new(format!("could not close Parquet column: {err}")))?;
        }
        row_group
            .close()
            .map_err(|err| OcelError::new(format!("could not close Parquet row group: {err}")))?;
    }
    writer
        .close()
        .map_err(|err| OcelError::new(format!("could not close Parquet file: {err}")))?;
    Ok(output)
}
