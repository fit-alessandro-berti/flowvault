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
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
        if self.index.is_empty() && !self.values.is_empty() {
            self.rebuild_index();
        }

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

    fn rebuild_index(&mut self) {
        self.index = self
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| (value.clone(), Symbol(index as u32)))
            .collect();
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

#[derive(Debug, Clone)]
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
    stateful_events: usize,
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
            stateful_events: self.count_events_with_attribute("state"),
        }
    }

    fn count_events_with_attribute(&self, attribute_name: &str) -> usize {
        self.events
            .iter()
            .filter(|event| {
                event
                    .attributes
                    .iter()
                    .any(|attribute| self.pool.resolve(attribute.name) == attribute_name)
            })
            .count()
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

    fn apply_state_query(&mut self, query: &str) -> OcelResult<String> {
        let state_query = StateQuery::parse(query)?;
        let eval_index = StateEvalIndex::build(self, &state_query);
        let attribute_symbol = self.pool.intern(&state_query.attribute_name);
        self.ensure_event_attribute(attribute_symbol, AttrType::String);

        let mut assigned = 0usize;
        for event_index in 0..self.events.len() {
            if let Some(state) = self.evaluate_state_query(&state_query, &eval_index, event_index) {
                let state_symbol = self.pool.intern(&state);
                let event = &mut self.events[event_index];
                event
                    .attributes
                    .retain(|attribute| attribute.name != attribute_symbol);
                event.attributes.push(Attribute {
                    name: attribute_symbol,
                    value: AttrValue::String(state_symbol),
                });
                assigned += 1;
            }
        }

        let result = StateQueryResult {
            attribute: state_query.attribute_name,
            assigned_events: assigned,
            total_events: self.events.len(),
        };
        serde_json::to_string(&result)
            .map_err(|err| OcelError::new(format!("could not serialize state query result: {err}")))
    }

    fn state_patterns_json(&self) -> OcelResult<String> {
        let analysis = self.detect_state_patterns()?;
        serde_json::to_string(&analysis).map_err(|err| {
            OcelError::new(format!("could not serialize state pattern analysis: {err}"))
        })
    }

    fn detect_state_patterns(&self) -> OcelResult<PatternAnalysis> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;

        if !self
            .events
            .iter()
            .any(|event| self.event_state(event, state_attribute).is_some())
        {
            return Err(OcelError::new(
                "event state attribute is empty; apply a state query first",
            ));
        }

        let mut intra = HashMap::<PatternKey, PatternAccumulator>::new();
        let mut inter = HashMap::<PatternKey, PatternAccumulator>::new();

        for (object_index, object) in self.objects.iter().enumerate() {
            let state_lifecycle = object
                .lifecycle
                .iter()
                .filter_map(|event_index| {
                    self.event_state(&self.events[*event_index], state_attribute)
                        .map(|state| (*event_index, state.to_owned()))
                })
                .collect::<Vec<_>>();

            if state_lifecycle.is_empty() {
                continue;
            }

            let episodes = state_episodes(&state_lifecycle);
            for episode in &episodes {
                let instance = self.pattern_instance(
                    PatternFamily::Intra,
                    object_index,
                    episode.state.clone(),
                    None,
                    self.intra_sequence(&state_lifecycle, episode),
                    &state_lifecycle[episode.start..=episode.end],
                );
                insert_pattern_instance(&mut intra, instance);
            }

            for episode_pair in episodes.windows(2) {
                let [left, right] = episode_pair else {
                    continue;
                };
                if left.state == right.state {
                    continue;
                }

                let mut segment_events = Vec::with_capacity(right.end - left.start + 1);
                segment_events.extend_from_slice(&state_lifecycle[left.start..=left.end]);
                segment_events.extend_from_slice(&state_lifecycle[right.start..=right.end]);

                let instance = self.pattern_instance(
                    PatternFamily::Inter,
                    object_index,
                    left.state.clone(),
                    Some(right.state.clone()),
                    self.inter_sequence(&state_lifecycle, left, right),
                    &segment_events,
                );
                insert_pattern_instance(&mut inter, instance);
            }
        }

        Ok(PatternAnalysis {
            intra: summarize_patterns(intra.into_values().collect()),
            inter: summarize_patterns(inter.into_values().collect()),
        })
    }

    fn pattern_instance(
        &self,
        family: PatternFamily,
        leading_object_index: usize,
        state: String,
        to_state: Option<String>,
        sequence: Vec<String>,
        segment_events: &[(usize, String)],
    ) -> PatternInstance {
        let leading_object = &self.objects[leading_object_index];
        let leading_type = self.pool.resolve(leading_object.type_name).to_owned();
        let mut object_types = BTreeSet::from([leading_type.clone()]);
        let mut eo_edges = BTreeMap::<(String, String), usize>::new();
        let mut oo_edges = BTreeMap::<(String, String), usize>::new();

        for (event_index, state) in segment_events {
            let event = &self.events[*event_index];
            let event_label = self.state_aware_event_label(*event_index, state);

            for relationship in &event.relationships {
                if relationship.object_id == leading_object.id {
                    continue;
                }

                let Some(related_object_index) =
                    self.object_index.get(&relationship.object_id).copied()
                else {
                    continue;
                };
                let related_type = self
                    .pool
                    .resolve(self.objects[related_object_index].type_name)
                    .to_owned();

                object_types.insert(related_type.clone());
                *eo_edges
                    .entry((event_label.clone(), related_type.clone()))
                    .or_default() += 1;
                let oo_pair = unordered_pair(&leading_type, &related_type);
                *oo_edges.entry(oo_pair).or_default() += 1;
            }
        }

        let mut df_edges = BTreeMap::<(String, String), usize>::new();
        for pair in sequence.windows(2) {
            let [source, target] = pair else {
                continue;
            };
            *df_edges
                .entry((source.clone(), target.clone()))
                .or_default() += 1;
        }

        PatternInstance {
            family,
            leading_object_type: leading_type,
            state,
            to_state,
            sequence,
            object_types,
            df_edges,
            eo_edges,
            oo_edges,
        }
    }

    fn state_aware_event_label(&self, event_index: usize, state: &str) -> String {
        format!(
            "{} [{}]",
            self.pool.resolve(self.events[event_index].type_name),
            state
        )
    }

    fn intra_sequence(
        &self,
        state_lifecycle: &[(usize, String)],
        episode: &StateEpisode,
    ) -> Vec<String> {
        let mut sequence = Vec::with_capacity(episode.end - episode.start + 3);
        sequence.push(format!("START {}", episode.state));
        sequence.extend(
            state_lifecycle[episode.start..=episode.end]
                .iter()
                .map(|(event_index, state)| self.state_aware_event_label(*event_index, state)),
        );
        sequence.push(format!("END {}", episode.state));
        sequence
    }

    fn inter_sequence(
        &self,
        state_lifecycle: &[(usize, String)],
        left: &StateEpisode,
        right: &StateEpisode,
    ) -> Vec<String> {
        let mut sequence = Vec::with_capacity(right.end - left.start + 4);
        sequence.push(format!("START {}", left.state));
        sequence.extend(
            state_lifecycle[left.start..=left.end]
                .iter()
                .map(|(event_index, state)| self.state_aware_event_label(*event_index, state)),
        );
        sequence.push(format!("CHANGE {} -> {}", left.state, right.state));
        sequence.extend(
            state_lifecycle[right.start..=right.end]
                .iter()
                .map(|(event_index, state)| self.state_aware_event_label(*event_index, state)),
        );
        sequence.push(format!("END {}", right.state));
        sequence
    }

    fn event_state<'a>(&'a self, event: &'a Event, state_attribute: Symbol) -> Option<&'a str> {
        event.attributes.iter().find_map(|attribute| {
            if attribute.name == state_attribute {
                match attribute.value {
                    AttrValue::String(symbol) => Some(self.pool.resolve(symbol)),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    fn symbol_for_value(&self, value: &str) -> Option<Symbol> {
        self.pool
            .values
            .iter()
            .position(|candidate| candidate == value)
            .map(|index| Symbol(index as u32))
    }

    fn ensure_event_attribute(&mut self, attribute_symbol: Symbol, attr_type: AttrType) {
        for event_type in &mut self.event_types {
            if !event_type
                .attributes
                .iter()
                .any(|attribute| attribute.name == attribute_symbol)
            {
                event_type.attributes.push(AttributeDef {
                    name: attribute_symbol,
                    attr_type,
                });
            }
        }
    }

    fn evaluate_state_query(
        &self,
        query: &StateQuery,
        eval_index: &StateEvalIndex,
        event_index: usize,
    ) -> Option<String> {
        let event = &self.events[event_index];
        let related_objects = event
            .relationships
            .iter()
            .filter_map(|relationship| self.object_index.get(&relationship.object_id).copied())
            .collect::<Vec<_>>();

        for branch in &query.branches {
            if branch.condition.references_object() {
                for object_index in &related_objects {
                    let context = EvalContext {
                        log: self,
                        eval_index,
                        event_index,
                        object_index: Some(*object_index),
                    };
                    if context.eval_condition(&branch.condition) {
                        return context.eval_state_value(&branch.value);
                    }
                }
            } else {
                let context = EvalContext {
                    log: self,
                    eval_index,
                    event_index,
                    object_index: None,
                };
                if context.eval_condition(&branch.condition) {
                    return context.eval_state_value(&branch.value);
                }
            }
        }

        query.else_value.as_ref().and_then(|value| {
            if value.references_object() {
                related_objects.first().and_then(|object_index| {
                    EvalContext {
                        log: self,
                        eval_index,
                        event_index,
                        object_index: Some(*object_index),
                    }
                    .eval_state_value(value)
                })
            } else {
                EvalContext {
                    log: self,
                    eval_index,
                    event_index,
                    object_index: None,
                }
                .eval_state_value(value)
            }
        })
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

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct PatternAnalysis {
    intra: Vec<PatternSummary>,
    inter: Vec<PatternSummary>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct PatternSummary {
    id: String,
    family: &'static str,
    label: String,
    leading_object_type: String,
    state: Option<String>,
    from_state: Option<String>,
    to_state: Option<String>,
    support: usize,
    mass: usize,
    sequence: Vec<String>,
    object_types: Vec<String>,
    df_edges: Vec<PatternEdge>,
    eo_edges: Vec<PatternEdge>,
    oo_edges: Vec<PatternEdge>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct PatternEdge {
    source: String,
    target: String,
    weight: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum PatternFamily {
    Intra,
    Inter,
}

impl PatternFamily {
    fn as_str(self) -> &'static str {
        match self {
            Self::Intra => "intra",
            Self::Inter => "inter",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct PatternKey {
    family: PatternFamily,
    leading_object_type: String,
    state: String,
    to_state: Option<String>,
    sequence: Vec<String>,
    eo_pairs: Vec<(String, String)>,
    oo_pairs: Vec<(String, String)>,
}

#[derive(Debug)]
struct PatternInstance {
    family: PatternFamily,
    leading_object_type: String,
    state: String,
    to_state: Option<String>,
    sequence: Vec<String>,
    object_types: BTreeSet<String>,
    df_edges: BTreeMap<(String, String), usize>,
    eo_edges: BTreeMap<(String, String), usize>,
    oo_edges: BTreeMap<(String, String), usize>,
}

impl PatternInstance {
    fn key(&self) -> PatternKey {
        PatternKey {
            family: self.family,
            leading_object_type: self.leading_object_type.clone(),
            state: self.state.clone(),
            to_state: self.to_state.clone(),
            sequence: self.sequence.clone(),
            eo_pairs: self.eo_edges.keys().cloned().collect(),
            oo_pairs: self.oo_edges.keys().cloned().collect(),
        }
    }
}

#[derive(Debug)]
struct PatternAccumulator {
    key: PatternKey,
    support: usize,
    mass: usize,
    object_types: BTreeSet<String>,
    df_edges: BTreeMap<(String, String), usize>,
    eo_edges: BTreeMap<(String, String), usize>,
    oo_edges: BTreeMap<(String, String), usize>,
}

impl PatternAccumulator {
    fn new(key: PatternKey) -> Self {
        Self {
            key,
            support: 0,
            mass: 0,
            object_types: BTreeSet::new(),
            df_edges: BTreeMap::new(),
            eo_edges: BTreeMap::new(),
            oo_edges: BTreeMap::new(),
        }
    }

    fn add(&mut self, instance: PatternInstance) {
        self.support += 1;
        self.mass += instance.sequence.len().saturating_sub(1);
        self.object_types.extend(instance.object_types);
        merge_weighted_edges(&mut self.df_edges, instance.df_edges);
        merge_weighted_edges(&mut self.eo_edges, instance.eo_edges);
        merge_weighted_edges(&mut self.oo_edges, instance.oo_edges);
    }

    fn into_summary(self, index: usize) -> PatternSummary {
        let family = self.key.family.as_str();
        let label = match &self.key.to_state {
            Some(to_state) => format!(
                "{} -> {} on {}",
                self.key.state, to_state, self.key.leading_object_type
            ),
            None => format!("{} on {}", self.key.state, self.key.leading_object_type),
        };

        PatternSummary {
            id: format!("{family}-{index}"),
            family,
            label,
            leading_object_type: self.key.leading_object_type,
            state: (self.key.family == PatternFamily::Intra).then_some(self.key.state.clone()),
            from_state: (self.key.family == PatternFamily::Inter).then_some(self.key.state),
            to_state: self.key.to_state,
            support: self.support,
            mass: self.mass,
            sequence: self.key.sequence,
            object_types: self.object_types.into_iter().collect(),
            df_edges: edge_map_to_vec(self.df_edges),
            eo_edges: edge_map_to_vec(self.eo_edges),
            oo_edges: edge_map_to_vec(self.oo_edges),
        }
    }
}

#[derive(Debug)]
struct StateEpisode {
    state: String,
    start: usize,
    end: usize,
}

fn state_episodes(state_lifecycle: &[(usize, String)]) -> Vec<StateEpisode> {
    if state_lifecycle.is_empty() {
        return Vec::new();
    }

    let mut episodes = Vec::new();
    let mut start = 0usize;
    let mut current_state = state_lifecycle[0].1.clone();

    for (index, (_, state)) in state_lifecycle.iter().enumerate().skip(1) {
        if *state != current_state {
            episodes.push(StateEpisode {
                state: current_state,
                start,
                end: index - 1,
            });
            start = index;
            current_state = state.clone();
        }
    }

    episodes.push(StateEpisode {
        state: current_state,
        start,
        end: state_lifecycle.len() - 1,
    });
    episodes
}

fn insert_pattern_instance(
    patterns: &mut HashMap<PatternKey, PatternAccumulator>,
    instance: PatternInstance,
) {
    let key = instance.key();
    patterns
        .entry(key.clone())
        .or_insert_with(|| PatternAccumulator::new(key))
        .add(instance);
}

fn summarize_patterns(mut accumulators: Vec<PatternAccumulator>) -> Vec<PatternSummary> {
    accumulators.sort_by(|left, right| {
        right
            .support
            .cmp(&left.support)
            .then_with(|| right.mass.cmp(&left.mass))
            .then_with(|| pattern_sort_label(&left.key).cmp(&pattern_sort_label(&right.key)))
    });

    accumulators
        .into_iter()
        .enumerate()
        .map(|(index, accumulator)| accumulator.into_summary(index + 1))
        .collect()
}

fn pattern_sort_label(key: &PatternKey) -> String {
    match &key.to_state {
        Some(to_state) => format!("{} -> {} {}", key.state, to_state, key.leading_object_type),
        None => format!("{} {}", key.state, key.leading_object_type),
    }
}

fn merge_weighted_edges(
    target: &mut BTreeMap<(String, String), usize>,
    source: BTreeMap<(String, String), usize>,
) {
    for (edge, weight) in source {
        *target.entry(edge).or_default() += weight;
    }
}

fn edge_map_to_vec(edges: BTreeMap<(String, String), usize>) -> Vec<PatternEdge> {
    edges
        .into_iter()
        .map(|((source, target), weight)| PatternEdge {
            source,
            target,
            weight,
        })
        .collect()
}

fn unordered_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_owned(), right.to_owned())
    } else {
        (right.to_owned(), left.to_owned())
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

    /// Applies a SQL-like CASE query and writes a string state attribute to events.
    #[wasm_bindgen(js_name = applyStateQuery)]
    pub fn apply_state_query(&mut self, query: &str) -> Result<String, JsValue> {
        self.log.apply_state_query(query).map_err(JsValue::from)
    }

    /// Detects ranked intra-state and inter-state behavioral patterns.
    #[wasm_bindgen(js_name = statePatternsJson)]
    pub fn state_patterns_json(&self) -> Result<String, JsValue> {
        self.log.state_patterns_json().map_err(JsValue::from)
    }
}

#[derive(Serialize)]
struct StateQueryResult {
    attribute: String,
    assigned_events: usize,
    total_events: usize,
}

#[derive(Debug)]
struct StateQuery {
    attribute_name: String,
    branches: Vec<StateBranch>,
    else_value: Option<ValueExpr>,
}

#[derive(Debug)]
struct StateBranch {
    condition: Expr,
    value: ValueExpr,
}

impl StateQuery {
    fn parse(query: &str) -> OcelResult<Self> {
        let tokens = tokenize(query)?;
        let mut parser = QueryParser {
            tokens,
            position: 0,
        };
        parser.parse_state_query()
    }

    fn referenced_fields(&self) -> ReferencedFields {
        let mut fields = ReferencedFields::default();
        for branch in &self.branches {
            branch.condition.collect_fields(&mut fields);
            branch.value.collect_fields(&mut fields);
        }
        if let Some(value) = &self.else_value {
            value.collect_fields(&mut fields);
        }
        fields
    }
}

#[derive(Default)]
struct ReferencedFields {
    event_attributes: HashSet<String>,
    object_attributes: HashSet<String>,
}

impl ReferencedFields {
    fn add_event_field(&mut self, field: &str) {
        if !matches!(field, "id" | "type" | "activity" | "time" | "timestamp") {
            self.event_attributes.insert(field.to_owned());
        }
    }

    fn add_object_field(&mut self, field: &str) {
        if !matches!(field, "id" | "type") {
            self.object_attributes.insert(field.to_owned());
        }
    }
}

struct StateEvalIndex {
    event_attributes: Vec<HashMap<String, usize>>,
    object_attributes: Vec<HashMap<String, Vec<usize>>>,
}

impl StateEvalIndex {
    fn build(log: &CompactOcelLog, query: &StateQuery) -> Self {
        let fields = query.referenced_fields();
        let event_attributes = log
            .events
            .iter()
            .map(|event| {
                event
                    .attributes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, attribute)| {
                        let name = log.pool.resolve(attribute.name).to_ascii_lowercase();
                        fields
                            .event_attributes
                            .contains(&name)
                            .then_some((name, index))
                    })
                    .collect::<HashMap<_, _>>()
            })
            .collect();
        let object_attributes = log
            .objects
            .iter()
            .map(|object| {
                let mut attributes = HashMap::<String, Vec<usize>>::new();
                for (index, attribute) in object.attributes.iter().enumerate() {
                    let name = log.pool.resolve(attribute.name).to_ascii_lowercase();
                    if fields.object_attributes.contains(&name) {
                        attributes.entry(name).or_default().push(index);
                    }
                }
                for indexes in attributes.values_mut() {
                    indexes.sort_by_key(|index| object.attributes[*index].time_ms);
                }
                attributes
            })
            .collect();

        Self {
            event_attributes,
            object_attributes,
        }
    }
}

#[derive(Debug, Clone)]
enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare {
        left: ValueExpr,
        op: CompareOp,
        right: ValueExpr,
    },
    IsNull {
        value: ValueExpr,
        negated: bool,
    },
}

impl Expr {
    fn references_object(&self) -> bool {
        match self {
            Self::Or(left, right) | Self::And(left, right) => {
                left.references_object() || right.references_object()
            }
            Self::Not(expr) => expr.references_object(),
            Self::Compare { left, right, .. } => {
                left.references_object() || right.references_object()
            }
            Self::IsNull { value, .. } => value.references_object(),
        }
    }

    fn collect_fields(&self, fields: &mut ReferencedFields) {
        match self {
            Self::Or(left, right) | Self::And(left, right) => {
                left.collect_fields(fields);
                right.collect_fields(fields);
            }
            Self::Not(expr) => expr.collect_fields(fields),
            Self::Compare { left, right, .. } => {
                left.collect_fields(fields);
                right.collect_fields(fields);
            }
            Self::IsNull { value, .. } => value.collect_fields(fields),
        }
    }
}

#[derive(Debug, Clone)]
enum ValueExpr {
    Field(FieldRef),
    Literal(QueryValue),
}

impl ValueExpr {
    fn references_object(&self) -> bool {
        matches!(self, Self::Field(FieldRef::Object(_)))
    }

    fn collect_fields(&self, fields: &mut ReferencedFields) {
        match self {
            Self::Field(FieldRef::Event(field)) => fields.add_event_field(field),
            Self::Field(FieldRef::Object(field)) => fields.add_object_field(field),
            Self::Literal(_) => {}
        }
    }
}

#[derive(Debug, Clone)]
enum FieldRef {
    Event(String),
    Object(String),
}

#[derive(Debug, Clone)]
enum QueryValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl QueryValue {
    fn as_state_string(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => {
                if value.fract() == 0.0 {
                    (*value as i64).to_string()
                } else {
                    value.to_string()
                }
            }
            Self::Boolean(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Like,
}

struct EvalContext<'a> {
    log: &'a CompactOcelLog,
    eval_index: &'a StateEvalIndex,
    event_index: usize,
    object_index: Option<usize>,
}

impl EvalContext<'_> {
    fn eval_condition(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Or(left, right) => self.eval_condition(left) || self.eval_condition(right),
            Expr::And(left, right) => self.eval_condition(left) && self.eval_condition(right),
            Expr::Not(expr) => !self.eval_condition(expr),
            Expr::Compare { left, op, right } => {
                let Some(left_value) = self.eval_value(left) else {
                    return false;
                };
                let Some(right_value) = self.eval_value(right) else {
                    return false;
                };
                compare_query_values(&left_value, *op, &right_value)
            }
            Expr::IsNull { value, negated } => {
                let is_null = self.eval_value(value).is_none();
                if *negated {
                    !is_null
                } else {
                    is_null
                }
            }
        }
    }

    fn eval_state_value(&self, value: &ValueExpr) -> Option<String> {
        self.eval_value(value).map(|value| value.as_state_string())
    }

    fn eval_value(&self, value: &ValueExpr) -> Option<QueryValue> {
        match value {
            ValueExpr::Literal(value) => Some(value.clone()),
            ValueExpr::Field(field) => self.eval_field(field),
        }
    }

    fn eval_field(&self, field: &FieldRef) -> Option<QueryValue> {
        match field {
            FieldRef::Event(field_name) => self.eval_event_field(field_name),
            FieldRef::Object(field_name) => self.eval_object_field(field_name),
        }
    }

    fn eval_event_field(&self, field_name: &str) -> Option<QueryValue> {
        let event = &self.log.events[self.event_index];
        match field_name.to_ascii_lowercase().as_str() {
            "id" => Some(QueryValue::String(
                self.log.pool.resolve(event.id).to_owned(),
            )),
            "type" | "activity" => Some(QueryValue::String(
                self.log.pool.resolve(event.type_name).to_owned(),
            )),
            "time" | "timestamp" => Some(QueryValue::Number(event.time_ms as f64)),
            other => self.eval_index.event_attributes[self.event_index]
                .get(other)
                .map(|attribute_index| {
                    self.attr_value_to_query_value(&event.attributes[*attribute_index].value)
                }),
        }
    }

    fn eval_object_field(&self, field_name: &str) -> Option<QueryValue> {
        let object = &self.log.objects[self.object_index?];
        match field_name.to_ascii_lowercase().as_str() {
            "id" => Some(QueryValue::String(
                self.log.pool.resolve(object.id).to_owned(),
            )),
            "type" => Some(QueryValue::String(
                self.log.pool.resolve(object.type_name).to_owned(),
            )),
            other => self.object_attribute_at_event_time(self.object_index?, object, other),
        }
    }

    fn object_attribute_at_event_time(
        &self,
        object_index: usize,
        object: &Object,
        attribute_name: &str,
    ) -> Option<QueryValue> {
        let event_time = self.log.events[self.event_index].time_ms;
        let attribute_indexes =
            self.eval_index.object_attributes[object_index].get(attribute_name)?;
        let partition = attribute_indexes.partition_point(|attribute_index| {
            object.attributes[*attribute_index].time_ms <= event_time
        });
        if partition == 0 {
            return None;
        }
        let attribute = &object.attributes[attribute_indexes[partition - 1]];
        Some(self.attr_value_to_query_value(&attribute.value))
    }

    fn attr_value_to_query_value(&self, value: &AttrValue) -> QueryValue {
        match value {
            AttrValue::String(symbol) => {
                QueryValue::String(self.log.pool.resolve(*symbol).to_owned())
            }
            AttrValue::Time(ms) => QueryValue::Number(*ms as f64),
            AttrValue::Integer(value) => QueryValue::Number(*value as f64),
            AttrValue::Float(value) => QueryValue::Number(*value),
            AttrValue::Boolean(value) => QueryValue::Boolean(*value),
        }
    }
}

fn compare_query_values(left: &QueryValue, op: CompareOp, right: &QueryValue) -> bool {
    match op {
        CompareOp::Eq => query_values_equal(left, right),
        CompareOp::Ne => !query_values_equal(left, right),
        CompareOp::Lt => compare_ordered(left, right).is_some_and(|ordering| ordering < 0),
        CompareOp::Le => compare_ordered(left, right).is_some_and(|ordering| ordering <= 0),
        CompareOp::Gt => compare_ordered(left, right).is_some_and(|ordering| ordering > 0),
        CompareOp::Ge => compare_ordered(left, right).is_some_and(|ordering| ordering >= 0),
        CompareOp::Like => sql_like(
            &left.as_state_string().to_ascii_lowercase(),
            &right.as_state_string().to_ascii_lowercase(),
        ),
    }
}

fn query_values_equal(left: &QueryValue, right: &QueryValue) -> bool {
    match (left, right) {
        (QueryValue::Boolean(left), QueryValue::Boolean(right)) => left == right,
        _ if query_value_as_number(left).is_some() && query_value_as_number(right).is_some() => {
            (query_value_as_number(left).expect("checked numeric left")
                - query_value_as_number(right).expect("checked numeric right"))
            .abs()
                < f64::EPSILON
        }
        _ => left.as_state_string() == right.as_state_string(),
    }
}

fn compare_ordered(left: &QueryValue, right: &QueryValue) -> Option<i8> {
    match (query_value_as_number(left), query_value_as_number(right)) {
        (Some(left), Some(right)) => {
            if left < right {
                Some(-1)
            } else if left > right {
                Some(1)
            } else {
                Some(0)
            }
        }
        _ => {
            let left = left.as_state_string();
            let right = right.as_state_string();
            match left.cmp(&right) {
                std::cmp::Ordering::Less => Some(-1),
                std::cmp::Ordering::Equal => Some(0),
                std::cmp::Ordering::Greater => Some(1),
            }
        }
    }
}

fn query_value_as_number(value: &QueryValue) -> Option<f64> {
    match value {
        QueryValue::Number(value) => Some(*value),
        QueryValue::String(value) => value.trim().parse::<f64>().ok(),
        QueryValue::Boolean(_) => None,
    }
}

fn sql_like(value: &str, pattern: &str) -> bool {
    if pattern == "%" {
        return true;
    }

    let parts = pattern.split('%').collect::<Vec<_>>();
    if parts.len() == 1 {
        return value == pattern;
    }

    let mut remainder = value;
    let starts_with_wildcard = pattern.starts_with('%');
    let ends_with_wildcard = pattern.ends_with('%');

    for (index, part) in parts.iter().filter(|part| !part.is_empty()).enumerate() {
        if index == 0 && !starts_with_wildcard {
            if !remainder.starts_with(part) {
                return false;
            }
            remainder = &remainder[part.len()..];
            continue;
        }

        let Some(position) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[position + part.len()..];
    }

    ends_with_wildcard
        || parts
            .last()
            .is_none_or(|last| last.is_empty() || remainder.is_empty())
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Identifier(String),
    String(String),
    Number(f64),
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Dot,
    LParen,
    RParen,
    Semicolon,
}

fn tokenize(input: &str) -> OcelResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((index, character)) = chars.next() {
        match character {
            character if character.is_whitespace() => {}
            '-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
                for (_, next) in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '\'' | '"' => {
                let quote = character;
                let mut value = String::new();
                let mut closed = false;
                while let Some((_, next)) = chars.next() {
                    if next == quote {
                        if chars.peek().is_some_and(|(_, repeated)| *repeated == quote) {
                            chars.next();
                            value.push(quote);
                        } else {
                            closed = true;
                            break;
                        }
                    } else {
                        value.push(next);
                    }
                }
                if !closed {
                    return Err(OcelError::new("unterminated string literal in state query"));
                }
                tokens.push(Token::String(value));
            }
            '=' => tokens.push(Token::Eq),
            '!' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Ne);
            }
            '<' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Le);
            }
            '<' if chars.peek().is_some_and(|(_, next)| *next == '>') => {
                chars.next();
                tokens.push(Token::Ne);
            }
            '<' => tokens.push(Token::Lt),
            '>' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Ge);
            }
            '>' => tokens.push(Token::Gt),
            '.' => tokens.push(Token::Dot),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ';' => tokens.push(Token::Semicolon),
            character if character.is_ascii_digit() || character == '-' => {
                let mut literal = character.to_string();
                while let Some((_, next)) = chars.peek() {
                    if next.is_ascii_digit() || *next == '.' {
                        literal.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let number = literal.parse::<f64>().map_err(|err| {
                    OcelError::new(format!("invalid numeric literal '{literal}': {err}"))
                })?;
                tokens.push(Token::Number(number));
            }
            character if is_identifier_start(character) => {
                let mut identifier = character.to_string();
                while let Some((_, next)) = chars.peek() {
                    if is_identifier_part(*next) {
                        identifier.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Identifier(identifier));
            }
            other => {
                return Err(OcelError::new(format!(
                    "unexpected character '{other}' at byte {index} in state query"
                )));
            }
        }
    }

    Ok(tokens)
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_part(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

struct QueryParser {
    tokens: Vec<Token>,
    position: usize,
}

impl QueryParser {
    fn parse_state_query(&mut self) -> OcelResult<StateQuery> {
        self.expect_keyword("STATE")?;
        let attribute_name = self.parse_identifier()?;
        self.expect_keyword("AS")?;
        self.expect_keyword("CASE")?;

        let mut branches = Vec::new();
        while self.consume_keyword("WHEN") {
            let condition = self.parse_expr()?;
            self.expect_keyword("THEN")?;
            let value = self.parse_value_expr()?;
            branches.push(StateBranch { condition, value });
        }

        if branches.is_empty() {
            return Err(OcelError::new(
                "state query must contain at least one WHEN branch",
            ));
        }

        let else_value = if self.consume_keyword("ELSE") {
            Some(self.parse_value_expr()?)
        } else {
            None
        };

        self.expect_keyword("END")?;
        self.consume_token(&Token::Semicolon);
        if !self.is_done() {
            return Err(OcelError::new("unexpected tokens after END in state query"));
        }

        Ok(StateQuery {
            attribute_name,
            branches,
            else_value,
        })
    }

    fn parse_expr(&mut self) -> OcelResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> OcelResult<Expr> {
        let mut expr = self.parse_and()?;
        while self.consume_keyword("OR") {
            let right = self.parse_and()?;
            expr = Expr::Or(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> OcelResult<Expr> {
        let mut expr = self.parse_not()?;
        while self.consume_keyword("AND") {
            let right = self.parse_not()?;
            expr = Expr::And(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    fn parse_not(&mut self) -> OcelResult<Expr> {
        if self.consume_keyword("NOT") {
            Ok(Expr::Not(Box::new(self.parse_not()?)))
        } else {
            self.parse_predicate()
        }
    }

    fn parse_predicate(&mut self) -> OcelResult<Expr> {
        if self.consume_token(&Token::LParen) {
            let expr = self.parse_expr()?;
            self.expect_token(&Token::RParen)?;
            return Ok(expr);
        }

        let left = self.parse_value_expr()?;

        if self.consume_keyword("IS") {
            let negated = self.consume_keyword("NOT");
            self.expect_keyword("NULL")?;
            return Ok(Expr::IsNull {
                value: left,
                negated,
            });
        }

        let op = self.parse_compare_op()?;
        let right = self.parse_value_expr()?;
        Ok(Expr::Compare { left, op, right })
    }

    fn parse_compare_op(&mut self) -> OcelResult<CompareOp> {
        if self.consume_token(&Token::Eq) {
            Ok(CompareOp::Eq)
        } else if self.consume_token(&Token::Ne) {
            Ok(CompareOp::Ne)
        } else if self.consume_token(&Token::Le) {
            Ok(CompareOp::Le)
        } else if self.consume_token(&Token::Lt) {
            Ok(CompareOp::Lt)
        } else if self.consume_token(&Token::Ge) {
            Ok(CompareOp::Ge)
        } else if self.consume_token(&Token::Gt) {
            Ok(CompareOp::Gt)
        } else if self.consume_keyword("LIKE") {
            Ok(CompareOp::Like)
        } else {
            Err(OcelError::new(
                "expected comparison operator in state query predicate",
            ))
        }
    }

    fn parse_value_expr(&mut self) -> OcelResult<ValueExpr> {
        match self.next().cloned() {
            Some(Token::String(value)) => Ok(ValueExpr::Literal(QueryValue::String(value))),
            Some(Token::Number(value)) => Ok(ValueExpr::Literal(QueryValue::Number(value))),
            Some(Token::Identifier(identifier)) => {
                if identifier.eq_ignore_ascii_case("TRUE") {
                    return Ok(ValueExpr::Literal(QueryValue::Boolean(true)));
                }
                if identifier.eq_ignore_ascii_case("FALSE") {
                    return Ok(ValueExpr::Literal(QueryValue::Boolean(false)));
                }
                if self.consume_token(&Token::Dot) {
                    let field = self.parse_field_name()?;
                    if identifier.eq_ignore_ascii_case("EVENT") {
                        Ok(ValueExpr::Field(FieldRef::Event(
                            field.to_ascii_lowercase(),
                        )))
                    } else if identifier.eq_ignore_ascii_case("OBJECT") {
                        Ok(ValueExpr::Field(FieldRef::Object(
                            field.to_ascii_lowercase(),
                        )))
                    } else {
                        Err(OcelError::new(format!(
                            "unsupported field namespace '{identifier}', expected event or object"
                        )))
                    }
                } else {
                    Ok(ValueExpr::Literal(QueryValue::String(identifier)))
                }
            }
            other => Err(OcelError::new(format!(
                "expected field or literal in state query, found {other:?}"
            ))),
        }
    }

    fn parse_field_name(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) | Some(Token::String(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected field name in state query, found {other:?}"
            ))),
        }
    }

    fn parse_identifier(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected identifier in state query, found {other:?}"
            ))),
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> OcelResult<()> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(OcelError::new(format!(
                "expected keyword {keyword} in state query"
            )))
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        match self.tokens.get(self.position) {
            Some(Token::Identifier(identifier)) if identifier.eq_ignore_ascii_case(keyword) => {
                self.position += 1;
                true
            }
            _ => false,
        }
    }

    fn expect_token(&mut self, token: &Token) -> OcelResult<()> {
        if self.consume_token(token) {
            Ok(())
        } else {
            Err(OcelError::new(format!(
                "expected token {token:?} in state query"
            )))
        }
    }

    fn consume_token(&mut self, token: &Token) -> bool {
        if self.tokens.get(self.position) == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    fn is_done(&self) -> bool {
        self.position >= self.tokens.len()
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
    use std::collections::HashSet;
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
    fn applies_sql_like_state_query_to_events() {
        let mut log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let result = log
            .apply_state_query(
                r#"
                STATE state AS CASE
                  WHEN object.is_blocked = 'Yes' THEN 'Blocked'
                  WHEN event.type LIKE '%Payment%' THEN 'Payment'
                  ELSE 'Normal'
                END
                "#,
            )
            .unwrap();

        assert!(result.contains(r#""assigned_events":13"#));
        assert_eq!(log.summary().stateful_events, 13);
        assert_eq!(event_state(&log, "e11"), Some("Blocked".to_owned()));
        assert_eq!(event_state(&log, "e9"), Some("Normal".to_owned()));
        assert_eq!(event_state(&log, "e13"), Some("Payment".to_owned()));

        let exported = log.export_json().unwrap();
        let reparsed = CompactOcelLog::from_input(&exported, Some("json")).unwrap();
        assert_eq!(reparsed.summary().stateful_events, 13);
        assert_eq!(event_state(&reparsed, "e11"), Some("Blocked".to_owned()));
    }

    #[test]
    fn supports_event_field_values_as_state_results() {
        let mut log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        log.apply_state_query(
            r#"
            STATE state AS CASE
              WHEN event.type = 'Insert Invoice' THEN event.type
              ELSE 'Other'
            END
            "#,
        )
        .unwrap();

        assert_eq!(event_state(&log, "e5"), Some("Insert Invoice".to_owned()));
        assert_eq!(event_state(&log, "e1"), Some("Other".to_owned()));
    }

    #[test]
    fn state_pattern_detection_requires_applied_states() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let error = log.detect_state_patterns().unwrap_err();

        assert!(error.to_string().contains("apply a state query first"));
    }

    #[test]
    fn detects_ranked_state_patterns_after_state_enrichment() {
        let mut log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        log.apply_state_query(
            r#"
            STATE state AS CASE
              WHEN object.is_blocked = 'Yes' THEN 'Invoice Blocked'
              WHEN event.type LIKE '%Payment%' THEN 'Payment Execution'
              WHEN event.type LIKE '%Invoice%' THEN 'Invoice Handling'
              ELSE 'Procurement'
            END
            "#,
        )
        .unwrap();

        let patterns = log.detect_state_patterns().unwrap();
        assert!(!patterns.intra.is_empty());
        assert!(!patterns.inter.is_empty());
        assert_patterns_sorted(&patterns.intra);
        assert_patterns_sorted(&patterns.inter);

        let first_intra = &patterns.intra[0];
        assert_eq!(first_intra.family, "intra");
        assert!(first_intra.sequence[0].starts_with("START "));
        assert!(first_intra
            .sequence
            .iter()
            .any(|label| label.contains(" [")));
        assert!(!first_intra.df_edges.is_empty());
        assert!(!first_intra.object_types.is_empty());

        assert!(patterns.inter.iter().any(|pattern| {
            pattern.family == "inter"
                && pattern.from_state.is_some()
                && pattern.to_state.is_some()
                && pattern
                    .sequence
                    .iter()
                    .any(|label| label.starts_with("CHANGE "))
        }));

        let json = log.state_patterns_json().unwrap();
        let exported: Value = serde_json::from_str(&json).unwrap();
        assert!(exported["intra"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
        assert!(exported["inter"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
    }

    #[test]
    fn detects_state_patterns_for_inventory_fixture() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../files/ocel2/inventory_management_simulated.json");
        let input = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
        let mut log = CompactOcelLog::from_input(&input, Some("json")).unwrap();
        log.apply_state_query(
            r#"STATE state AS CASE
  WHEN event."Stock After" = 0 THEN 'Zero Stock'
  WHEN event."Stock After" < 30 THEN 'Low Stock'
  WHEN event."Stock After" >= 100 THEN 'High Stock'
  ELSE 'Available Stock'
END"#,
        )
        .unwrap();

        let patterns = log.detect_state_patterns().unwrap();
        assert!(!patterns.intra.is_empty());
        assert!(!patterns.inter.is_empty());
        assert_patterns_sorted(&patterns.intra);
        assert_patterns_sorted(&patterns.inter);
        assert!(patterns
            .intra
            .iter()
            .any(|pattern| pattern.state.as_deref() == Some("Zero Stock")));
        assert!(patterns
            .inter
            .iter()
            .any(
                |pattern| pattern.from_state.as_deref() == Some("Zero Stock")
                    || pattern.to_state.as_deref() == Some("Zero Stock")
            ));
    }

    #[test]
    fn preset_state_queries_apply_to_fixture_logs() {
        for (fixture, queries) in fixture_state_queries() {
            let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../files/ocel2")
                .join(fixture);
            let input = fs::read_to_string(&fixture_path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
            let mut log = CompactOcelLog::from_input(&input, Some("json"))
                .unwrap_or_else(|err| panic!("failed to import {fixture}: {err}"));

            for (name, query, expected_states) in queries {
                log.apply_state_query(query)
                    .unwrap_or_else(|err| panic!("preset '{name}' failed on {fixture}: {err}"));
                assert_eq!(
                    log.summary().stateful_events,
                    log.summary().events,
                    "preset '{name}' should assign all events in {fixture}"
                );
                assert_eq!(
                    event_states(&log),
                    expected_states
                        .iter()
                        .map(|state| state.to_string())
                        .collect::<HashSet<_>>(),
                    "preset '{name}' produced unexpected states in {fixture}"
                );
            }
        }
    }

    #[test]
    fn imports_new_inventory_management_fixtures() {
        for (fixture, format) in [
            ("inventory_management_simulated.json", "json"),
            ("inventory_management_simulated.xml", "xml"),
        ] {
            let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../files/ocel2")
                .join(fixture);
            let input = fs::read_to_string(&fixture_path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
            let summary = CompactOcelLog::from_input(&input, Some(format))
                .unwrap_or_else(|err| panic!("failed to import {fixture}: {err}"))
                .summary();

            assert_eq!(summary.event_types, 5);
            assert_eq!(summary.object_types, 7);
            assert_eq!(summary.events, 1210);
            assert_eq!(summary.objects, 1701);
            assert_eq!(summary.e2o_relationships, 5767);
            assert_eq!(summary.o2o_relationships, 0);
        }
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

    fn assert_patterns_sorted(patterns: &[PatternSummary]) {
        for pair in patterns.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            assert!(
                left.support > right.support
                    || (left.support == right.support && left.mass >= right.mass),
                "patterns should be sorted by descending support and mass: {left:?} before {right:?}"
            );
        }
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

    fn event_state(log: &CompactOcelLog, event_id: &str) -> Option<String> {
        let event = log
            .events
            .iter()
            .find(|event| log.pool.resolve(event.id) == event_id)?;
        event.attributes.iter().find_map(|attribute| {
            if log.pool.resolve(attribute.name) == "state" {
                match &attribute.value {
                    AttrValue::String(symbol) => Some(log.pool.resolve(*symbol).to_owned()),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    fn event_states(log: &CompactOcelLog) -> HashSet<String> {
        log.events
            .iter()
            .flat_map(|event| {
                event.attributes.iter().filter_map(|attribute| {
                    if log.pool.resolve(attribute.name) == "state" {
                        match &attribute.value {
                            AttrValue::String(symbol) => Some(log.pool.resolve(*symbol).to_owned()),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    fn fixture_state_queries() -> Vec<(
        &'static str,
        Vec<(&'static str, &'static str, &'static [&'static str])>,
    )> {
        vec![
            (
                "ocel20_example.json",
                vec![
                    (
                        "Payment Block Status",
                        r#"STATE state AS CASE
  WHEN object.is_blocked = 'Yes' THEN 'Invoice Blocked'
  WHEN event.type LIKE '%Payment%' THEN 'Payment Execution'
  WHEN event.type LIKE '%Invoice%' THEN 'Invoice Handling'
  ELSE 'Procurement'
END"#,
                        &[
                            "Invoice Blocked",
                            "Payment Execution",
                            "Invoice Handling",
                            "Procurement",
                        ],
                    ),
                    (
                        "Purchase Size",
                        r#"STATE state AS CASE
  WHEN object.po_quantity > 500 THEN 'Large PO'
  WHEN object.pr_quantity >= 500 THEN 'Large Requisition'
  WHEN object.po_product = 'Notebooks' THEN 'Maverick Buying'
  ELSE 'Standard Purchase'
END"#,
                        &[
                            "Large PO",
                            "Large Requisition",
                            "Maverick Buying",
                            "Standard Purchase",
                        ],
                    ),
                    (
                        "Actor and Automation",
                        r#"STATE state AS CASE
  WHEN event.invoice_blocker IS NOT NULL OR event.invoice_block_rem IS NOT NULL THEN 'Manual Block Control'
  WHEN event.payment_inserter = 'Robot' THEN 'Automated Payment'
  WHEN event.po_creator = 'Mario' OR event.invoice_inserter = 'Mario' THEN 'Maverick Flow'
  ELSE 'Regular Work'
END"#,
                        &[
                            "Manual Block Control",
                            "Automated Payment",
                            "Maverick Flow",
                            "Regular Work",
                        ],
                    ),
                ],
            ),
            (
                "container_logistics.json",
                vec![
                    (
                        "Shipment Status",
                        r#"STATE state AS CASE
  WHEN object.Status = 'shipped' THEN 'Shipped'
  WHEN object.Status = 'in transit' THEN 'In Transit'
  WHEN object.Status = 'full' THEN 'Loaded'
  WHEN object.Status = 'empty' THEN 'Empty'
  ELSE 'Planning'
END"#,
                        &["Shipped", "In Transit", "Loaded", "Empty", "Planning"],
                    ),
                    (
                        "Load Planning",
                        r#"STATE state AS CASE
  WHEN event.type = 'Book Vehicles' THEN 'Vehicle Booking'
  WHEN event.type LIKE '%Load%' THEN 'Transport Loading'
  WHEN object.AmountofGoods >= 900 THEN 'Large Order'
  ELSE 'Standard Load'
END"#,
                        &[
                            "Vehicle Booking",
                            "Transport Loading",
                            "Large Order",
                            "Standard Load",
                        ],
                    ),
                    (
                        "Process Phase",
                        r#"STATE state AS CASE
  WHEN event.type LIKE '%Depart%' OR event.type LIKE '%Drive%' THEN 'Outbound'
  WHEN event.type LIKE '%Load%' OR event.type LIKE '%Weigh%' THEN 'Loading'
  WHEN event.type LIKE '%Order%' OR event.type LIKE '%Create%' OR event.type LIKE '%Book%' THEN 'Planning'
  ELSE 'Warehouse Handling'
END"#,
                        &["Outbound", "Loading", "Planning", "Warehouse Handling"],
                    ),
                ],
            ),
            (
                "order-management.json",
                vec![
                    (
                        "Fulfillment Stage",
                        r#"STATE state AS CASE
  WHEN event.type = 'failed delivery' THEN 'Delivery Failure'
  WHEN event.type = 'package delivered' THEN 'Delivered'
  WHEN event.type LIKE '%package%' OR event.type = 'send package' THEN 'Packaging'
  WHEN event.type LIKE '%pay%' OR event.type = 'payment reminder' THEN 'Payment'
  ELSE 'Order Handling'
END"#,
                        &[
                            "Delivery Failure",
                            "Delivered",
                            "Packaging",
                            "Payment",
                            "Order Handling",
                        ],
                    ),
                    (
                        "Value and Weight",
                        r#"STATE state AS CASE
  WHEN object.weight >= 10 THEN 'Heavy'
  WHEN object.price >= 1000 THEN 'High Value'
  WHEN object.price >= 250 THEN 'Medium Value'
  ELSE 'Standard'
END"#,
                        &["High Value", "Medium Value", "Heavy", "Standard"],
                    ),
                    (
                        "Exception Risk",
                        r#"STATE state AS CASE
  WHEN event.type = 'item out of stock' THEN 'Stock Exception'
  WHEN event.type = 'reorder item' THEN 'Replenishment'
  WHEN event.type = 'payment reminder' THEN 'Payment Risk'
  WHEN event.type = 'failed delivery' THEN 'Delivery Risk'
  ELSE 'Nominal'
END"#,
                        &[
                            "Stock Exception",
                            "Replenishment",
                            "Payment Risk",
                            "Delivery Risk",
                            "Nominal",
                        ],
                    ),
                ],
            ),
            (
                "inventory_management_simulated.json",
                vec![
                    (
                        "Stock Status",
                        r#"STATE state AS CASE
  WHEN event."Stock After" = 0 THEN 'Zero Stock'
  WHEN event."Stock After" < 30 THEN 'Low Stock'
  WHEN event."Stock After" >= 100 THEN 'High Stock'
  ELSE 'Available Stock'
END"#,
                        &["Zero Stock", "Low Stock", "High Stock", "Available Stock"],
                    ),
                    (
                        "Activity Phase",
                        r#"STATE state AS CASE
  WHEN event.type = 'Goods Receipt' THEN 'Goods Receipt'
  WHEN event.type = 'Goods Issue' THEN 'Goods Issue'
  WHEN event.type = 'Create Purchase Order Item' THEN 'Purchase Order'
  WHEN event.type = 'Create Purchase Suggestion Item' THEN 'Purchase Suggestion'
  WHEN event.type = 'Create Sales Order Item' THEN 'Sales Order'
  ELSE 'Inventory Activity'
END"#,
                        &[
                            "Goods Receipt",
                            "Goods Issue",
                            "Purchase Order",
                            "Purchase Suggestion",
                            "Sales Order",
                        ],
                    ),
                    (
                        "Stock Movement",
                        r#"STATE state AS CASE
  WHEN event."Stock After" > event."Stock Before" THEN 'Stock Increase'
  WHEN event."Stock After" < event."Stock Before" THEN 'Stock Decrease'
  WHEN event."Stock After" = 0 THEN 'Zero Stable'
  ELSE 'No Stock Change'
END"#,
                        &[
                            "Stock Increase",
                            "Stock Decrease",
                            "Zero Stable",
                            "No Stock Change",
                        ],
                    ),
                ],
            ),
        ]
    }
}
