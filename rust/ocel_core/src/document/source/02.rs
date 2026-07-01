
fn parse_json(input: &str) -> OcelResult<SourceLog> {
    let raw: RawJsonLog = serde_json::from_str(input)
        .map_err(|err| OcelError::new(format!("invalid OCEL JSON: {err}")))?;

    Ok(SourceLog {
        event_types: raw
            .event_types
            .into_iter()
            .map(|event_type| SourceType {
                name: event_type.name,
                attributes: event_type
                    .attributes
                    .into_iter()
                    .map(|attribute| SourceAttributeDef {
                        name: attribute.name,
                        attr_type: attribute.attr_type,
                    })
                    .collect(),
            })
            .collect(),
        object_types: raw
            .object_types
            .into_iter()
            .map(|object_type| SourceType {
                name: object_type.name,
                attributes: object_type
                    .attributes
                    .into_iter()
                    .map(|attribute| SourceAttributeDef {
                        name: attribute.name,
                        attr_type: attribute.attr_type,
                    })
                    .collect(),
            })
            .collect(),
        events: raw
            .events
            .into_iter()
            .map(|event| {
                Ok(SourceEvent {
                    id: event.id,
                    type_name: event.type_name,
                    time: event.time,
                    attributes: event
                        .attributes
                        .into_iter()
                        .map(|attribute| {
                            Ok(SourceAttribute {
                                name: attribute.name,
                                value: source_value_from_json(attribute.value)?,
                            })
                        })
                        .collect::<OcelResult<Vec<_>>>()?,
                    relationships: event
                        .relationships
                        .into_iter()
                        .map(|relationship| SourceRelationship {
                            object_id: relationship.object_id,
                            qualifier: relationship.qualifier,
                        })
                        .collect(),
                })
            })
            .collect::<OcelResult<Vec<_>>>()?,
        objects: raw
            .objects
            .into_iter()
            .map(|object| {
                Ok(SourceObject {
                    id: object.id,
                    type_name: object.type_name,
                    attributes: object
                        .attributes
                        .into_iter()
                        .map(|attribute| {
                            Ok(SourceTimedAttribute {
                                name: attribute.name,
                                time: attribute.time,
                                value: source_value_from_json(attribute.value)?,
                            })
                        })
                        .collect::<OcelResult<Vec<_>>>()?,
                    relationships: object
                        .relationships
                        .into_iter()
                        .map(|relationship| SourceRelationship {
                            object_id: relationship.object_id,
                            qualifier: relationship.qualifier,
                        })
                        .collect(),
                })
            })
            .collect::<OcelResult<Vec<_>>>()?,
    })
}

fn source_value_from_json(value: Value) -> OcelResult<SourceValue> {
    match value {
        Value::String(value) => Ok(SourceValue::String(value)),
        Value::Bool(value) => Ok(SourceValue::Boolean(value)),
        Value::Number(value) => {
            if let Some(number) = value.as_i64() {
                Ok(SourceValue::Integer(number))
            } else if let Some(number) = value.as_f64() {
                if number.is_finite() {
                    Ok(SourceValue::Float(number))
                } else {
                    Err(OcelError::new(
                        "JSON attribute contains a non-finite number",
                    ))
                }
            } else {
                Err(OcelError::new(
                    "JSON attribute contains an unsupported number",
                ))
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => {
            Err(OcelError::new("OCEL attributes must be scalar JSON values"))
        }
    }
}

fn parse_xml(input: &str) -> OcelResult<SourceLog> {
    let document =
        Document::parse(input).map_err(|err| OcelError::new(format!("invalid OCEL XML: {err}")))?;
    let root = document.root_element();
    if root.tag_name().name() != "log" {
        return Err(OcelError::new("OCEL XML root element must be <log>"));
    }

    let event_types = parse_xml_types(required_child(root, "event-types")?, "event-type")?;
    let object_types = parse_xml_types(required_child(root, "object-types")?, "object-type")?;
    let events = parse_xml_events(required_child(root, "events")?)?;
    let objects = parse_xml_objects(required_child(root, "objects")?)?;

    Ok(SourceLog {
        event_types,
        object_types,
        events,
        objects,
    })
}

fn parse_xml_types(parent: Node<'_, '_>, type_tag: &str) -> OcelResult<Vec<SourceType>> {
    let mut types = Vec::new();
    for node in element_children_named(parent, type_tag) {
        let attributes = optional_child(node, "attributes")
            .map(|attributes_node| {
                element_children_named(attributes_node, "attribute")
                    .map(|attribute_node| {
                        Ok(SourceAttributeDef {
                            name: required_attr(attribute_node, "name")?.to_owned(),
                            attr_type: required_attr(attribute_node, "type")?.to_owned(),
                        })
                    })
                    .collect::<OcelResult<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        types.push(SourceType {
            name: required_attr(node, "name")?.to_owned(),
            attributes,
        });
    }
    Ok(types)
}
