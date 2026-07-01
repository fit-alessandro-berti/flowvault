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
