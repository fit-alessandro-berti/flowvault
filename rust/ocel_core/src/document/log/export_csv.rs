impl CompactOcelLog {
    fn export_csv(&self) -> OcelResult<String> {
        self.validate_csv_representability()?;
        let object_type_names = self
            .object_types
            .iter()
            .map(|type_def| self.pool.resolve(type_def.name).to_owned())
            .collect::<Vec<_>>();
        let mut event_attribute_names = Vec::new();
        for type_def in &self.event_types {
            for attribute in &type_def.attributes {
                let name = self.pool.resolve(attribute.name).to_owned();
                if !event_attribute_names.contains(&name) {
                    event_attribute_names.push(name);
                }
            }
        }

        let mut writer = csv::WriterBuilder::new()
            .terminator(csv::Terminator::CRLF)
            .from_writer(Vec::new());
        let mut header = vec![
            "id".to_owned(),
            "activity".to_owned(),
            "timestamp".to_owned(),
        ];
        header.extend(
            object_type_names
                .iter()
                .map(|type_name| format!("ot:{type_name}")),
        );
        header.extend(event_attribute_names.iter().cloned());
        writer
            .write_record(&header)
            .map_err(|err| OcelError::new(format!("could not write OCEL CSV header: {err}")))?;

        let object_types_by_id = self
            .objects
            .iter()
            .map(|object| (object.id, self.pool.resolve(object.type_name)))
            .collect::<HashMap<_, _>>();
        let object_type_positions = object_type_names
            .iter()
            .enumerate()
            .map(|(index, name)| (name.as_str(), index))
            .collect::<HashMap<_, _>>();
        let event_attribute_positions = event_attribute_names
            .iter()
            .enumerate()
            .map(|(index, name)| (name.as_str(), index))
            .collect::<HashMap<_, _>>();

        let mut established_objects = HashSet::new();
        let mut event_order = (0..self.events.len()).collect::<Vec<_>>();
        event_order.sort_by_key(|index| (self.events[*index].time_micros, *index));
        for event_index in event_order {
            let event = &self.events[event_index];
            let mut row = vec![String::new(); header.len()];
            row[0] = self.pool.resolve(event.id).to_owned();
            row[1] = self.pool.resolve(event.type_name).to_owned();
            row[2] = format_timestamp_micros(event.time_micros)?;
            let mut references_by_type = vec![Vec::<String>::new(); object_type_names.len()];
            for relationship in &event.relationships {
                let type_name = object_types_by_id.get(&relationship.object_id).ok_or_else(|| {
                    OcelError::new(format!(
                        "event '{}' references unknown object '{}'",
                        self.pool.resolve(event.id),
                        self.pool.resolve(relationship.object_id)
                    ))
                })?;
                let type_position = object_type_positions[type_name];
                references_by_type[type_position].push(self.csv_relationship_reference(relationship));
                established_objects.insert(relationship.object_id);
            }
            for (index, references) in references_by_type.into_iter().enumerate() {
                row[3 + index] = references.join("/");
            }
            for attribute in &event.attributes {
                let name = self.pool.resolve(attribute.name);
                let position = event_attribute_positions.get(name).ok_or_else(|| {
                    OcelError::new(format!("event attribute '{name}' has no CSV column"))
                })?;
                row[3 + object_type_names.len() + *position] =
                    self.csv_attribute_text(&attribute.value)?;
            }
            writer.write_record(&row).map_err(|err| {
                OcelError::new(format!("could not write OCEL CSV event row: {err}"))
            })?;
        }

        let mut object_order = (0..self.objects.len()).collect::<Vec<_>>();
        object_order.sort_by(|left, right| {
            let left_object = &self.objects[*left];
            let right_object = &self.objects[*right];
            (
                self.pool.resolve(left_object.type_name),
                self.pool.resolve(left_object.id),
            )
                .cmp(&(
                    self.pool.resolve(right_object.type_name),
                    self.pool.resolve(right_object.id),
                ))
        });
        for object_index in &object_order {
            let object = &self.objects[*object_index];
            let initial_attributes = self.csv_object_attributes_at(object, 0)?;
            if established_objects.contains(&object.id) && initial_attributes.is_empty() {
                continue;
            }
            let mut row = vec![String::new(); header.len()];
            let type_name = self.pool.resolve(object.type_name);
            let type_position = object_type_positions[type_name];
            row[3 + type_position] = self.csv_object_reference(
                self.pool.resolve(object.id),
                None,
                &initial_attributes,
            )?;
            writer.write_record(&row).map_err(|err| {
                OcelError::new(format!("could not write OCEL CSV object row: {err}"))
            })?;
        }

        let mut sources = object_order
            .iter()
            .filter(|index| !self.objects[**index].relationships.is_empty())
            .copied()
            .collect::<Vec<_>>();
        sources.sort_by_key(|index| self.pool.resolve(self.objects[*index].id));
        for source_index in sources {
            let source = &self.objects[source_index];
            let mut row = vec![String::new(); header.len()];
            row[0] = self.pool.resolve(source.id).to_owned();
            row[1] = "o2o".to_owned();
            let mut references_by_type = vec![Vec::<String>::new(); object_type_names.len()];
            for relationship in &source.relationships {
                let type_name = object_types_by_id.get(&relationship.object_id).ok_or_else(|| {
                    OcelError::new(format!(
                        "object '{}' references unknown object '{}'",
                        self.pool.resolve(source.id),
                        self.pool.resolve(relationship.object_id)
                    ))
                })?;
                references_by_type[object_type_positions[type_name]]
                    .push(self.csv_relationship_reference(relationship));
            }
            for (index, references) in references_by_type.into_iter().enumerate() {
                row[3 + index] = references.join("/");
            }
            writer.write_record(&row).map_err(|err| {
                OcelError::new(format!("could not write OCEL CSV object relationship row: {err}"))
            })?;
        }

        let mut changes = Vec::<(i64, usize, usize, usize)>::new();
        let mut sequence = 0usize;
        for (object_index, object) in self.objects.iter().enumerate() {
            for (attribute_index, attribute) in object.attributes.iter().enumerate() {
                if attribute.time_micros != 0 {
                    changes.push((attribute.time_micros, sequence, object_index, attribute_index));
                }
                sequence += 1;
            }
        }
        changes.sort_by_key(|(time, sequence, _, _)| (*time, *sequence));
        for (time_micros, _, object_index, attribute_index) in changes {
            let object = &self.objects[object_index];
            let attribute = &object.attributes[attribute_index];
            let mut attributes = Map::new();
            attributes.insert(
                self.pool.resolve(attribute.name).to_owned(),
                self.attr_value_to_json(&attribute.value)?,
            );
            let mut row = vec![String::new(); header.len()];
            row[2] = format_timestamp_micros(time_micros)?;
            let type_name = self.pool.resolve(object.type_name);
            row[3 + object_type_positions[type_name]] = self.csv_object_reference(
                self.pool.resolve(object.id),
                None,
                &attributes,
            )?;
            writer.write_record(&row).map_err(|err| {
                OcelError::new(format!("could not write OCEL CSV object attribute row: {err}"))
            })?;
        }

        let bytes = writer
            .into_inner()
            .map_err(|err| OcelError::new(format!("could not finish OCEL CSV export: {err}")))?;
        String::from_utf8(bytes)
            .map_err(|err| OcelError::new(format!("OCEL CSV export is not UTF-8: {err}")))
    }

    fn validate_csv_representability(&self) -> OcelResult<()> {
        for type_def in &self.event_types {
            let type_name = self.pool.resolve(type_def.name);
            if type_name.eq_ignore_ascii_case("o2o") {
                return Err(OcelError::new(
                    "the event type 'o2o' is not representable in OCEL CSV",
                ));
            }
            if type_name != type_name.trim() {
                return Err(OcelError::new(format!(
                    "event type '{type_name}' has leading/trailing whitespace and is not losslessly representable in OCEL CSV"
                )));
            }
            if type_name.is_empty() {
                return Err(OcelError::new(
                    "an empty event type is not representable in OCEL CSV",
                ));
            }
            let events = self
                .events
                .iter()
                .filter(|event| event.type_name == type_def.name)
                .collect::<Vec<_>>();
            if events.is_empty() {
                return Err(OcelError::new(format!(
                    "event type '{type_name}' has no events and cannot be reconstructed from OCEL CSV"
                )));
            }
            for attribute in &type_def.attributes {
                let name = self.pool.resolve(attribute.name);
                if matches!(name, "id" | "activity" | "timestamp") || name.starts_with("ot:") {
                    return Err(OcelError::new(format!(
                        "event attribute '{name}' conflicts with a reserved OCEL CSV column"
                    )));
                }
                if !events
                    .iter()
                    .any(|event| event.attributes.iter().any(|value| value.name == attribute.name))
                {
                    return Err(OcelError::new(format!(
                        "event attribute '{type_name}.{name}' has no values and cannot be reconstructed from OCEL CSV"
                    )));
                }
            }
        }
        for type_def in &self.object_types {
            let type_name = self.pool.resolve(type_def.name);
            if type_name.is_empty() || type_name != type_name.trim() {
                return Err(OcelError::new(format!(
                    "object type '{type_name}' is not losslessly representable in OCEL CSV"
                )));
            }
            for attribute in &type_def.attributes {
                if !self.objects.iter().any(|object| {
                    object.type_name == type_def.name
                        && object
                            .attributes
                            .iter()
                            .any(|value| value.name == attribute.name)
                }) {
                    return Err(OcelError::new(format!(
                        "object attribute '{type_name}.{}' has no values and cannot be reconstructed from OCEL CSV",
                        self.pool.resolve(attribute.name)
                    )));
                }
            }
        }
        for event in &self.events {
            self.validate_csv_identifier(self.pool.resolve(event.id), "event id")?;
            for attribute in &event.attributes {
                if let AttrValue::String(symbol) = &attribute.value {
                    if self.pool.resolve(*symbol).is_empty() {
                        return Err(OcelError::new(format!(
                            "event '{}' has an empty string attribute value, which is indistinguishable from a missing OCEL CSV cell",
                            self.pool.resolve(event.id)
                        )));
                    }
                }
            }
            for relationship in &event.relationships {
                self.validate_csv_trimmed_value(
                    self.pool.resolve(relationship.qualifier),
                    "relationship qualifier",
                )?;
            }
        }
        for object in &self.objects {
            self.validate_csv_identifier(self.pool.resolve(object.id), "object id")?;
            for relationship in &object.relationships {
                self.validate_csv_trimmed_value(
                    self.pool.resolve(relationship.qualifier),
                    "relationship qualifier",
                )?;
            }
        }
        Ok(())
    }

    fn validate_csv_identifier(&self, value: &str, label: &str) -> OcelResult<()> {
        if value.is_empty() {
            return Err(OcelError::new(format!(
                "an empty {label} is not representable in OCEL CSV"
            )));
        }
        self.validate_csv_trimmed_value(value, label)
    }

    fn validate_csv_trimmed_value(&self, value: &str, label: &str) -> OcelResult<()> {
        if value != value.trim() {
            return Err(OcelError::new(format!(
                "{label} '{value}' has leading/trailing whitespace and is not losslessly representable in OCEL CSV"
            )));
        }
        Ok(())
    }

    fn csv_relationship_reference(&self, relationship: &Relationship) -> String {
        let qualifier = self.pool.resolve(relationship.qualifier);
        let escaped_id = escape_csv_reference_part(self.pool.resolve(relationship.object_id));
        if qualifier.is_empty() {
            escaped_id
        } else {
            format!("{escaped_id}#{}", escape_csv_reference_part(qualifier))
        }
    }

    fn csv_object_reference(
        &self,
        object_id: &str,
        qualifier: Option<&str>,
        attributes: &Map<String, Value>,
    ) -> OcelResult<String> {
        let mut output = escape_csv_reference_part(object_id);
        if let Some(qualifier) = qualifier.filter(|qualifier| !qualifier.is_empty()) {
            output.push('#');
            output.push_str(&escape_csv_reference_part(qualifier));
        }
        if !attributes.is_empty() {
            output.push_str(&serde_json::to_string(attributes).map_err(|err| {
                OcelError::new(format!("could not serialize OCEL CSV object attributes: {err}"))
            })?);
        }
        Ok(output)
    }

    fn csv_object_attributes_at(&self, object: &Object, time_micros: i64) -> OcelResult<Map<String, Value>> {
        let mut attributes = Map::new();
        for attribute in &object.attributes {
            if attribute.time_micros != time_micros {
                continue;
            }
            let name = self.pool.resolve(attribute.name).to_owned();
            let value = self.attr_value_to_json(&attribute.value)?;
            if let Some(existing) = attributes.insert(name.clone(), value.clone()) {
                if existing != value {
                    return Err(OcelError::new(format!(
                        "object '{}' has conflicting values for attribute '{name}' at the same timestamp",
                        self.pool.resolve(object.id)
                    )));
                }
            }
        }
        Ok(attributes)
    }

    fn csv_attribute_text(&self, value: &AttrValue) -> OcelResult<String> {
        match value {
            AttrValue::String(symbol) => Ok(self.pool.resolve(*symbol).to_owned()),
            AttrValue::Time(micros) => format_timestamp_micros(*micros),
            AttrValue::Integer(value) => Ok(value.to_string()),
            AttrValue::Float(value) if value.is_finite() => {
                let mut text = value.to_string();
                if !text.contains(['.', 'e', 'E']) {
                    text.push_str(".0");
                }
                Ok(text)
            }
            AttrValue::Float(_) => Err(OcelError::new("cannot export non-finite float to CSV")),
            AttrValue::Boolean(value) => Ok(value.to_string()),
        }
    }
}

fn escape_csv_reference_part(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '/' | '#' | '{' | '\\') {
            output.push('\\');
        }
        output.push(character);
    }
    output
}
