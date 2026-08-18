impl CompactOcelLog {
    fn export_bundle(&self) -> OcelResult<Vec<u8>> {
        self.validate_bundle_attribute_names()?;
        let mut metadata = BundleMetadata {
            ocel_version: "2.0".to_owned(),
            bundle_format_version: "1.0".to_owned(),
            storage_format: "parquet".to_owned(),
            event_types: BTreeMap::new(),
            object_types: BTreeMap::new(),
            relations: BundleRelationMetadata {
                e2o: "relations/e2o.parquet".to_owned(),
                o2o: "relations/o2o.parquet".to_owned(),
            },
        };
        for type_def in &self.event_types {
            let type_name = self.pool.resolve(type_def.name).to_owned();
            metadata.event_types.insert(
                type_name.clone(),
                BundleTypeMetadata {
                    file: format!(
                        "events/event_{}.parquet",
                        percent_encode_bundle_type(&type_name)
                    ),
                    changes_file: None,
                    attributes: self.bundle_attribute_metadata(type_def),
                },
            );
        }
        for type_def in &self.object_types {
            let type_name = self.pool.resolve(type_def.name).to_owned();
            let encoded = percent_encode_bundle_type(&type_name);
            metadata.object_types.insert(
                type_name,
                BundleTypeMetadata {
                    file: format!("objects/object_{encoded}.parquet"),
                    changes_file: Some(format!(
                        "object_changes/object_changes_{encoded}.parquet"
                    )),
                    attributes: self.bundle_attribute_metadata(type_def),
                },
            );
        }

        let cursor = std::io::Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let metadata_json = serde_json::to_vec_pretty(&metadata)
            .map_err(|err| OcelError::new(format!("could not serialize OCEL bundle metadata: {err}")))?;
        write_bundle_zip_entry(&mut archive, "ocel-meta.json", &metadata_json)?;

        let events_by_type = self
            .event_types
            .iter()
            .map(|type_def| {
                (
                    type_def.name,
                    self.events
                        .iter()
                        .filter(|event| event.type_name == type_def.name)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        for type_def in &self.event_types {
            let type_name = self.pool.resolve(type_def.name);
            let type_metadata = &metadata.event_types[type_name];
            let mut columns = vec![
                bundle_required_column("ocel_id", BundleColumnType::String),
                bundle_required_column("ocel_time", BundleColumnType::Time),
            ];
            columns.extend(self.bundle_columns(type_def));
            let rows = events_by_type[&type_def.name]
                .iter()
                .map(|event| {
                    let values = event
                        .attributes
                        .iter()
                        .map(|attribute| (attribute.name, &attribute.value))
                        .collect::<HashMap<_, _>>();
                    let mut row = BundleRow::new();
                    row.insert(
                        "ocel_id".to_owned(),
                        Some(BundleCell::String(self.pool.resolve(event.id).to_owned())),
                    );
                    row.insert(
                        "ocel_time".to_owned(),
                        Some(BundleCell::Time(event.time_micros)),
                    );
                    for attribute in &type_def.attributes {
                        row.insert(
                            self.pool.resolve(attribute.name).to_owned(),
                            values
                                .get(&attribute.name)
                                .map(|value| self.attr_value_to_bundle(value, attribute.attr_type))
                                .transpose()?,
                        );
                    }
                    Ok(row)
                })
                .collect::<OcelResult<Vec<_>>>()?;
            let parquet = write_bundle_parquet(&columns, &rows)?;
            write_bundle_zip_entry(&mut archive, &type_metadata.file, &parquet)?;
        }

        let objects_by_type = self
            .object_types
            .iter()
            .map(|type_def| {
                (
                    type_def.name,
                    self.objects
                        .iter()
                        .filter(|object| object.type_name == type_def.name)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        for type_def in &self.object_types {
            let type_name = self.pool.resolve(type_def.name);
            let type_metadata = &metadata.object_types[type_name];
            let mut object_columns = vec![bundle_required_column(
                "ocel_id",
                BundleColumnType::String,
            )];
            object_columns.extend(self.bundle_columns(type_def));
            let mut change_columns = vec![
                bundle_required_column("ocel_id", BundleColumnType::String),
                bundle_required_column("ocel_time", BundleColumnType::Time),
                bundle_required_column("ocel_changed_field", BundleColumnType::String),
            ];
            change_columns.extend(self.bundle_columns(type_def));

            let mut object_rows = Vec::new();
            let mut change_rows = Vec::new();
            for object in &objects_by_type[&type_def.name] {
                let initial_values = self.bundle_object_values_at(object, 0)?;
                let mut row = BundleRow::new();
                row.insert(
                    "ocel_id".to_owned(),
                    Some(BundleCell::String(self.pool.resolve(object.id).to_owned())),
                );
                for attribute in &type_def.attributes {
                    row.insert(
                        self.pool.resolve(attribute.name).to_owned(),
                        initial_values.get(&attribute.name).cloned(),
                    );
                }
                object_rows.push(row);

                let mut seen_changes = HashMap::<(Symbol, i64), BundleCell>::new();
                for attribute in &object.attributes {
                    if attribute.time_micros == 0 {
                        continue;
                    }
                    let attr_type = type_def
                        .attributes
                        .iter()
                        .find(|definition| definition.name == attribute.name)
                        .map(|definition| definition.attr_type)
                        .ok_or_else(|| {
                            OcelError::new(format!(
                                "object attribute '{}' has no type definition",
                                self.pool.resolve(attribute.name)
                            ))
                        })?;
                    let value = self.attr_value_to_bundle(&attribute.value, attr_type)?;
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
                    let mut change_row = BundleRow::new();
                    change_row.insert(
                        "ocel_id".to_owned(),
                        Some(BundleCell::String(self.pool.resolve(object.id).to_owned())),
                    );
                    change_row.insert(
                        "ocel_time".to_owned(),
                        Some(BundleCell::Time(attribute.time_micros)),
                    );
                    change_row.insert(
                        "ocel_changed_field".to_owned(),
                        Some(BundleCell::String(
                            self.pool.resolve(attribute.name).to_owned(),
                        )),
                    );
                    for definition in &type_def.attributes {
                        change_row.insert(
                            self.pool.resolve(definition.name).to_owned(),
                            (definition.name == attribute.name).then(|| value.clone()),
                        );
                    }
                    change_rows.push(change_row);
                }
            }
            let object_parquet = write_bundle_parquet(&object_columns, &object_rows)?;
            write_bundle_zip_entry(&mut archive, &type_metadata.file, &object_parquet)?;
            let changes_file = type_metadata
                .changes_file
                .as_ref()
                .expect("export metadata always has changesFile for objects");
            let change_parquet = write_bundle_parquet(&change_columns, &change_rows)?;
            write_bundle_zip_entry(&mut archive, changes_file, &change_parquet)?;
        }

        let e2o_columns = vec![
            bundle_required_column("ocel_event_id", BundleColumnType::String),
            bundle_required_column("ocel_object_id", BundleColumnType::String),
            bundle_required_column("ocel_qualifier", BundleColumnType::String),
        ];
        let mut e2o_rows = Vec::new();
        for event in &self.events {
            for relationship in &event.relationships {
                let mut row = BundleRow::new();
                row.insert(
                    "ocel_event_id".to_owned(),
                    Some(BundleCell::String(self.pool.resolve(event.id).to_owned())),
                );
                row.insert(
                    "ocel_object_id".to_owned(),
                    Some(BundleCell::String(
                        self.pool.resolve(relationship.object_id).to_owned(),
                    )),
                );
                row.insert(
                    "ocel_qualifier".to_owned(),
                    Some(BundleCell::String(
                        self.pool.resolve(relationship.qualifier).to_owned(),
                    )),
                );
                e2o_rows.push(row);
            }
        }
        let e2o_parquet = write_bundle_parquet(&e2o_columns, &e2o_rows)?;
        write_bundle_zip_entry(&mut archive, &metadata.relations.e2o, &e2o_parquet)?;

        let o2o_columns = vec![
            bundle_required_column("ocel_source_id", BundleColumnType::String),
            bundle_required_column("ocel_target_id", BundleColumnType::String),
            bundle_required_column("ocel_qualifier", BundleColumnType::String),
        ];
        let mut o2o_rows = Vec::new();
        for object in &self.objects {
            for relationship in &object.relationships {
                let mut row = BundleRow::new();
                row.insert(
                    "ocel_source_id".to_owned(),
                    Some(BundleCell::String(self.pool.resolve(object.id).to_owned())),
                );
                row.insert(
                    "ocel_target_id".to_owned(),
                    Some(BundleCell::String(
                        self.pool.resolve(relationship.object_id).to_owned(),
                    )),
                );
                row.insert(
                    "ocel_qualifier".to_owned(),
                    Some(BundleCell::String(
                        self.pool.resolve(relationship.qualifier).to_owned(),
                    )),
                );
                o2o_rows.push(row);
            }
        }
        let o2o_parquet = write_bundle_parquet(&o2o_columns, &o2o_rows)?;
        write_bundle_zip_entry(&mut archive, &metadata.relations.o2o, &o2o_parquet)?;

        let cursor = archive
            .finish()
            .map_err(|err| OcelError::new(format!("could not finish OCEL bundle ZIP: {err}")))?;
        Ok(cursor.into_inner())
    }

    fn validate_bundle_attribute_names(&self) -> OcelResult<()> {
        for type_def in &self.event_types {
            for attribute in &type_def.attributes {
                let name = self.pool.resolve(attribute.name);
                if matches!(name, "ocel_id" | "ocel_time") {
                    return Err(OcelError::new(format!(
                        "event attribute '{name}' conflicts with a required bundle column"
                    )));
                }
            }
        }
        for type_def in &self.object_types {
            for attribute in &type_def.attributes {
                let name = self.pool.resolve(attribute.name);
                if matches!(name, "ocel_id" | "ocel_time" | "ocel_changed_field") {
                    return Err(OcelError::new(format!(
                        "object attribute '{name}' conflicts with a required bundle column"
                    )));
                }
            }
        }
        Ok(())
    }

    fn bundle_attribute_metadata(&self, type_def: &TypeDef) -> Vec<BundleAttributeMetadata> {
        type_def
            .attributes
            .iter()
            .map(|attribute| BundleAttributeMetadata {
                name: self.pool.resolve(attribute.name).to_owned(),
                attr_type: attribute.attr_type.as_str().to_owned(),
            })
            .collect()
    }

    fn bundle_columns(&self, type_def: &TypeDef) -> Vec<BundleColumnSpec> {
        type_def
            .attributes
            .iter()
            .map(|attribute| BundleColumnSpec {
                name: self.pool.resolve(attribute.name).to_owned(),
                column_type: BundleColumnType::from_attr_type(attribute.attr_type),
                required: false,
            })
            .collect()
    }

    fn attr_value_to_bundle(
        &self,
        value: &AttrValue,
        attr_type: AttrType,
    ) -> OcelResult<BundleCell> {
        match (attr_type, value) {
            (AttrType::String, AttrValue::String(symbol)) => {
                Ok(BundleCell::String(self.pool.resolve(*symbol).to_owned()))
            }
            (AttrType::Time, AttrValue::Time(micros)) => Ok(BundleCell::Time(*micros)),
            (AttrType::Integer, AttrValue::Integer(value)) => Ok(BundleCell::Integer(*value)),
            (AttrType::Float, AttrValue::Float(value)) if value.is_finite() => {
                Ok(BundleCell::Float(*value))
            }
            (AttrType::Boolean, AttrValue::Boolean(value)) => Ok(BundleCell::Boolean(*value)),
            _ => Err(OcelError::new(
                "attribute value does not match its OCEL type definition",
            )),
        }
    }

    fn bundle_object_values_at(
        &self,
        object: &Object,
        time_micros: i64,
    ) -> OcelResult<HashMap<Symbol, BundleCell>> {
        let type_def = self
            .object_types
            .iter()
            .find(|type_def| type_def.name == object.type_name)
            .ok_or_else(|| OcelError::new("object has no type definition"))?;
        let attr_types = type_def
            .attributes
            .iter()
            .map(|attribute| (attribute.name, attribute.attr_type))
            .collect::<HashMap<_, _>>();
        let mut result = HashMap::new();
        for attribute in &object.attributes {
            if attribute.time_micros != time_micros {
                continue;
            }
            let attr_type = attr_types.get(&attribute.name).copied().ok_or_else(|| {
                OcelError::new(format!(
                    "object attribute '{}' has no type definition",
                    self.pool.resolve(attribute.name)
                ))
            })?;
            let value = self.attr_value_to_bundle(&attribute.value, attr_type)?;
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

fn percent_encode_bundle_type(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'.' | b'_' | b'-') {
            output.push(*byte as char);
        } else {
            write!(output, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    output
}

fn write_bundle_zip_entry<W: std::io::Write + std::io::Seek>(
    archive: &mut zip::ZipWriter<W>,
    path: &str,
    content: &[u8],
) -> OcelResult<()> {
    validate_bundle_path(path)?;
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    archive
        .start_file(path, options)
        .map_err(|err| OcelError::new(format!("could not add '{path}' to OCEL bundle: {err}")))?;
    std::io::Write::write_all(archive, content)
        .map_err(|err| OcelError::new(format!("could not write '{path}' to OCEL bundle: {err}")))
}
