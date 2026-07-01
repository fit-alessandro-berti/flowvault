struct SourceLog {
    event_types: Vec<SourceType>,
    object_types: Vec<SourceType>,
    events: Vec<SourceEvent>,
    objects: Vec<SourceObject>,
}

#[derive(Debug)]
struct SourceType {
    name: String,
    attributes: Vec<SourceAttributeDef>,
}

#[derive(Debug)]
struct SourceAttributeDef {
    name: String,
    attr_type: String,
}

#[derive(Debug)]
struct SourceEvent {
    id: String,
    type_name: String,
    time: String,
    attributes: Vec<SourceAttribute>,
    relationships: Vec<SourceRelationship>,
}

#[derive(Debug)]
struct SourceObject {
    id: String,
    type_name: String,
    attributes: Vec<SourceTimedAttribute>,
    relationships: Vec<SourceRelationship>,
}

#[derive(Debug)]
struct SourceAttribute {
    name: String,
    value: SourceValue,
}

#[derive(Debug)]
struct SourceTimedAttribute {
    name: String,
    time: String,
    value: SourceValue,
}

#[derive(Debug)]
struct SourceRelationship {
    object_id: String,
    qualifier: String,
}

#[derive(Debug)]
enum SourceValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug, Deserialize)]
struct RawJsonLog {
    #[serde(rename = "eventTypes")]
    event_types: Vec<RawJsonType>,
    #[serde(rename = "objectTypes")]
    object_types: Vec<RawJsonType>,
    events: Vec<RawJsonEvent>,
    objects: Vec<RawJsonObject>,
}

#[derive(Debug, Deserialize)]
struct RawJsonType {
    name: String,
    #[serde(default)]
    attributes: Vec<RawJsonAttributeDef>,
}

#[derive(Debug, Deserialize)]
struct RawJsonAttributeDef {
    name: String,
    #[serde(rename = "type")]
    attr_type: String,
}

#[derive(Debug, Deserialize)]
struct RawJsonEvent {
    id: String,
    #[serde(rename = "type")]
    type_name: String,
    time: String,
    #[serde(default)]
    attributes: Vec<RawJsonAttribute>,
    #[serde(default)]
    relationships: Vec<RawJsonRelationship>,
}

#[derive(Debug, Deserialize)]
struct RawJsonObject {
    id: String,
    #[serde(rename = "type")]
    type_name: String,
    #[serde(default)]
    attributes: Vec<RawJsonTimedAttribute>,
    #[serde(default)]
    relationships: Vec<RawJsonRelationship>,
}

#[derive(Debug, Deserialize)]
struct RawJsonAttribute {
    name: String,
    value: Value,
}

#[derive(Debug, Deserialize)]
struct RawJsonTimedAttribute {
    name: String,
    time: String,
    value: Value,
}

#[derive(Debug, Deserialize)]
struct RawJsonRelationship {
    #[serde(rename = "objectId")]
    object_id: String,
    #[serde(default)]
    qualifier: String,
}

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

fn parse_xml_events(parent: Node<'_, '_>) -> OcelResult<Vec<SourceEvent>> {
    let mut events = Vec::new();
    for node in element_children_named(parent, "event") {
        let attributes = optional_child(node, "attributes")
            .map(parse_xml_attributes)
            .transpose()?
            .unwrap_or_default();
        let relationships = optional_child(node, "objects")
            .map(parse_xml_relationships)
            .transpose()?
            .unwrap_or_default();
        events.push(SourceEvent {
            id: required_attr(node, "id")?.to_owned(),
            type_name: required_attr(node, "type")?.to_owned(),
            time: required_attr(node, "time")?.to_owned(),
            attributes,
            relationships,
        });
    }
    Ok(events)
}

fn parse_xml_objects(parent: Node<'_, '_>) -> OcelResult<Vec<SourceObject>> {
    let mut objects = Vec::new();
    for node in element_children_named(parent, "object") {
        let attributes = optional_child(node, "attributes")
            .map(parse_xml_timed_attributes)
            .transpose()?
            .unwrap_or_default();
        let relationships = optional_child(node, "objects")
            .map(parse_xml_relationships)
            .transpose()?
            .unwrap_or_default();
        objects.push(SourceObject {
            id: required_attr(node, "id")?.to_owned(),
            type_name: required_attr(node, "type")?.to_owned(),
            attributes,
            relationships,
        });
    }
    Ok(objects)
}

fn parse_xml_attributes(parent: Node<'_, '_>) -> OcelResult<Vec<SourceAttribute>> {
    element_children_named(parent, "attribute")
        .map(|attribute_node| {
            Ok(SourceAttribute {
                name: required_attr(attribute_node, "name")?.to_owned(),
                value: SourceValue::String(attribute_node.text().unwrap_or("").to_owned()),
            })
        })
        .collect()
}

fn parse_xml_timed_attributes(parent: Node<'_, '_>) -> OcelResult<Vec<SourceTimedAttribute>> {
    element_children_named(parent, "attribute")
        .map(|attribute_node| {
            Ok(SourceTimedAttribute {
                name: required_attr(attribute_node, "name")?.to_owned(),
                time: required_attr(attribute_node, "time")?.to_owned(),
                value: SourceValue::String(attribute_node.text().unwrap_or("").to_owned()),
            })
        })
        .collect()
}

fn parse_xml_relationships(parent: Node<'_, '_>) -> OcelResult<Vec<SourceRelationship>> {
    element_children_named(parent, "relationship")
        .map(|relationship_node| {
            Ok(SourceRelationship {
                object_id: required_attr(relationship_node, "object-id")?.to_owned(),
                qualifier: relationship_node
                    .attribute("qualifier")
                    .or_else(|| relationship_node.attribute("relationship"))
                    .unwrap_or("")
                    .to_owned(),
            })
        })
        .collect()
}

fn required_child<'a, 'input>(parent: Node<'a, 'input>, tag: &str) -> OcelResult<Node<'a, 'input>> {
    optional_child(parent, tag).ok_or_else(|| OcelError::new(format!("missing <{tag}> element")))
}

fn optional_child<'a, 'input>(parent: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    parent
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == tag)
}

fn element_children_named<'a, 'input>(
    parent: Node<'a, 'input>,
    tag: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    parent
        .children()
        .filter(move |child| child.is_element() && child.tag_name().name() == tag)
}

fn required_attr<'a>(node: Node<'a, '_>, name: &str) -> OcelResult<&'a str> {
    node.attribute(name).ok_or_else(|| {
        OcelError::new(format!(
            "missing required XML attribute '{name}' on <{}>",
            node.tag_name().name()
        ))
    })
}

fn compact_type_def(
    source_type: &SourceType,
    pool: &mut StringPool,
    attr_types: &mut HashMap<(String, String), AttrType>,
    type_label: &str,
) -> OcelResult<TypeDef> {
    let mut seen_attributes = HashSet::new();
    let mut attributes = Vec::with_capacity(source_type.attributes.len());

    for attribute in &source_type.attributes {
        if !seen_attributes.insert(attribute.name.clone()) {
            return Err(OcelError::new(format!(
                "duplicate attribute '{}' on {type_label} '{}'",
                attribute.name, source_type.name
            )));
        }

        let attr_type = AttrType::parse(&attribute.attr_type)?;
        attr_types.insert(
            (source_type.name.clone(), attribute.name.clone()),
            attr_type,
        );
        attributes.push(AttributeDef {
            name: pool.intern(&attribute.name),
            attr_type,
        });
    }

    Ok(TypeDef {
        name: pool.intern(&source_type.name),
        attributes,
    })
}

fn compact_attributes(
    source_attributes: &[SourceAttribute],
    type_name: &str,
    attr_types: &HashMap<(String, String), AttrType>,
    pool: &mut StringPool,
) -> OcelResult<Vec<Attribute>> {
    source_attributes
        .iter()
        .map(|source_attribute| {
            let attr_type = attr_types
                .get(&(type_name.to_owned(), source_attribute.name.clone()))
                .copied();
            Ok(Attribute {
                name: pool.intern(&source_attribute.name),
                value: compact_value(&source_attribute.value, attr_type, pool)?,
            })
        })
        .collect()
}

fn compact_timed_attributes(
    source_attributes: &[SourceTimedAttribute],
    type_name: &str,
    attr_types: &HashMap<(String, String), AttrType>,
    pool: &mut StringPool,
) -> OcelResult<Vec<TimedAttribute>> {
    source_attributes
        .iter()
        .map(|source_attribute| {
            let attr_type = attr_types
                .get(&(type_name.to_owned(), source_attribute.name.clone()))
                .copied();
            Ok(TimedAttribute {
                name: pool.intern(&source_attribute.name),
                time_ms: parse_timestamp_ms(&source_attribute.time)?,
                value: compact_value(&source_attribute.value, attr_type, pool)?,
            })
        })
        .collect()
}

fn compact_relationships(
    relationships: &[SourceRelationship],
    pool: &mut StringPool,
) -> Vec<Relationship> {
    relationships
        .iter()
        .map(|relationship| Relationship {
            object_id: pool.intern(&relationship.object_id),
            qualifier: pool.intern(&relationship.qualifier),
        })
        .collect()
}

fn compact_value(
    source_value: &SourceValue,
    attr_type: Option<AttrType>,
    pool: &mut StringPool,
) -> OcelResult<AttrValue> {
    let attr_type = attr_type.unwrap_or_else(|| infer_attr_type(source_value));
    match attr_type {
        AttrType::String => Ok(AttrValue::String(
            pool.intern(&source_value_as_string(source_value)),
        )),
        AttrType::Time => parse_time_value(source_value).map(AttrValue::Time),
        AttrType::Integer => parse_integer_value(source_value).map(AttrValue::Integer),
        AttrType::Float => parse_float_value(source_value).map(AttrValue::Float),
        AttrType::Boolean => parse_boolean_value(source_value).map(AttrValue::Boolean),
    }
}

fn infer_attr_type(source_value: &SourceValue) -> AttrType {
    match source_value {
        SourceValue::String(_) => AttrType::String,
        SourceValue::Integer(_) => AttrType::Integer,
        SourceValue::Float(_) => AttrType::Float,
        SourceValue::Boolean(_) => AttrType::Boolean,
    }
}

fn source_value_as_string(source_value: &SourceValue) -> String {
    match source_value {
        SourceValue::String(value) => value.clone(),
        SourceValue::Integer(value) => value.to_string(),
        SourceValue::Float(value) => value.to_string(),
        SourceValue::Boolean(value) => value.to_string(),
    }
}

fn parse_time_value(source_value: &SourceValue) -> OcelResult<i64> {
    match source_value {
        SourceValue::String(value) => parse_timestamp_ms(value),
        _ => Err(OcelError::new("time attributes must be ISO 8601 strings")),
    }
}

fn parse_integer_value(source_value: &SourceValue) -> OcelResult<i64> {
    match source_value {
        SourceValue::Integer(value) => Ok(*value),
        SourceValue::Float(value) if value.fract() == 0.0 => Ok(*value as i64),
        SourceValue::String(value) => value
            .trim()
            .parse::<i64>()
            .map_err(|err| OcelError::new(format!("invalid integer attribute '{value}': {err}"))),
        SourceValue::Float(_) | SourceValue::Boolean(_) => {
            Err(OcelError::new("integer attributes must be integer values"))
        }
    }
}

fn parse_float_value(source_value: &SourceValue) -> OcelResult<f64> {
    let value = match source_value {
        SourceValue::Integer(value) => *value as f64,
        SourceValue::Float(value) => *value,
        SourceValue::String(value) => value
            .trim()
            .parse::<f64>()
            .map_err(|err| OcelError::new(format!("invalid float attribute '{value}': {err}")))?,
        SourceValue::Boolean(_) => return Err(OcelError::new("float attributes must be numeric")),
    };

    if value.is_finite() {
        Ok(value)
    } else {
        Err(OcelError::new("float attributes must be finite"))
    }
}

fn parse_boolean_value(source_value: &SourceValue) -> OcelResult<bool> {
    match source_value {
        SourceValue::Boolean(value) => Ok(*value),
        SourceValue::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(OcelError::new(format!(
                "invalid boolean attribute '{value}'"
            ))),
        },
        SourceValue::Integer(1) => Ok(true),
        SourceValue::Integer(0) => Ok(false),
        SourceValue::Integer(_) | SourceValue::Float(_) => Err(OcelError::new(
            "boolean attributes must be true/false or 1/0",
        )),
    }
}

fn decode_ocel_bytes(input: &[u8]) -> OcelResult<String> {
    let bytes = if input.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = GzDecoder::new(input);
        let mut decoded = Vec::new();
        decoder
            .read_to_end(&mut decoded)
            .map_err(|err| OcelError::new(format!("could not decompress gzip OCEL file: {err}")))?;
        decoded
    } else {
        input.to_vec()
    };

    String::from_utf8(bytes)
        .map_err(|err| OcelError::new(format!("OCEL input is not valid UTF-8: {err}")))
}

fn detect_format(input: &str, hint: Option<&str>) -> OcelResult<OcelFormat> {
    if let Some(hint) = hint {
        let hint = hint.to_ascii_lowercase();
        let hint = hint.strip_suffix(".gz").unwrap_or(&hint);
        if hint.ends_with(".json") || hint.ends_with(".jsonocel") || hint == "json" {
            return Ok(OcelFormat::Json);
        }
        if hint.ends_with(".xml") || hint.ends_with(".xmlocel") || hint == "xml" {
            return Ok(OcelFormat::Xml);
        }
    }

    let first = input
        .trim_start()
        .chars()
        .next()
        .ok_or_else(|| OcelError::new("cannot import an empty OCEL file"))?;
    match first {
        '{' => Ok(OcelFormat::Json),
        '<' => Ok(OcelFormat::Xml),
        _ => Err(OcelError::new(
            "could not detect OCEL format; expected JSON or XML input",
        )),
    }
}

fn parse_timestamp_ms(input: &str) -> OcelResult<i64> {
    let value = input.trim();
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(value) {
        return Ok(timestamp.timestamp_millis());
    }

    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(timestamp) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(
                DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc).timestamp_millis()
            );
        }
    }

    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let timestamp = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| OcelError::new(format!("invalid date '{value}'")))?;
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc).timestamp_millis());
    }

    Err(OcelError::new(format!(
        "invalid ISO 8601 timestamp '{input}'"
    )))
}

fn format_timestamp_ms(timestamp_ms: i64) -> OcelResult<String> {
    let timestamp = DateTime::<Utc>::from_timestamp_millis(timestamp_ms).ok_or_else(|| {
        OcelError::new(format!("timestamp {timestamp_ms} is outside chrono range"))
    })?;
    let precision = if timestamp_ms % 1000 == 0 {
        SecondsFormat::Secs
    } else {
        SecondsFormat::Millis
    };
    Ok(timestamp.to_rfc3339_opts(precision, true))
}

fn escape_xml_attr(value: &str) -> String {
    escape_xml(value, true)
}

fn escape_xml_text(value: &str) -> String {
    escape_xml(value, false)
}

fn escape_xml(value: &str, attribute: bool) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' if attribute => escaped.push_str("&quot;"),
            '\'' if attribute => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}
