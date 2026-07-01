
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
