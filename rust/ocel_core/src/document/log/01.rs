impl CompactOcelLog {
    fn from_input(input: &str, format_hint: Option<&str>) -> OcelResult<Self> {
        let format = detect_format(input, format_hint)?;
        let source = match format {
            OcelFormat::Json => parse_json(input)?,
            OcelFormat::Xml => parse_xml(input)?,
        };
        Self::from_source(source, format)
    }

    fn from_bytes(input: &[u8], format_hint: Option<&str>) -> OcelResult<Self> {
        let text = decode_ocel_bytes(input)?;
        Self::from_input(&text, format_hint)
    }

    fn from_source(source: SourceLog, format: OcelFormat) -> OcelResult<Self> {
        let mut pool = StringPool::default();
        let mut event_type_names = HashSet::new();
        let mut object_type_names = HashSet::new();
        let mut event_attr_types = HashMap::new();
        let mut object_attr_types = HashMap::new();
        let mut event_types = Vec::with_capacity(source.event_types.len());
        let mut object_types = Vec::with_capacity(source.object_types.len());

        for source_type in &source.event_types {
            if !event_type_names.insert(source_type.name.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate event type '{}'",
                    source_type.name
                )));
            }
            event_types.push(compact_type_def(
                source_type,
                &mut pool,
                &mut event_attr_types,
                "event type",
            )?);
        }

        for source_type in &source.object_types {
            if !object_type_names.insert(source_type.name.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate object type '{}'",
                    source_type.name
                )));
            }
            object_types.push(compact_type_def(
                source_type,
                &mut pool,
                &mut object_attr_types,
                "object type",
            )?);
        }

        let mut object_ids = HashSet::new();
        for object in &source.objects {
            if !object_ids.insert(object.id.clone()) {
                return Err(OcelError::new(format!(
                    "duplicate object id '{}'",
                    object.id
                )));
            }
            if !object_type_names.contains(&object.type_name) {
                return Err(OcelError::new(format!(
                    "object '{}' references unknown object type '{}'",
                    object.id, object.type_name
                )));
            }
        }

        let mut event_ids = HashSet::new();
        for event in &source.events {
            if !event_ids.insert(event.id.clone()) {
                return Err(OcelError::new(format!("duplicate event id '{}'", event.id)));
            }
            if !event_type_names.contains(&event.type_name) {
                return Err(OcelError::new(format!(
                    "event '{}' references unknown event type '{}'",
                    event.id, event.type_name
                )));
            }
            for rel in &event.relationships {
                if !object_ids.contains(&rel.object_id) {
                    return Err(OcelError::new(format!(
                        "event '{}' references unknown object '{}'",
                        event.id, rel.object_id
                    )));
                }
            }
        }

        let mut objects = Vec::with_capacity(source.objects.len());
        let mut object_index = HashMap::with_capacity(source.objects.len());

        for source_object in &source.objects {
            for rel in &source_object.relationships {
                if !object_ids.contains(&rel.object_id) {
                    return Err(OcelError::new(format!(
                        "object '{}' references unknown object '{}'",
                        source_object.id, rel.object_id
                    )));
                }
            }

            let id = pool.intern(&source_object.id);
            let object = Object {
                id,
                type_name: pool.intern(&source_object.type_name),
                attributes: compact_timed_attributes(
                    &source_object.attributes,
                    &source_object.type_name,
                    &object_attr_types,
                    &mut pool,
                )?,
                relationships: compact_relationships(&source_object.relationships, &mut pool),
                lifecycle: Vec::new(),
            };
            object_index.insert(id, objects.len());
            objects.push(object);
        }

        let mut events = Vec::with_capacity(source.events.len());
        for source_event in &source.events {
            let time_ms = parse_timestamp_ms(&source_event.time)?;
            let event = Event {
                id: pool.intern(&source_event.id),
                type_name: pool.intern(&source_event.type_name),
                time_ms,
                attributes: compact_attributes(
                    &source_event.attributes,
                    &source_event.type_name,
                    &event_attr_types,
                    &mut pool,
                )?,
                relationships: compact_relationships(&source_event.relationships, &mut pool),
            };
            events.push(event);
        }

        for (event_index, event) in events.iter().enumerate() {
            for rel in &event.relationships {
                if let Some(object_pos) = object_index.get(&rel.object_id) {
                    objects[*object_pos].lifecycle.push(event_index);
                }
            }
        }

        for object in &mut objects {
            object
                .lifecycle
                .sort_by_key(|event_index| (events[*event_index].time_ms, *event_index));
        }

        Ok(Self {
            format,
            pool: pool.finish(),
            event_types,
            object_types,
            events,
            objects,
            object_index,
            state_leading_object_type: None,
        })
    }
}
