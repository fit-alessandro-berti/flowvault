//! OCEL 2.0 import/export core.
//!
//! The public WebAssembly API accepts standard OCEL 2.0 JSON and XML files and
//! returns standard JSON/XML exports. Internally, the log is stored in a compact
//! representation: repeated strings are interned once, object/event references
//! are symbols, timestamps are Unix timestamps in milliseconds, and every object
//! keeps a timestamp-ordered lifecycle of related events.

use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Write};
use wasm_bindgen::prelude::*;

type OcelResult<T> = Result<T, OcelError>;

#[derive(Debug, Clone)]
struct OcelError {
    message: String,
}

impl OcelError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for OcelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for OcelError {}

impl From<OcelError> for JsValue {
    fn from(value: OcelError) -> Self {
        JsValue::from_str(&value.message)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OcelFormat {
    Json,
    Xml,
}

impl OcelFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct Symbol(u32);

#[derive(Debug, Default)]
struct StringPool {
    values: Vec<String>,
    index: HashMap<String, Symbol>,
}

impl StringPool {
    fn intern(&mut self, value: &str) -> Symbol {
        if let Some(symbol) = self.index.get(value) {
            return *symbol;
        }

        let symbol = Symbol(self.values.len() as u32);
        let owned = value.to_owned();
        self.values.push(owned.clone());
        self.index.insert(owned, symbol);
        symbol
    }

    fn resolve(&self, symbol: Symbol) -> &str {
        &self.values[symbol.0 as usize]
    }

    fn finish(mut self) -> Self {
        self.index.clear();
        self.index.shrink_to_fit();
        self
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum AttrType {
    String,
    Time,
    Integer,
    Float,
    Boolean,
}

impl AttrType {
    fn parse(value: &str) -> OcelResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "string" => Ok(Self::String),
            "time" => Ok(Self::Time),
            "integer" => Ok(Self::Integer),
            "float" => Ok(Self::Float),
            "boolean" => Ok(Self::Boolean),
            other => Err(OcelError::new(format!(
                "unsupported OCEL attribute type '{other}'"
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Time => "time",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
        }
    }
}

#[derive(Debug)]
struct AttributeDef {
    name: Symbol,
    attr_type: AttrType,
}

#[derive(Debug)]
struct TypeDef {
    name: Symbol,
    attributes: Vec<AttributeDef>,
}

#[derive(Debug)]
enum AttrValue {
    String(Symbol),
    Time(i64),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug)]
struct Attribute {
    name: Symbol,
    value: AttrValue,
}

#[derive(Debug)]
struct TimedAttribute {
    name: Symbol,
    time_ms: i64,
    value: AttrValue,
}

#[derive(Debug)]
struct Relationship {
    object_id: Symbol,
    qualifier: Symbol,
}

#[derive(Debug)]
struct Event {
    id: Symbol,
    type_name: Symbol,
    time_ms: i64,
    attributes: Vec<Attribute>,
    relationships: Vec<Relationship>,
}

#[derive(Debug)]
struct Object {
    id: Symbol,
    type_name: Symbol,
    attributes: Vec<TimedAttribute>,
    relationships: Vec<Relationship>,
    lifecycle: Vec<usize>,
}

#[derive(Debug)]
struct CompactOcelLog {
    format: OcelFormat,
    pool: StringPool,
    event_types: Vec<TypeDef>,
    object_types: Vec<TypeDef>,
    events: Vec<Event>,
    objects: Vec<Object>,
    object_index: HashMap<Symbol, usize>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct OcelSummary {
    source_format: &'static str,
    event_types: usize,
    object_types: usize,
    events: usize,
    objects: usize,
    e2o_relationships: usize,
    o2o_relationships: usize,
    interned_strings: usize,
    objects_with_lifecycle: usize,
}

impl CompactOcelLog {
    fn from_input(input: &str, format_hint: Option<&str>) -> OcelResult<Self> {
        let format = detect_format(input, format_hint)?;
        let source = match format {
            OcelFormat::Json => parse_json(input)?,
            OcelFormat::Xml => parse_xml(input)?,
        };
        Self::from_source(source, format)
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
        })
    }

    fn summary(&self) -> OcelSummary {
        OcelSummary {
            source_format: self.format.as_str(),
            event_types: self.event_types.len(),
            object_types: self.object_types.len(),
            events: self.events.len(),
            objects: self.objects.len(),
            e2o_relationships: self
                .events
                .iter()
                .map(|event| event.relationships.len())
                .sum(),
            o2o_relationships: self
                .objects
                .iter()
                .map(|object| object.relationships.len())
                .sum(),
            interned_strings: self.pool.values.len(),
            objects_with_lifecycle: self
                .objects
                .iter()
                .filter(|object| !object.lifecycle.is_empty())
                .count(),
        }
    }

    fn summary_json(&self) -> String {
        serde_json::to_string(&self.summary()).expect("summary serialization cannot fail")
    }

    fn lifecycle_json(&self, object_id: &str) -> OcelResult<String> {
        let lookup = self
            .pool
            .values
            .iter()
            .position(|value| value == object_id)
            .map(|index| Symbol(index as u32))
            .and_then(|symbol| self.object_index.get(&symbol).copied());

        let object_index = lookup.ok_or_else(|| {
            OcelError::new(format!("object id '{object_id}' was not found in the log"))
        })?;

        let event_ids: Vec<&str> = self.objects[object_index]
            .lifecycle
            .iter()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .collect();
        serde_json::to_string(&event_ids)
            .map_err(|err| OcelError::new(format!("could not serialize lifecycle: {err}")))
    }

    fn export_json(&self) -> OcelResult<String> {
        let mut top = Map::new();
        top.insert(
            "eventTypes".to_owned(),
            Value::Array(
                self.event_types
                    .iter()
                    .map(|type_def| self.type_def_to_json(type_def))
                    .collect(),
            ),
        );
        top.insert(
            "objectTypes".to_owned(),
            Value::Array(
                self.object_types
                    .iter()
                    .map(|type_def| self.type_def_to_json(type_def))
                    .collect(),
            ),
        );
        top.insert(
            "events".to_owned(),
            Value::Array(
                self.events
                    .iter()
                    .map(|event| self.event_to_json(event))
                    .collect::<OcelResult<Vec<_>>>()?,
            ),
        );
        top.insert(
            "objects".to_owned(),
            Value::Array(
                self.objects
                    .iter()
                    .map(|object| self.object_to_json(object))
                    .collect::<OcelResult<Vec<_>>>()?,
            ),
        );

        serde_json::to_string_pretty(&Value::Object(top))
            .map_err(|err| OcelError::new(format!("could not export JSON: {err}")))
    }

    fn type_def_to_json(&self, type_def: &TypeDef) -> Value {
        json!({
            "name": self.pool.resolve(type_def.name),
            "attributes": type_def.attributes.iter().map(|attribute| {
                json!({
                    "name": self.pool.resolve(attribute.name),
                    "type": attribute.attr_type.as_str(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    fn event_to_json(&self, event: &Event) -> OcelResult<Value> {
        Ok(json!({
            "id": self.pool.resolve(event.id),
            "type": self.pool.resolve(event.type_name),
            "time": format_timestamp_ms(event.time_ms)?,
            "attributes": event.attributes.iter().map(|attribute| {
                self.attribute_to_json(attribute)
            }).collect::<OcelResult<Vec<_>>>()?,
            "relationships": self.relationships_to_json(&event.relationships),
        }))
    }

    fn object_to_json(&self, object: &Object) -> OcelResult<Value> {
        Ok(json!({
            "id": self.pool.resolve(object.id),
            "type": self.pool.resolve(object.type_name),
            "attributes": object.attributes.iter().map(|attribute| {
                self.timed_attribute_to_json(attribute)
            }).collect::<OcelResult<Vec<_>>>()?,
            "relationships": self.relationships_to_json(&object.relationships),
        }))
    }

    fn attribute_to_json(&self, attribute: &Attribute) -> OcelResult<Value> {
        Ok(json!({
            "name": self.pool.resolve(attribute.name),
            "value": self.attr_value_to_json(&attribute.value)?,
        }))
    }

    fn timed_attribute_to_json(&self, attribute: &TimedAttribute) -> OcelResult<Value> {
        Ok(json!({
            "name": self.pool.resolve(attribute.name),
            "time": format_timestamp_ms(attribute.time_ms)?,
            "value": self.attr_value_to_json(&attribute.value)?,
        }))
    }

    fn relationships_to_json(&self, relationships: &[Relationship]) -> Value {
        Value::Array(
            relationships
                .iter()
                .map(|relationship| {
                    json!({
                        "objectId": self.pool.resolve(relationship.object_id),
                        "qualifier": self.pool.resolve(relationship.qualifier),
                    })
                })
                .collect(),
        )
    }

    fn attr_value_to_json(&self, value: &AttrValue) -> OcelResult<Value> {
        match value {
            AttrValue::String(symbol) => Ok(Value::String(self.pool.resolve(*symbol).to_owned())),
            AttrValue::Time(ms) => Ok(Value::String(format_timestamp_ms(*ms)?)),
            AttrValue::Integer(number) => Ok(Value::Number(Number::from(*number))),
            AttrValue::Float(number) => Number::from_f64(*number)
                .map(Value::Number)
                .ok_or_else(|| OcelError::new("cannot export non-finite float value")),
            AttrValue::Boolean(value) => Ok(Value::Bool(*value)),
        }
    }

    fn export_xml(&self) -> OcelResult<String> {
        let mut output = String::new();
        output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<log>\n");

        output.push_str("  <event-types>\n");
        for event_type in &self.event_types {
            self.write_type_xml(&mut output, "event-type", event_type, 2)?;
        }
        output.push_str("  </event-types>\n");

        output.push_str("  <object-types>\n");
        for object_type in &self.object_types {
            self.write_type_xml(&mut output, "object-type", object_type, 2)?;
        }
        output.push_str("  </object-types>\n");

        output.push_str("  <events>\n");
        for event in &self.events {
            writeln!(
                output,
                "    <event id=\"{}\" type=\"{}\" time=\"{}\">",
                escape_xml_attr(self.pool.resolve(event.id)),
                escape_xml_attr(self.pool.resolve(event.type_name)),
                format_timestamp_ms(event.time_ms)?
            )
            .expect("writing to String cannot fail");
            self.write_attributes_xml(&mut output, &event.attributes, 6)?;
            self.write_relationships_xml(&mut output, &event.relationships, 6);
            output.push_str("    </event>\n");
        }
        output.push_str("  </events>\n");

        output.push_str("  <objects>\n");
        for object in &self.objects {
            writeln!(
                output,
                "    <object id=\"{}\" type=\"{}\">",
                escape_xml_attr(self.pool.resolve(object.id)),
                escape_xml_attr(self.pool.resolve(object.type_name))
            )
            .expect("writing to String cannot fail");
            self.write_timed_attributes_xml(&mut output, &object.attributes, 6)?;
            self.write_relationships_xml(&mut output, &object.relationships, 6);
            output.push_str("    </object>\n");
        }
        output.push_str("  </objects>\n</log>\n");
        Ok(output)
    }

    fn write_type_xml(
        &self,
        output: &mut String,
        tag: &str,
        type_def: &TypeDef,
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        writeln!(
            output,
            "{pad}<{tag} name=\"{}\">",
            escape_xml_attr(self.pool.resolve(type_def.name))
        )
        .expect("writing to String cannot fail");
        if type_def.attributes.is_empty() {
            writeln!(output, "{pad}  <attributes/>").expect("writing to String cannot fail");
        } else {
            writeln!(output, "{pad}  <attributes>").expect("writing to String cannot fail");
            for attribute in &type_def.attributes {
                writeln!(
                    output,
                    "{pad}    <attribute name=\"{}\" type=\"{}\"/>",
                    escape_xml_attr(self.pool.resolve(attribute.name)),
                    attribute.attr_type.as_str()
                )
                .expect("writing to String cannot fail");
            }
            writeln!(output, "{pad}  </attributes>").expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</{tag}>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_attributes_xml(
        &self,
        output: &mut String,
        attributes: &[Attribute],
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        if attributes.is_empty() {
            writeln!(output, "{pad}<attributes/>").expect("writing to String cannot fail");
            return Ok(());
        }

        writeln!(output, "{pad}<attributes>").expect("writing to String cannot fail");
        for attribute in attributes {
            writeln!(
                output,
                "{pad}  <attribute name=\"{}\">{}</attribute>",
                escape_xml_attr(self.pool.resolve(attribute.name)),
                escape_xml_text(&self.attr_value_to_xml_text(&attribute.value)?)
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</attributes>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_timed_attributes_xml(
        &self,
        output: &mut String,
        attributes: &[TimedAttribute],
        indent: usize,
    ) -> OcelResult<()> {
        let pad = " ".repeat(indent);
        if attributes.is_empty() {
            writeln!(output, "{pad}<attributes/>").expect("writing to String cannot fail");
            return Ok(());
        }

        writeln!(output, "{pad}<attributes>").expect("writing to String cannot fail");
        for attribute in attributes {
            writeln!(
                output,
                "{pad}  <attribute name=\"{}\" time=\"{}\">{}</attribute>",
                escape_xml_attr(self.pool.resolve(attribute.name)),
                format_timestamp_ms(attribute.time_ms)?,
                escape_xml_text(&self.attr_value_to_xml_text(&attribute.value)?)
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</attributes>").expect("writing to String cannot fail");
        Ok(())
    }

    fn write_relationships_xml(
        &self,
        output: &mut String,
        relationships: &[Relationship],
        indent: usize,
    ) {
        if relationships.is_empty() {
            return;
        }

        let pad = " ".repeat(indent);
        writeln!(output, "{pad}<objects>").expect("writing to String cannot fail");
        for relationship in relationships {
            writeln!(
                output,
                "{pad}  <relationship object-id=\"{}\" qualifier=\"{}\"/>",
                escape_xml_attr(self.pool.resolve(relationship.object_id)),
                escape_xml_attr(self.pool.resolve(relationship.qualifier))
            )
            .expect("writing to String cannot fail");
        }
        writeln!(output, "{pad}</objects>").expect("writing to String cannot fail");
    }

    fn attr_value_to_xml_text(&self, value: &AttrValue) -> OcelResult<String> {
        match value {
            AttrValue::String(symbol) => Ok(self.pool.resolve(*symbol).to_owned()),
            AttrValue::Time(ms) => format_timestamp_ms(*ms),
            AttrValue::Integer(number) => Ok(number.to_string()),
            AttrValue::Float(number) => {
                if !number.is_finite() {
                    return Err(OcelError::new("cannot export non-finite float value"));
                }
                Ok(number.to_string())
            }
            AttrValue::Boolean(value) => Ok(if *value { "1" } else { "0" }.to_owned()),
        }
    }
}

/// Parsed OCEL document exposed to JavaScript.
///
/// Constructing this type imports and validates the OCEL text once. Subsequent
/// summary/export calls reuse the compact in-memory representation.
#[wasm_bindgen]
pub struct OcelDocument {
    log: CompactOcelLog,
}

#[wasm_bindgen]
impl OcelDocument {
    /// Imports an OCEL 2.0 JSON or XML string.
    ///
    /// `format_hint` may be `"json"`, `"xml"`, a filename, or `undefined`.
    /// When omitted, the parser detects JSON/XML from the first non-whitespace
    /// character.
    #[wasm_bindgen(constructor)]
    pub fn new(input: &str, format_hint: Option<String>) -> Result<OcelDocument, JsValue> {
        let log = CompactOcelLog::from_input(input, format_hint.as_deref())?;
        Ok(Self { log })
    }

    /// Returns summary counts as a JSON string.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> String {
        self.log.summary_json()
    }

    /// Exports the document as OCEL 2.0 JSON.
    #[wasm_bindgen(js_name = exportJson)]
    pub fn export_json(&self) -> Result<String, JsValue> {
        self.log.export_json().map_err(JsValue::from)
    }

    /// Exports the document as OCEL 2.0 XML.
    #[wasm_bindgen(js_name = exportXml)]
    pub fn export_xml(&self) -> Result<String, JsValue> {
        self.log.export_xml().map_err(JsValue::from)
    }

    /// Returns the ordered event IDs related to an object ID as a JSON array.
    #[wasm_bindgen(js_name = objectLifecycleJson)]
    pub fn object_lifecycle_json(&self, object_id: &str) -> Result<String, JsValue> {
        self.log.lifecycle_json(object_id).map_err(JsValue::from)
    }
}

#[derive(Debug)]
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

fn detect_format(input: &str, hint: Option<&str>) -> OcelResult<OcelFormat> {
    if let Some(hint) = hint {
        let hint = hint.to_ascii_lowercase();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    const JSON_EXAMPLE: &str = include_str!("../../../files/ocel2/ocel20_example.json");
    const XML_EXAMPLE: &str = include_str!("../../../files/ocel2/ocel20_example.xml");

    #[test]
    fn imports_json_example_and_counts_relationships() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let summary = log.summary();

        assert_eq!(summary.event_types, 8);
        assert_eq!(summary.object_types, 4);
        assert_eq!(summary.events, 13);
        assert_eq!(summary.objects, 9);
        assert_eq!(summary.e2o_relationships, 20);
        assert_eq!(summary.o2o_relationships, 7);
        assert!(summary.interned_strings < 120);
        assert_eq!(summary.objects_with_lifecycle, 9);
    }

    #[test]
    fn imports_xml_example_and_counts_relationships() {
        let log = CompactOcelLog::from_input(XML_EXAMPLE, Some("xml")).unwrap();
        let summary = log.summary();

        assert_eq!(summary.event_types, 8);
        assert_eq!(summary.object_types, 4);
        assert_eq!(summary.events, 13);
        assert_eq!(summary.objects, 9);
        assert_eq!(summary.e2o_relationships, 20);
        assert_eq!(summary.o2o_relationships, 7);
    }

    #[test]
    fn keeps_ordered_object_lifecycles() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();

        assert_eq!(log.lifecycle_json("PR1").unwrap(), r#"["e1","e2","e3"]"#);
        assert_eq!(
            log.lifecycle_json("PO1").unwrap(),
            r#"["e3","e4","e5","e6"]"#
        );
        assert_eq!(log.lifecycle_json("P3").unwrap(), r#"["e13"]"#);
    }

    #[test]
    fn json_export_round_trips() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let exported = log.export_json().unwrap();
        let imported = CompactOcelLog::from_input(&exported, Some("json")).unwrap();

        assert_eq!(imported.summary().events, 13);
        assert_eq!(imported.summary().objects, 9);
        assert_eq!(imported.summary().e2o_relationships, 20);
        assert_eq!(
            imported.lifecycle_json("PO1").unwrap(),
            r#"["e3","e4","e5","e6"]"#
        );
    }

    #[test]
    fn xml_export_round_trips() {
        let log = CompactOcelLog::from_input(XML_EXAMPLE, Some("xml")).unwrap();
        let exported = log.export_xml().unwrap();
        let imported = CompactOcelLog::from_input(&exported, Some("xml")).unwrap();

        assert_eq!(imported.summary().events, 13);
        assert_eq!(imported.summary().objects, 9);
        assert_eq!(imported.summary().o2o_relationships, 7);
    }

    #[test]
    fn converts_iso_timestamps_to_unix_milliseconds() {
        assert_eq!(parse_timestamp_ms("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(
            parse_timestamp_ms("2022-01-09T14:00:00+00:00").unwrap(),
            1_641_736_800_000
        );
        assert_eq!(
            format_timestamp_ms(1_641_736_800_000).unwrap(),
            "2022-01-09T14:00:00Z"
        );
    }

    #[test]
    fn rejects_unknown_relationship_targets() {
        let input = r#"{
          "eventTypes": [{"name": "a", "attributes": []}],
          "objectTypes": [{"name": "o", "attributes": []}],
          "events": [{
            "id": "e1",
            "type": "a",
            "time": "1970-01-01T00:00:00Z",
            "relationships": [{"objectId": "missing", "qualifier": "x"}]
          }],
          "objects": [{"id": "o1", "type": "o"}]
        }"#;

        let error = CompactOcelLog::from_input(input, Some("json")).unwrap_err();
        assert!(error.to_string().contains("unknown object 'missing'"));
    }

    #[test]
    fn imports_and_exports_all_ocel_fixtures() {
        for fixture_path in ocel_fixture_paths() {
            let fixture_name = fixture_path.display().to_string();
            let input = fs::read_to_string(&fixture_path)
                .unwrap_or_else(|err| panic!("failed to read {fixture_name}: {err}"));
            let format_hint = fixture_path
                .extension()
                .and_then(|extension| extension.to_str())
                .expect("fixture path should have an extension");
            let log = CompactOcelLog::from_input(&input, Some(format_hint))
                .unwrap_or_else(|err| panic!("failed to import {fixture_name}: {err}"));
            let summary = log.summary();

            assert!(
                summary.event_types > 0,
                "{fixture_name} should declare event types"
            );
            assert!(
                summary.object_types > 0,
                "{fixture_name} should declare object types"
            );
            assert!(summary.events > 0, "{fixture_name} should contain events");
            assert!(summary.objects > 0, "{fixture_name} should contain objects");
            assert!(
                summary.e2o_relationships >= summary.objects_with_lifecycle,
                "{fixture_name} should not have more lifecycle indexes than E2O relationships"
            );

            let exported_json = log
                .export_json()
                .unwrap_or_else(|err| panic!("failed to export {fixture_name} as JSON: {err}"));
            let reparsed_json = CompactOcelLog::from_input(&exported_json, Some("json"))
                .unwrap_or_else(|err| {
                    panic!("failed to reimport JSON export of {fixture_name}: {err}")
                });
            assert_same_structural_summary(
                &reparsed_json.summary(),
                &summary,
                &format!("JSON export changed summary counts for {fixture_name}"),
            );

            let exported_xml = log
                .export_xml()
                .unwrap_or_else(|err| panic!("failed to export {fixture_name} as XML: {err}"));
            let reparsed_xml = CompactOcelLog::from_input(&exported_xml, Some("xml"))
                .unwrap_or_else(|err| {
                    panic!("failed to reimport XML export of {fixture_name}: {err}")
                });
            assert_same_structural_summary(
                &reparsed_xml.summary(),
                &summary,
                &format!("XML export changed summary counts for {fixture_name}"),
            );
        }
    }

    fn assert_same_structural_summary(actual: &OcelSummary, expected: &OcelSummary, context: &str) {
        assert_eq!(actual.event_types, expected.event_types, "{context}");
        assert_eq!(actual.object_types, expected.object_types, "{context}");
        assert_eq!(actual.events, expected.events, "{context}");
        assert_eq!(actual.objects, expected.objects, "{context}");
        assert_eq!(
            actual.e2o_relationships, expected.e2o_relationships,
            "{context}"
        );
        assert_eq!(
            actual.o2o_relationships, expected.o2o_relationships,
            "{context}"
        );
        assert_eq!(
            actual.objects_with_lifecycle, expected.objects_with_lifecycle,
            "{context}"
        );
    }

    fn ocel_fixture_paths() -> Vec<PathBuf> {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../files/ocel2");
        let mut paths = fs::read_dir(&fixture_dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_dir.display()))
            .map(|entry| {
                entry
                    .expect("failed to read fixture directory entry")
                    .path()
            })
            .filter(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| matches!(extension, "json" | "xml" | "jsonocel" | "xmlocel"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }
}
