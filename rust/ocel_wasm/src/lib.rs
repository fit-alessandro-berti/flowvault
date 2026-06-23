//! OCEL 2.0 import/export core.
//!
//! The public WebAssembly API accepts standard OCEL 2.0 JSON and XML files and
//! returns standard JSON/XML exports. Internally, the log is stored in a compact
//! representation: repeated strings are interned once, object/event references
//! are symbols, timestamps are Unix timestamps in milliseconds, and every object
//! keeps a timestamp-ordered lifecycle of related events.

use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use flate2::read::GzDecoder;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::{Display, Write};
use std::io::Read;
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct Symbol(u32);

#[derive(Debug, Default, Clone)]
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

#[derive(Debug, Clone)]
struct AttributeDef {
    name: Symbol,
    attr_type: AttrType,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
struct Attribute {
    name: Symbol,
    value: AttrValue,
}

#[derive(Debug, Clone)]
struct TimedAttribute {
    name: Symbol,
    time_ms: i64,
    value: AttrValue,
}

#[derive(Debug, Clone)]
struct Relationship {
    object_id: Symbol,
    qualifier: Symbol,
}

#[derive(Debug, Clone)]
struct Event {
    id: Symbol,
    type_name: Symbol,
    time_ms: i64,
    attributes: Vec<Attribute>,
    relationships: Vec<Relationship>,
}

#[derive(Debug, Clone)]
struct Object {
    id: Symbol,
    type_name: Symbol,
    attributes: Vec<TimedAttribute>,
    relationships: Vec<Relationship>,
    lifecycle: Vec<usize>,
}

#[derive(Debug, Clone)]
struct CompactOcelLog {
    format: OcelFormat,
    pool: StringPool,
    event_types: Vec<TypeDef>,
    object_types: Vec<TypeDef>,
    events: Vec<Event>,
    objects: Vec<Object>,
    object_index: HashMap<Symbol, usize>,
    state_leading_object_type: Option<Symbol>,
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

#[derive(Clone, Deserialize)]
struct OcelFilterRequest {
    event_types: Vec<String>,
    object_types: Vec<String>,
}

#[derive(Serialize)]
struct FilterOptions {
    event_types: Vec<String>,
    object_types: Vec<String>,
}

#[derive(Default, Deserialize)]
struct GraphFilterRequest {
    object_types: Option<Vec<String>>,
    min_activity_frequency: Option<usize>,
    min_path_frequency: Option<usize>,
}

#[derive(Deserialize)]
struct StateDetectionRequest {
    object_type: String,
    window_size: Option<usize>,
    som_width: Option<usize>,
    som_height: Option<usize>,
    epochs: Option<usize>,
    color_attribute: Option<String>,
}

#[derive(Deserialize)]
struct StateDetectionCellRequest {
    object_type: String,
    window_size: Option<usize>,
    som_width: Option<usize>,
    som_height: Option<usize>,
    epochs: Option<usize>,
    color_attribute: Option<String>,
    cell_x: usize,
    cell_y: usize,
}

#[derive(Default)]
struct GraphLayoutFilter {
    min_activity_frequency: usize,
    min_path_frequency: usize,
}

impl OcelFilterRequest {
    fn all_for(log: &CompactOcelLog) -> Self {
        let options = log.filter_options();
        Self {
            event_types: options.event_types,
            object_types: options.object_types,
        }
    }
}

impl GraphFilterRequest {
    fn layout_filter(&self) -> GraphLayoutFilter {
        GraphLayoutFilter {
            min_activity_frequency: self.min_activity_frequency.unwrap_or_default(),
            min_path_frequency: self.min_path_frequency.unwrap_or_default(),
        }
    }
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

    fn filter(&self, filter: &OcelFilterRequest) -> Self {
        let event_type_selection = filter
            .event_types
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let object_type_selection = filter
            .object_types
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let all_event_types_selected = event_type_selection.len() >= self.event_types.len();
        let all_object_types_selected = object_type_selection.len() >= self.object_types.len();

        if all_event_types_selected && all_object_types_selected {
            return self.clone();
        }

        let selected_object_ids = self
            .objects
            .iter()
            .filter(|object| object_type_selection.contains(self.pool.resolve(object.type_name)))
            .map(|object| object.id)
            .collect::<HashSet<_>>();

        let mut retained_events = Vec::new();
        let mut retained_object_ids = HashSet::new();

        for event in &self.events {
            if !event_type_selection.contains(self.pool.resolve(event.type_name)) {
                continue;
            }

            let relationships = event
                .relationships
                .iter()
                .filter(|relationship| selected_object_ids.contains(&relationship.object_id))
                .cloned()
                .collect::<Vec<_>>();

            if !all_object_types_selected && relationships.is_empty() {
                continue;
            }

            retained_object_ids.extend(
                relationships
                    .iter()
                    .map(|relationship| relationship.object_id),
            );
            retained_events.push(Event {
                id: event.id,
                type_name: event.type_name,
                time_ms: event.time_ms,
                attributes: event.attributes.clone(),
                relationships,
            });
        }

        let mut objects = self
            .objects
            .iter()
            .filter(|object| retained_object_ids.contains(&object.id))
            .map(|object| Object {
                id: object.id,
                type_name: object.type_name,
                attributes: object.attributes.clone(),
                relationships: object
                    .relationships
                    .iter()
                    .filter(|relationship| retained_object_ids.contains(&relationship.object_id))
                    .cloned()
                    .collect(),
                lifecycle: Vec::new(),
            })
            .collect::<Vec<_>>();
        let object_index = objects
            .iter()
            .enumerate()
            .map(|(index, object)| (object.id, index))
            .collect::<HashMap<_, _>>();

        for (event_index, event) in retained_events.iter().enumerate() {
            for relationship in &event.relationships {
                if let Some(object_pos) = object_index.get(&relationship.object_id) {
                    objects[*object_pos].lifecycle.push(event_index);
                }
            }
        }

        for object in &mut objects {
            object
                .lifecycle
                .sort_by_key(|event_index| (retained_events[*event_index].time_ms, *event_index));
        }

        let retained_event_types = retained_events
            .iter()
            .map(|event| event.type_name)
            .collect::<HashSet<_>>();
        let retained_object_types = objects
            .iter()
            .map(|object| object.type_name)
            .collect::<HashSet<_>>();

        Self {
            format: self.format,
            pool: self.pool.clone(),
            event_types: self
                .event_types
                .iter()
                .filter(|type_def| retained_event_types.contains(&type_def.name))
                .cloned()
                .collect(),
            object_types: self
                .object_types
                .iter()
                .filter(|type_def| retained_object_types.contains(&type_def.name))
                .cloned()
                .collect(),
            events: retained_events,
            objects,
            object_index,
            state_leading_object_type: self.state_leading_object_type,
        }
    }

    fn filter_options(&self) -> FilterOptions {
        let event_types = self
            .event_types
            .iter()
            .map(|type_def| self.pool.resolve(type_def.name).to_owned())
            .collect();
        let object_types = self
            .object_types
            .iter()
            .map(|type_def| self.pool.resolve(type_def.name).to_owned())
            .collect();

        FilterOptions {
            event_types,
            object_types,
        }
    }

    fn object_type_symbol(&self, object_type: &str) -> Option<Symbol> {
        self.object_types
            .iter()
            .find(|type_def| self.pool.resolve(type_def.name) == object_type)
            .map(|type_def| type_def.name)
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
        let leading_type_symbol = self
            .object_type_symbol(&state_query.leading_object_type)
            .ok_or_else(|| {
                OcelError::new(format!(
                    "unknown leading object type '{}'",
                    state_query.leading_object_type
                ))
            })?;
        let eval_index = StateEvalIndex::build(self, &state_query);
        let attribute_symbol = self.pool.intern(&state_query.attribute_name);
        self.ensure_event_attribute(attribute_symbol, AttrType::String);
        self.state_leading_object_type = Some(leading_type_symbol);

        for event in &mut self.events {
            event
                .attributes
                .retain(|attribute| attribute.name != attribute_symbol);
        }

        let mut assigned = 0usize;
        for event_index in 0..self.events.len() {
            if let Some(state) = self.evaluate_state_query(&state_query, &eval_index, event_index) {
                let state_symbol = self.pool.intern(&state);
                let event = &mut self.events[event_index];
                event.attributes.push(Attribute {
                    name: attribute_symbol,
                    value: AttrValue::String(state_symbol),
                });
                assigned += 1;
            }
        }

        let result = StateQueryResult {
            attribute: state_query.attribute_name,
            leading_object_type: state_query.leading_object_type,
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

    fn state_detection_json(&self, request: &StateDetectionRequest) -> OcelResult<String> {
        let analysis = self.detect_execution_states(request)?;
        serde_json::to_string(&analysis)
            .map_err(|err| OcelError::new(format!("could not serialize state detection: {err}")))
    }

    fn state_detection_cell_json(&self, request: &StateDetectionCellRequest) -> OcelResult<String> {
        let state_request = StateDetectionRequest {
            object_type: request.object_type.clone(),
            window_size: request.window_size,
            som_width: request.som_width,
            som_height: request.som_height,
            epochs: request.epochs,
            color_attribute: request.color_attribute.clone(),
        };
        let run = self.compute_state_detection_run(&state_request)?;
        let detail = self.state_detection_cell_detail(&run, request.cell_x, request.cell_y)?;
        serde_json::to_string(&detail).map_err(|err| {
            OcelError::new(format!("could not serialize state detection cell: {err}"))
        })
    }

    fn state_feature_table_csv(&self, request: &StateDetectionRequest) -> OcelResult<String> {
        let table = self.state_feature_table(&request.object_type)?;
        Ok(feature_table_to_csv(&table))
    }

    fn state_feature_table(&self, object_type: &str) -> OcelResult<FeatureTableData> {
        let object_type_symbol = self.object_type_symbol(object_type).ok_or_else(|| {
            OcelError::new(format!(
                "unknown object type '{object_type}' for state detection"
            ))
        })?;
        let object_indices = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| (object.type_name == object_type_symbol).then_some(index))
            .collect::<Vec<_>>();
        let encoder = self.build_feature_encoder(&object_indices);
        let columns = encoder.columns.iter().map(FeatureColumn::label).collect();
        let rows = object_indices
            .iter()
            .map(|object_index| {
                let object = &self.objects[*object_index];
                FeatureRow {
                    object_id: self.pool.resolve(object.id).to_owned(),
                    values: self.encode_feature_vector(
                        *object_index,
                        &object.lifecycle,
                        i64::MAX,
                        &encoder,
                    ),
                }
            })
            .collect();

        Ok(FeatureTableData { columns, rows })
    }

    fn detect_execution_states(
        &self,
        request: &StateDetectionRequest,
    ) -> OcelResult<StateDetectionResult> {
        let run = self.compute_state_detection_run(request)?;
        let window_size = request.window_size.unwrap_or(4).clamp(1, 30);
        let som_width = run.som.width;
        let som_height = run.som.weights.len() / som_width;
        let som_summary =
            self.summarize_som(&run.windows, &run.pca.points, &run.som, &run.color_metric);
        let feature_columns = run
            .encoder
            .columns
            .iter()
            .map(FeatureColumn::label)
            .collect::<Vec<_>>();
        let table_preview = run
            .feature_table
            .rows
            .iter()
            .take(15)
            .map(|row| FeaturePreviewRow {
                object_id: row.object_id.clone(),
                values: row.values.clone(),
            })
            .collect();
        let projected_windows = self.projected_windows(&run, 500);

        Ok(StateDetectionResult {
            object_type: request.object_type.clone(),
            window_size,
            som_width,
            som_height,
            color_attribute: run.color_metric.id(),
            color_attributes: run.color_options,
            object_count: run.object_indices.len(),
            feature_count: feature_columns.len(),
            window_count: run.windows.len(),
            feature_columns,
            table_preview,
            pca: PcaSummary {
                pc1_variance: round_f64(run.pca.pc1_variance),
                pc2_variance: round_f64(run.pca.pc2_variance),
                pc1_explained_ratio: round_f64(run.pca.pc1_explained_ratio),
                pc2_explained_ratio: round_f64(run.pca.pc2_explained_ratio),
            },
            som: som_summary,
            windows: projected_windows,
        })
    }

    fn compute_state_detection_run(
        &self,
        request: &StateDetectionRequest,
    ) -> OcelResult<StateDetectionRun> {
        let object_type_symbol =
            self.object_type_symbol(&request.object_type)
                .ok_or_else(|| {
                    OcelError::new(format!(
                        "unknown object type '{}' for state detection",
                        request.object_type
                    ))
                })?;
        let object_indices = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| (object.type_name == object_type_symbol).then_some(index))
            .collect::<Vec<_>>();
        if object_indices.is_empty() {
            return Err(OcelError::new(format!(
                "no objects of type '{}' are available in the active log",
                request.object_type
            )));
        }

        let encoder = self.build_feature_encoder(&object_indices);
        if encoder.columns.is_empty() {
            return Err(OcelError::new(format!(
                "no numerical features could be extracted for '{}'",
                request.object_type
            )));
        }

        let window_size = request.window_size.unwrap_or(4).clamp(1, 30);
        let windows = self.encode_lifecycle_windows(&object_indices, window_size, &encoder);
        if windows.is_empty() {
            return Err(OcelError::new(format!(
                "no lifecycle windows could be extracted for '{}'",
                request.object_type
            )));
        }

        let values = windows
            .iter()
            .map(|window| window.values.clone())
            .collect::<Vec<_>>();
        let pca = pca_project(&values);
        let (som_width, som_height) =
            default_som_dimensions(pca.points.len(), request.som_width, request.som_height);
        let som = train_som(
            &pca.points,
            som_width,
            som_height,
            request.epochs.unwrap_or(120).clamp(10, 500),
        );
        let feature_table = self.state_feature_table(&request.object_type)?;
        let color_options = self.state_detection_color_options(&object_indices);
        let color_metric =
            self.resolve_color_metric(request.color_attribute.as_deref(), &color_options);

        Ok(StateDetectionRun {
            object_indices,
            encoder,
            feature_table,
            windows,
            pca,
            som,
            color_metric,
            color_options,
        })
    }

    fn state_detection_color_options(
        &self,
        object_indices: &[usize],
    ) -> Vec<StateDetectionColorOption> {
        let mut attributes = BTreeMap::<String, AttributeFeatureCollector>::new();
        for object_index in object_indices {
            let object = &self.objects[*object_index];
            for attribute in &object.attributes {
                let entry = attributes
                    .entry(self.pool.resolve(attribute.name).to_owned())
                    .or_default();
                if attr_value_to_f64(&attribute.value).is_some() {
                    entry.has_numeric = true;
                } else {
                    entry
                        .categories
                        .insert(self.attr_value_label(&attribute.value));
                }
            }
        }

        let mut options = vec![StateDetectionColorOption {
            id: "__window_count".to_owned(),
            label: "Assigned windows".to_owned(),
            kind: "count",
        }];
        for (name, collector) in attributes {
            if collector.has_numeric && collector.categories.is_empty() {
                options.push(StateDetectionColorOption {
                    id: format!("attribute::{name}"),
                    label: name,
                    kind: "numeric",
                });
            } else if !collector.categories.is_empty() && collector.categories.len() < 50 {
                options.push(StateDetectionColorOption {
                    id: format!("attribute::{name}"),
                    label: name,
                    kind: "categorical",
                });
            }
        }
        options
    }

    fn resolve_color_metric(
        &self,
        requested: Option<&str>,
        options: &[StateDetectionColorOption],
    ) -> ColorMetric {
        let Some(requested) = requested else {
            return ColorMetric::WindowCount;
        };
        if requested == "__window_count" {
            return ColorMetric::WindowCount;
        }
        let Some(option) = options.iter().find(|option| option.id == requested) else {
            return ColorMetric::WindowCount;
        };
        let attribute_name = option
            .id
            .strip_prefix("attribute::")
            .unwrap_or(&option.label)
            .to_owned();
        match option.kind {
            "numeric" => ColorMetric::NumericAttribute(attribute_name),
            "categorical" => ColorMetric::CategoricalAttribute(attribute_name),
            _ => ColorMetric::WindowCount,
        }
    }

    fn projected_windows(
        &self,
        run: &StateDetectionRun,
        limit: usize,
    ) -> Vec<StateWindowProjection> {
        run.windows
            .iter()
            .zip(run.pca.points.iter())
            .zip(run.som.assignments.iter())
            .take(limit)
            .map(|((window, (pc1, pc2)), (cell_x, cell_y))| {
                let object = &self.objects[window.object_index];
                let first_event = window
                    .event_indices
                    .first()
                    .map(|event_index| self.pool.resolve(self.events[*event_index].id))
                    .unwrap_or("");
                let last_event = window
                    .event_indices
                    .last()
                    .map(|event_index| self.pool.resolve(self.events[*event_index].id))
                    .unwrap_or("");
                StateWindowProjection {
                    object_id: self.pool.resolve(object.id).to_owned(),
                    start_event: first_event.to_owned(),
                    end_event: last_event.to_owned(),
                    pc1: round_f64(*pc1),
                    pc2: round_f64(*pc2),
                    cell_x: *cell_x,
                    cell_y: *cell_y,
                }
            })
            .collect()
    }

    fn state_detection_cell_detail(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> OcelResult<StateDetectionCellDetail> {
        let height = run.som.weights.len() / run.som.width;
        if cell_x >= run.som.width || cell_y >= height {
            return Err(OcelError::new(format!(
                "SOM cell {},{} is outside the {}x{} grid",
                cell_x, cell_y, run.som.width, height
            )));
        }

        let som_summary =
            self.summarize_som(&run.windows, &run.pca.points, &run.som, &run.color_metric);
        let cell = som_summary
            .cells
            .into_iter()
            .find(|cell| cell.x == cell_x && cell.y == cell_y)
            .expect("validated SOM cell must exist");
        let dfg = self.state_detection_cell_dfg(run, cell_x, cell_y);
        let (entering_windows, exiting_windows, entering_indices, exiting_indices) =
            self.state_detection_boundary_windows(run, cell_x, cell_y);
        let entering_dfg = self.state_detection_windows_dfg(
            run,
            &entering_indices,
            format!("Entering Windows: {}", cell.label),
            "Directly-follows graph over windows entering the selected SOM cell".to_owned(),
        );
        let exiting_dfg = self.state_detection_windows_dfg(
            run,
            &exiting_indices,
            format!("Exiting Windows: {}", cell.label),
            "Directly-follows graph over windows exiting the selected SOM cell".to_owned(),
        );

        Ok(StateDetectionCellDetail {
            cell,
            dfg,
            entering_dfg,
            exiting_dfg,
            entering_window_count: entering_indices.len(),
            exiting_window_count: exiting_indices.len(),
            entering_windows,
            exiting_windows,
        })
    }

    fn state_detection_cell_dfg(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> LayoutGraph {
        let mut graph = GraphAccumulator::new(
            format!("State Detection Cell S{}-{}", cell_x + 1, cell_y + 1),
            "Directly-follows graph over windows assigned to the selected SOM cell".to_owned(),
        );
        let object_type = self
            .pool
            .resolve(self.objects[run.windows[0].object_index].type_name);

        for (window, (assigned_x, assigned_y)) in run.windows.iter().zip(run.som.assignments.iter())
        {
            if *assigned_x != cell_x || *assigned_y != cell_y || window.event_indices.is_empty() {
                continue;
            }
            self.accumulate_window_directly_follows(&mut graph, window, object_type);
        }

        layout_accumulated_graph(graph)
    }

    fn state_detection_windows_dfg(
        &self,
        run: &StateDetectionRun,
        window_indices: &[usize],
        title: String,
        subtitle: String,
    ) -> LayoutGraph {
        let mut graph = GraphAccumulator::new(title, subtitle);
        let object_type = self
            .pool
            .resolve(self.objects[run.windows[0].object_index].type_name);

        for window_index in window_indices {
            if let Some(window) = run.windows.get(*window_index) {
                self.accumulate_window_directly_follows(&mut graph, window, object_type);
            }
        }

        layout_accumulated_graph(graph)
    }

    fn accumulate_window_directly_follows(
        &self,
        graph: &mut GraphAccumulator,
        window: &WindowEncoding,
        object_type: &str,
    ) {
        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            window.event_indices.len() as f64 + 1.0,
            1,
        );

        for (position, event_index) in window.event_indices.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            graph.add_node(event_type, "activity", position as f64 + 1.0, 1);
        }

        if let Some(first_index) = window.event_indices.first() {
            let first = self.pool.resolve(self.events[*first_index].type_name);
            graph.add_edge(&start, first, object_type, 1);
        }
        for pair in window.event_indices.windows(2) {
            let [source_index, target_index] = pair else {
                continue;
            };
            let source = self.pool.resolve(self.events[*source_index].type_name);
            let target = self.pool.resolve(self.events[*target_index].type_name);
            graph.add_edge(source, target, object_type, 1);
        }
        if let Some(last_index) = window.event_indices.last() {
            let last = self.pool.resolve(self.events[*last_index].type_name);
            graph.add_edge(last, &end, object_type, 1);
        }
    }

    fn state_detection_boundary_windows(
        &self,
        run: &StateDetectionRun,
        cell_x: usize,
        cell_y: usize,
    ) -> (
        Vec<StateDetectionBoundaryWindow>,
        Vec<StateDetectionBoundaryWindow>,
        Vec<usize>,
        Vec<usize>,
    ) {
        let mut entering = Vec::new();
        let mut exiting = Vec::new();
        let mut entering_indices = Vec::new();
        let mut exiting_indices = Vec::new();

        for index in 1..run.windows.len() {
            let previous = &run.windows[index - 1];
            let current = &run.windows[index];
            if previous.object_index != current.object_index {
                continue;
            }
            let source = run.som.assignments[index - 1];
            let target = run.som.assignments[index];
            if source == target {
                continue;
            }
            if target == (cell_x, cell_y) {
                if entering.len() < 100 {
                    entering.push(self.boundary_window_summary(run, index, source, target));
                }
                entering_indices.push(index);
            }
            if source == (cell_x, cell_y) {
                if exiting.len() < 100 {
                    exiting.push(self.boundary_window_summary(run, index - 1, source, target));
                }
                exiting_indices.push(index - 1);
            }
        }

        (entering, exiting, entering_indices, exiting_indices)
    }

    fn boundary_window_summary(
        &self,
        run: &StateDetectionRun,
        window_index: usize,
        source: (usize, usize),
        target: (usize, usize),
    ) -> StateDetectionBoundaryWindow {
        let window = &run.windows[window_index];
        let projection = self.projected_window(window, run.pca.points[window_index], target);
        StateDetectionBoundaryWindow {
            object_id: projection.object_id,
            start_event: projection.start_event,
            end_event: projection.end_event,
            source_cell: cell_label(source.0, source.1),
            target_cell: cell_label(target.0, target.1),
            pc1: projection.pc1,
            pc2: projection.pc2,
            activities: self.window_activity_sequence(window),
        }
    }

    fn projected_window(
        &self,
        window: &WindowEncoding,
        point: (f64, f64),
        cell: (usize, usize),
    ) -> StateWindowProjection {
        let object = &self.objects[window.object_index];
        let first_event = window
            .event_indices
            .first()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .unwrap_or("");
        let last_event = window
            .event_indices
            .last()
            .map(|event_index| self.pool.resolve(self.events[*event_index].id))
            .unwrap_or("");
        StateWindowProjection {
            object_id: self.pool.resolve(object.id).to_owned(),
            start_event: first_event.to_owned(),
            end_event: last_event.to_owned(),
            pc1: round_f64(point.0),
            pc2: round_f64(point.1),
            cell_x: cell.0,
            cell_y: cell.1,
        }
    }

    fn window_activity_sequence(&self, window: &WindowEncoding) -> Vec<String> {
        window
            .event_indices
            .iter()
            .map(|event_index| {
                self.pool
                    .resolve(self.events[*event_index].type_name)
                    .to_owned()
            })
            .collect()
    }

    fn build_feature_encoder(&self, object_indices: &[usize]) -> FeatureEncoder {
        let mut activity_types = BTreeSet::<String>::new();
        let mut related_object_types = BTreeSet::<String>::new();
        let mut attributes = BTreeMap::<String, AttributeFeatureCollector>::new();

        for object_index in object_indices {
            let object = &self.objects[*object_index];
            for event_index in &object.lifecycle {
                let event = &self.events[*event_index];
                activity_types.insert(self.pool.resolve(event.type_name).to_owned());
                for relationship in &event.relationships {
                    if relationship.object_id == object.id {
                        continue;
                    }
                    if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                        related_object_types.insert(
                            self.pool
                                .resolve(self.objects[*related_index].type_name)
                                .to_owned(),
                        );
                    }
                }
            }

            for relationship in &object.relationships {
                if relationship.object_id == object.id {
                    continue;
                }
                if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                    related_object_types.insert(
                        self.pool
                            .resolve(self.objects[*related_index].type_name)
                            .to_owned(),
                    );
                }
            }

            for (name, value) in self.latest_attribute_values_at(object, i64::MAX) {
                let entry = attributes.entry(name).or_default();
                if attr_value_to_f64(value).is_some() {
                    entry.has_numeric = true;
                } else {
                    entry.categories.insert(self.attr_value_label(value));
                }
            }
        }

        let mut columns = Vec::new();
        columns.extend(
            activity_types
                .into_iter()
                .map(|event_type| FeatureColumn::Activity { event_type }),
        );
        columns.extend(
            related_object_types
                .into_iter()
                .map(|object_type| FeatureColumn::RelatedObjectType { object_type }),
        );
        for (name, collector) in attributes {
            if !collector.categories.is_empty() {
                if collector.categories.len() < 50 {
                    columns.extend(collector.categories.into_iter().map(|value| {
                        FeatureColumn::CategoricalAttribute {
                            name: name.clone(),
                            value,
                        }
                    }));
                }
            } else if collector.has_numeric {
                columns.push(FeatureColumn::NumericAttribute { name });
            }
        }

        FeatureEncoder { columns }
    }

    fn encode_lifecycle_windows(
        &self,
        object_indices: &[usize],
        window_size: usize,
        encoder: &FeatureEncoder,
    ) -> Vec<WindowEncoding> {
        let mut windows = Vec::new();
        for object_index in object_indices {
            let lifecycle = &self.objects[*object_index].lifecycle;
            if lifecycle.is_empty() {
                continue;
            }

            if lifecycle.len() <= window_size {
                let event_indices = lifecycle.clone();
                let end_time = event_indices
                    .last()
                    .map(|event_index| self.events[*event_index].time_ms)
                    .unwrap_or(i64::MAX);
                windows.push(WindowEncoding {
                    object_index: *object_index,
                    values: self.encode_feature_vector(
                        *object_index,
                        &event_indices,
                        end_time,
                        encoder,
                    ),
                    event_indices,
                });
                continue;
            }

            for start in 0..=lifecycle.len() - window_size {
                let event_indices = lifecycle[start..start + window_size].to_vec();
                let end_time = event_indices
                    .last()
                    .map(|event_index| self.events[*event_index].time_ms)
                    .unwrap_or(i64::MAX);
                windows.push(WindowEncoding {
                    object_index: *object_index,
                    values: self.encode_feature_vector(
                        *object_index,
                        &event_indices,
                        end_time,
                        encoder,
                    ),
                    event_indices,
                });
            }
        }
        windows
    }

    fn encode_feature_vector(
        &self,
        object_index: usize,
        event_indices: &[usize],
        attribute_time_ms: i64,
        encoder: &FeatureEncoder,
    ) -> Vec<f64> {
        let object = &self.objects[object_index];
        let mut activity_counts = BTreeMap::<String, f64>::new();
        let mut related_objects = BTreeMap::<String, BTreeSet<Symbol>>::new();

        for event_index in event_indices {
            let event = &self.events[*event_index];
            *activity_counts
                .entry(self.pool.resolve(event.type_name).to_owned())
                .or_default() += 1.0;
            for relationship in &event.relationships {
                if relationship.object_id == object.id {
                    continue;
                }
                if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                    related_objects
                        .entry(
                            self.pool
                                .resolve(self.objects[*related_index].type_name)
                                .to_owned(),
                        )
                        .or_default()
                        .insert(relationship.object_id);
                }
            }
        }

        for relationship in &object.relationships {
            if relationship.object_id == object.id {
                continue;
            }
            if let Some(related_index) = self.object_index.get(&relationship.object_id) {
                related_objects
                    .entry(
                        self.pool
                            .resolve(self.objects[*related_index].type_name)
                            .to_owned(),
                    )
                    .or_default()
                    .insert(relationship.object_id);
            }
        }

        let attribute_values = self.latest_attribute_values_at(object, attribute_time_ms);
        encoder
            .columns
            .iter()
            .map(|column| match column {
                FeatureColumn::Activity { event_type } => {
                    *activity_counts.get(event_type).unwrap_or(&0.0)
                }
                FeatureColumn::RelatedObjectType { object_type } => related_objects
                    .get(object_type)
                    .map(|objects| objects.len() as f64)
                    .unwrap_or(0.0),
                FeatureColumn::NumericAttribute { name } => attribute_values
                    .get(name)
                    .and_then(|value| attr_value_to_f64(value))
                    .unwrap_or(0.0),
                FeatureColumn::CategoricalAttribute { name, value } => attribute_values
                    .get(name)
                    .is_some_and(|candidate| self.attr_value_label(candidate) == *value)
                    .then_some(1.0)
                    .unwrap_or(0.0),
            })
            .collect()
    }

    fn latest_attribute_values_at<'a>(
        &'a self,
        object: &'a Object,
        time_ms: i64,
    ) -> BTreeMap<String, &'a AttrValue> {
        let mut latest = BTreeMap::<String, (i64, &'a AttrValue)>::new();
        for attribute in &object.attributes {
            if attribute.time_ms > time_ms {
                continue;
            }
            let name = self.pool.resolve(attribute.name).to_owned();
            if latest
                .get(&name)
                .is_none_or(|(existing_time, _)| attribute.time_ms >= *existing_time)
            {
                latest.insert(name, (attribute.time_ms, &attribute.value));
            }
        }
        latest
            .into_iter()
            .map(|(name, (_time, value))| (name, value))
            .collect()
    }

    fn attr_value_label(&self, value: &AttrValue) -> String {
        match value {
            AttrValue::String(symbol) => self.pool.resolve(*symbol).to_owned(),
            AttrValue::Time(ms) => ms.to_string(),
            AttrValue::Integer(value) => value.to_string(),
            AttrValue::Float(value) => value.to_string(),
            AttrValue::Boolean(value) => value.to_string(),
        }
    }

    fn summarize_som(
        &self,
        windows: &[WindowEncoding],
        points: &[(f64, f64)],
        som: &SomModel,
        color_metric: &ColorMetric,
    ) -> SomSummary {
        let mut cell_counts = vec![0usize; som.width * som.weights.len() / som.width];
        let mut pc_sums = vec![(0.0, 0.0); cell_counts.len()];
        let mut activity_counts = vec![BTreeMap::<String, usize>::new(); cell_counts.len()];
        let mut numeric_color_sums = vec![(0.0, 0usize); cell_counts.len()];
        let mut categorical_color_counts =
            vec![BTreeMap::<String, usize>::new(); cell_counts.len()];
        let mut transitions = BTreeMap::<(usize, usize, usize, usize), usize>::new();

        for ((window, (pc1, pc2)), (cell_x, cell_y)) in windows
            .iter()
            .zip(points.iter())
            .zip(som.assignments.iter())
        {
            let cell_index = cell_y * som.width + cell_x;
            cell_counts[cell_index] += 1;
            pc_sums[cell_index].0 += *pc1;
            pc_sums[cell_index].1 += *pc2;
            if let Some(activity) = self.dominant_window_activity(window) {
                *activity_counts[cell_index].entry(activity).or_default() += 1;
            }
            match color_metric {
                ColorMetric::WindowCount => {}
                ColorMetric::NumericAttribute(name) => {
                    if let Some(value) = self.window_attribute_value(window, name) {
                        if let Some(number) = attr_value_to_f64(value) {
                            numeric_color_sums[cell_index].0 += number;
                            numeric_color_sums[cell_index].1 += 1;
                        }
                    }
                }
                ColorMetric::CategoricalAttribute(name) => {
                    if let Some(value) = self.window_attribute_value(window, name) {
                        *categorical_color_counts[cell_index]
                            .entry(self.attr_value_label(value))
                            .or_default() += 1;
                    }
                }
            }
        }

        for pair in windows.windows(2).zip(som.assignments.windows(2)) {
            let (window_pair, cell_pair) = pair;
            let [left_window, right_window] = window_pair else {
                continue;
            };
            if left_window.object_index != right_window.object_index {
                continue;
            }
            let [(source_x, source_y), (target_x, target_y)] = cell_pair else {
                continue;
            };
            if source_x == target_x && source_y == target_y {
                continue;
            }
            *transitions
                .entry((*source_x, *source_y, *target_x, *target_y))
                .or_default() += 1;
        }

        let max_count = cell_counts.iter().copied().max().unwrap_or(0).max(1);
        let numeric_averages = numeric_color_sums
            .iter()
            .map(|(sum, count)| (*count > 0).then_some(sum / *count as f64))
            .collect::<Vec<_>>();
        let numeric_min = numeric_averages
            .iter()
            .filter_map(|value| *value)
            .fold(f64::INFINITY, f64::min);
        let numeric_max = numeric_averages
            .iter()
            .filter_map(|value| *value)
            .fold(f64::NEG_INFINITY, f64::max);
        let categorical_max = categorical_color_counts
            .iter()
            .filter_map(|counts| counts.values().max().copied())
            .max()
            .unwrap_or(1)
            .max(1);
        let height = som.weights.len() / som.width;
        let mut cells = Vec::with_capacity(som.weights.len());
        for y in 0..height {
            for x in 0..som.width {
                let index = y * som.width + x;
                let count = cell_counts[index];
                let dominant_activity = activity_counts[index]
                    .iter()
                    .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
                    .map(|(activity, _)| activity.clone());
                let (color_value, color_label, color_kind) = match color_metric {
                    ColorMetric::WindowCount => (
                        count as f64 / max_count as f64,
                        format!("{} windows", count),
                        "count".to_owned(),
                    ),
                    ColorMetric::NumericAttribute(name) => {
                        if let Some(average) = numeric_averages[index] {
                            let normalized = if (numeric_max - numeric_min).abs() <= f64::EPSILON {
                                1.0
                            } else {
                                (average - numeric_min) / (numeric_max - numeric_min)
                            };
                            (
                                normalized,
                                format!("avg {name}: {}", format_numeric_feature(average)),
                                "numeric".to_owned(),
                            )
                        } else {
                            (0.0, format!("avg {name}: n/a"), "numeric".to_owned())
                        }
                    }
                    ColorMetric::CategoricalAttribute(name) => {
                        let dominant_category =
                            categorical_color_counts[index]
                                .iter()
                                .max_by(|left, right| {
                                    left.1.cmp(right.1).then_with(|| right.0.cmp(left.0))
                                });
                        if let Some((category, category_count)) = dominant_category {
                            (
                                *category_count as f64 / categorical_max as f64,
                                format!("{name}: {category} ({category_count})"),
                                "categorical".to_owned(),
                            )
                        } else {
                            (0.0, format!("{name}: n/a"), "categorical".to_owned())
                        }
                    }
                };
                cells.push(SomCellSummary {
                    x,
                    y,
                    label: format!("S{}-{}", x + 1, y + 1),
                    count,
                    color_value: round_f64(color_value),
                    color_label,
                    color_kind,
                    avg_pc1: round_f64(if count == 0 {
                        som.weights[index].0
                    } else {
                        pc_sums[index].0 / count as f64
                    }),
                    avg_pc2: round_f64(if count == 0 {
                        som.weights[index].1
                    } else {
                        pc_sums[index].1 / count as f64
                    }),
                    dominant_activity,
                });
            }
        }

        let mut transitions = transitions
            .into_iter()
            .map(|((source_x, source_y, target_x, target_y), count)| {
                let distance = source_x.abs_diff(target_x) + source_y.abs_diff(target_y);
                SomTransitionSummary {
                    source_x,
                    source_y,
                    target_x,
                    target_y,
                    count,
                    distance,
                    nearby: distance <= 1,
                }
            })
            .collect::<Vec<_>>();
        transitions.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.distance.cmp(&right.distance))
                .then_with(|| left.source_y.cmp(&right.source_y))
                .then_with(|| left.source_x.cmp(&right.source_x))
        });

        SomSummary { cells, transitions }
    }

    fn window_attribute_value<'a>(
        &'a self,
        window: &WindowEncoding,
        attribute_name: &str,
    ) -> Option<&'a AttrValue> {
        let object = &self.objects[window.object_index];
        let event_time = window
            .event_indices
            .last()
            .map(|event_index| self.events[*event_index].time_ms)
            .unwrap_or(i64::MAX);
        self.latest_attribute_values_at(object, event_time)
            .remove(attribute_name)
    }

    fn dominant_window_activity(&self, window: &WindowEncoding) -> Option<String> {
        let mut counts = BTreeMap::<String, usize>::new();
        for event_index in &window.event_indices {
            *counts
                .entry(
                    self.pool
                        .resolve(self.events[*event_index].type_name)
                        .to_owned(),
                )
                .or_default() += 1;
        }
        counts
            .into_iter()
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
            .map(|(activity, _)| activity)
    }

    fn directly_follows_graph_json(&self, object_type: &str) -> OcelResult<String> {
        self.object_type_symbol(object_type).ok_or_else(|| {
            OcelError::new(format!("unknown leading object type '{object_type}'"))
        })?;
        let mut graph = GraphAccumulator::new(
            format!("Directly-Follows Graph: {object_type}"),
            format!("Flattened over {object_type} object lifecycles"),
        );
        for object in &self.objects {
            let current_type = self.pool.resolve(object.type_name);
            if current_type == object_type {
                self.accumulate_directly_follows_for_object(&mut graph, object, current_type);
            }
        }
        graph.into_layout()
    }

    fn object_centric_directly_follows_graph_json(&self) -> OcelResult<String> {
        self.object_centric_directly_follows_graph_json_with_filter(&GraphFilterRequest::default())
    }

    fn object_centric_directly_follows_graph_json_with_filter(
        &self,
        request: &GraphFilterRequest,
    ) -> OcelResult<String> {
        let mut graph = GraphAccumulator::new(
            "Object-Centric Directly-Follows Graph".to_owned(),
            "Flattened over selected object types with typed lifecycle edges".to_owned(),
        );
        let selected_object_types = request.object_types.as_ref().map(|object_types| {
            object_types
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        });
        for object in &self.objects {
            let object_type = self.pool.resolve(object.type_name);
            if selected_object_types
                .as_ref()
                .is_some_and(|selected| !selected.contains(object_type))
            {
                continue;
            }
            self.accumulate_directly_follows_for_object(&mut graph, object, object_type);
        }
        graph.into_filtered_layout(request.layout_filter())
    }

    fn state_aware_ocdfg_json(&self) -> OcelResult<String> {
        self.state_aware_ocdfg_json_with_filter(&GraphFilterRequest::default())
    }

    fn state_aware_ocdfg_json_with_filter(
        &self,
        request: &GraphFilterRequest,
    ) -> OcelResult<String> {
        let state_attribute = self.symbol_for_value("state").ok_or_else(|| {
            OcelError::new("event state attribute is missing; apply a state query first")
        })?;
        let mut graph = GraphAccumulator::new(
            "State-Aware Object-Centric Directly-Follows Graph".to_owned(),
            "State-enriched lifecycles collated across object types".to_owned(),
        );
        let selected_object_types = request.object_types.as_ref().map(|object_types| {
            object_types
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        });

        for object in &self.objects {
            let object_type = self.pool.resolve(object.type_name);
            if selected_object_types
                .as_ref()
                .is_some_and(|selected| !selected.contains(object_type))
            {
                continue;
            }
            self.accumulate_state_aware_directly_follows_for_object(
                &mut graph,
                object,
                object_type,
                state_attribute,
            );
        }

        graph.into_filtered_layout(request.layout_filter())
    }

    fn accumulate_directly_follows_for_object(
        &self,
        graph: &mut GraphAccumulator,
        object: &Object,
        object_type: &str,
    ) {
        if object.lifecycle.is_empty() {
            return;
        }

        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            object.lifecycle.len() as f64 + 1.0,
            1,
        );

        for (position, event_index) in object.lifecycle.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            graph.add_node(event_type, "activity", position as f64 + 1.0, 1);
        }

        if let Some(first_index) = object.lifecycle.first() {
            let first = self.pool.resolve(self.events[*first_index].type_name);
            graph.add_edge(&start, first, object_type, 1);
        }

        for pair in object.lifecycle.windows(2) {
            let [source_index, target_index] = pair else {
                continue;
            };
            let source = self.pool.resolve(self.events[*source_index].type_name);
            let target = self.pool.resolve(self.events[*target_index].type_name);
            graph.add_edge(source, target, object_type, 1);
        }

        if let Some(last_index) = object.lifecycle.last() {
            let last = self.pool.resolve(self.events[*last_index].type_name);
            graph.add_edge(last, &end, object_type, 1);
        }
    }

    fn accumulate_state_aware_directly_follows_for_object(
        &self,
        graph: &mut GraphAccumulator,
        object: &Object,
        object_type: &str,
        state_attribute: Symbol,
    ) {
        let stateful_lifecycle = object
            .lifecycle
            .iter()
            .filter_map(|event_index| {
                self.event_state(&self.events[*event_index], state_attribute)
                    .map(|state| (*event_index, state.to_owned()))
            })
            .collect::<Vec<_>>();

        if stateful_lifecycle.is_empty() {
            return;
        }

        let start = object_boundary_label("START", object_type);
        let end = object_boundary_label("END", object_type);
        graph.add_object_boundary_node(&start, "object-start", object_type, 0.0, 1);
        graph.add_object_boundary_node(
            &end,
            "object-end",
            object_type,
            stateful_lifecycle.len() as f64 * 2.0,
            1,
        );

        for (position, (event_index, state)) in stateful_lifecycle.iter().enumerate() {
            let event_type = self.pool.resolve(self.events[*event_index].type_name);
            let label = format!("{event_type} [{state}]");
            graph.add_node(&label, "state-activity", position as f64 * 2.0 + 1.0, 1);
        }

        if let Some((first_index, first_state)) = stateful_lifecycle.first() {
            let first_event_type = self.pool.resolve(self.events[*first_index].type_name);
            let first = format!("{first_event_type} [{first_state}]");
            graph.add_edge(&start, &first, object_type, 1);
        }

        for (position, pair) in stateful_lifecycle.windows(2).enumerate() {
            let [(source_index, source_state), (target_index, target_state)] = pair else {
                continue;
            };
            let source_event_type = self.pool.resolve(self.events[*source_index].type_name);
            let target_event_type = self.pool.resolve(self.events[*target_index].type_name);
            let source = format!("{source_event_type} [{source_state}]");
            let target = format!("{target_event_type} [{target_state}]");

            if source_state == target_state {
                graph.add_edge(&source, &target, object_type, 1);
                continue;
            }

            let change = format!("CHANGE {source_state} -> {target_state}");
            graph.add_node(&change, "state-change", position as f64 * 2.0 + 2.0, 1);
            graph.add_edge(&source, &change, object_type, 1);
            graph.add_edge(&change, &target, object_type, 1);
        }

        if let Some((last_index, last_state)) = stateful_lifecycle.last() {
            let last_event_type = self.pool.resolve(self.events[*last_index].type_name);
            let last = format!("{last_event_type} [{last_state}]");
            graph.add_edge(&last, &end, object_type, 1);
        }
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
            if self
                .state_leading_object_type
                .is_some_and(|leading_type| object.type_name != leading_type)
            {
                continue;
            }

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
        let leading_type_symbol = self.object_type_symbol(&query.leading_object_type)?;
        let related_objects = event
            .relationships
            .iter()
            .filter_map(|relationship| self.object_index.get(&relationship.object_id).copied())
            .filter(|object_index| self.objects[*object_index].type_name == leading_type_symbol)
            .collect::<Vec<_>>();
        if related_objects.is_empty() {
            return None;
        }

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

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionResult {
    object_type: String,
    window_size: usize,
    som_width: usize,
    som_height: usize,
    color_attribute: String,
    color_attributes: Vec<StateDetectionColorOption>,
    object_count: usize,
    feature_count: usize,
    window_count: usize,
    feature_columns: Vec<String>,
    table_preview: Vec<FeaturePreviewRow>,
    pca: PcaSummary,
    som: SomSummary,
    windows: Vec<StateWindowProjection>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionColorOption {
    id: String,
    label: String,
    kind: &'static str,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct FeaturePreviewRow {
    object_id: String,
    values: Vec<f64>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct PcaSummary {
    pc1_variance: f64,
    pc2_variance: f64,
    pc1_explained_ratio: f64,
    pc2_explained_ratio: f64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct SomSummary {
    cells: Vec<SomCellSummary>,
    transitions: Vec<SomTransitionSummary>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct SomCellSummary {
    x: usize,
    y: usize,
    label: String,
    count: usize,
    color_value: f64,
    color_label: String,
    color_kind: String,
    avg_pc1: f64,
    avg_pc2: f64,
    dominant_activity: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct SomTransitionSummary {
    source_x: usize,
    source_y: usize,
    target_x: usize,
    target_y: usize,
    count: usize,
    distance: usize,
    nearby: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateWindowProjection {
    object_id: String,
    start_event: String,
    end_event: String,
    pc1: f64,
    pc2: f64,
    cell_x: usize,
    cell_y: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionCellDetail {
    cell: SomCellSummary,
    dfg: LayoutGraph,
    entering_dfg: LayoutGraph,
    exiting_dfg: LayoutGraph,
    entering_window_count: usize,
    exiting_window_count: usize,
    entering_windows: Vec<StateDetectionBoundaryWindow>,
    exiting_windows: Vec<StateDetectionBoundaryWindow>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct StateDetectionBoundaryWindow {
    object_id: String,
    start_event: String,
    end_event: String,
    source_cell: String,
    target_cell: String,
    pc1: f64,
    pc2: f64,
    activities: Vec<String>,
}

struct FeatureTableData {
    columns: Vec<String>,
    rows: Vec<FeatureRow>,
}

struct FeatureRow {
    object_id: String,
    values: Vec<f64>,
}

#[derive(Clone)]
enum FeatureColumn {
    Activity { event_type: String },
    RelatedObjectType { object_type: String },
    NumericAttribute { name: String },
    CategoricalAttribute { name: String, value: String },
}

impl FeatureColumn {
    fn label(&self) -> String {
        match self {
            Self::Activity { event_type } => format!("activity.{event_type}"),
            Self::RelatedObjectType { object_type } => format!("related_objects.{object_type}"),
            Self::NumericAttribute { name } => format!("attribute.{name}"),
            Self::CategoricalAttribute { name, value } => {
                format!("attribute.{name}={value}")
            }
        }
    }
}

#[derive(Default)]
struct AttributeFeatureCollector {
    has_numeric: bool,
    categories: BTreeSet<String>,
}

struct FeatureEncoder {
    columns: Vec<FeatureColumn>,
}

struct WindowEncoding {
    object_index: usize,
    event_indices: Vec<usize>,
    values: Vec<f64>,
}

struct PcaProjection {
    points: Vec<(f64, f64)>,
    pc1_variance: f64,
    pc2_variance: f64,
    pc1_explained_ratio: f64,
    pc2_explained_ratio: f64,
}

struct SomModel {
    width: usize,
    assignments: Vec<(usize, usize)>,
    weights: Vec<(f64, f64)>,
}

struct StateDetectionRun {
    object_indices: Vec<usize>,
    encoder: FeatureEncoder,
    feature_table: FeatureTableData,
    windows: Vec<WindowEncoding>,
    pca: PcaProjection,
    som: SomModel,
    color_metric: ColorMetric,
    color_options: Vec<StateDetectionColorOption>,
}

#[derive(Clone)]
enum ColorMetric {
    WindowCount,
    NumericAttribute(String),
    CategoricalAttribute(String),
}

impl ColorMetric {
    fn id(&self) -> String {
        match self {
            Self::WindowCount => "__window_count".to_owned(),
            Self::NumericAttribute(name) | Self::CategoricalAttribute(name) => {
                format!("attribute::{name}")
            }
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutGraph {
    title: String,
    subtitle: String,
    width: f64,
    height: f64,
    nodes: Vec<LayoutNode>,
    edges: Vec<LayoutEdge>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutNode {
    id: String,
    label: String,
    kind: String,
    shape: String,
    color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    object_type: Option<String>,
    count: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    lines: Vec<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutEdge {
    id: String,
    source: String,
    target: String,
    kind: String,
    path: String,
    label: String,
    title: String,
    weight: usize,
    object_type: String,
    color: String,
    directed: bool,
    points: Vec<LayoutPoint>,
    label_x: f64,
    label_y: f64,
    object_types: Vec<WeightedObjectType>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct LayoutPoint {
    x: f64,
    y: f64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
struct WeightedObjectType {
    object_type: String,
    weight: usize,
}

struct GraphAccumulator {
    title: String,
    subtitle: String,
    nodes: BTreeMap<String, GraphNodeAccumulator>,
    edges: BTreeMap<(String, String, String), GraphEdgeAccumulator>,
    object_type_colors: BTreeMap<String, String>,
}

struct GraphNodeAccumulator {
    label: String,
    kind: String,
    shape: String,
    color: String,
    object_type: Option<String>,
    count: usize,
    order_sum: f64,
    order_count: usize,
}

struct GraphEdgeAccumulator {
    source: String,
    target: String,
    object_type: String,
    color: String,
    weight: usize,
}

impl GraphAccumulator {
    fn new(title: String, subtitle: String) -> Self {
        Self {
            title,
            subtitle,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            object_type_colors: BTreeMap::new(),
        }
    }

    fn add_node(&mut self, label: &str, kind: &str, order: f64, count: usize) {
        self.add_node_with_style(label, kind, "rect", "#42635c", None, order, count);
    }

    fn add_object_boundary_node(
        &mut self,
        label: &str,
        kind: &str,
        object_type: &str,
        order: f64,
        count: usize,
    ) {
        let color = self.color_for_object_type(object_type);
        self.add_node_with_style(
            label,
            kind,
            "ellipse",
            &color,
            Some(object_type),
            order,
            count,
        );
    }

    fn add_node_with_style(
        &mut self,
        label: &str,
        kind: &str,
        shape: &str,
        color: &str,
        object_type: Option<&str>,
        order: f64,
        count: usize,
    ) {
        let entry = self
            .nodes
            .entry(label.to_owned())
            .or_insert_with(|| GraphNodeAccumulator {
                label: label.to_owned(),
                kind: kind.to_owned(),
                shape: shape.to_owned(),
                color: color.to_owned(),
                object_type: object_type.map(str::to_owned),
                count: 0,
                order_sum: 0.0,
                order_count: 0,
            });
        entry.count += count;
        entry.order_sum += order * count as f64;
        entry.order_count += count;
        if entry.kind != "state-change" && kind == "state-change" {
            entry.kind = kind.to_owned();
            entry.shape = shape.to_owned();
            entry.color = color.to_owned();
            entry.object_type = object_type.map(str::to_owned);
        }
    }

    fn add_edge(&mut self, source: &str, target: &str, object_type: &str, weight: usize) {
        let color = self.color_for_object_type(object_type);
        let entry = self
            .edges
            .entry((source.to_owned(), target.to_owned(), object_type.to_owned()))
            .or_insert_with(|| GraphEdgeAccumulator {
                source: source.to_owned(),
                target: target.to_owned(),
                object_type: object_type.to_owned(),
                color,
                weight: 0,
            });
        entry.weight += weight;
    }

    fn color_for_object_type(&mut self, object_type: &str) -> String {
        if let Some(color) = self.object_type_colors.get(object_type) {
            return color.clone();
        }

        let index = self.object_type_colors.len();
        let color = object_type_graph_color(index);
        self.object_type_colors
            .insert(object_type.to_owned(), color.clone());
        color
    }

    fn into_layout(self) -> OcelResult<String> {
        self.into_filtered_layout(GraphLayoutFilter::default())
    }

    fn into_filtered_layout(mut self, filter: GraphLayoutFilter) -> OcelResult<String> {
        self.apply_layout_filter(filter);
        let graph = layout_accumulated_graph(self);
        serde_json::to_string(&graph)
            .map_err(|err| OcelError::new(format!("could not serialize graph layout: {err}")))
    }

    fn apply_layout_filter(&mut self, filter: GraphLayoutFilter) {
        if filter.min_activity_frequency > 0 {
            self.nodes.retain(|_, node| {
                node.kind == "object-start"
                    || node.kind == "object-end"
                    || node.count >= filter.min_activity_frequency
            });
        }

        self.edges.retain(|_, edge| {
            edge.weight >= filter.min_path_frequency
                && self.nodes.contains_key(&edge.source)
                && self.nodes.contains_key(&edge.target)
        });

        let mut connected_boundary_nodes = BTreeSet::<String>::new();
        for edge in self.edges.values() {
            connected_boundary_nodes.insert(edge.source.clone());
            connected_boundary_nodes.insert(edge.target.clone());
        }

        self.nodes.retain(|_, node| {
            if node.kind == "object-start" || node.kind == "object-end" {
                connected_boundary_nodes.contains(&node.label)
            } else {
                true
            }
        });
    }
}

fn object_boundary_label(boundary: &str, object_type: &str) -> String {
    format!("{boundary}\n{object_type}")
}

fn object_type_graph_color(index: usize) -> String {
    let hue = (214 + index * 137) % 360;
    format!("hsl({hue} 68% 38%)")
}

fn layout_accumulated_graph(graph: GraphAccumulator) -> LayoutGraph {
    let mut node_items = graph
        .nodes
        .into_values()
        .map(|node| {
            let average_order = if node.order_count == 0 {
                0.0
            } else {
                node.order_sum / node.order_count as f64
            };
            (average_order, node)
        })
        .collect::<Vec<_>>();

    node_items.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.1.count.cmp(&left.1.count))
            .then_with(|| left.1.label.cmp(&right.1.label))
    });

    let mut layers = BTreeMap::<i32, Vec<(f64, GraphNodeAccumulator)>>::new();
    for (average_order, node) in node_items {
        layers
            .entry((average_order * 2.0).round() as i32)
            .or_default()
            .push((average_order, node));
    }
    let mut layers = layers.into_iter().collect::<Vec<_>>();
    let max_layer_rows = layers
        .iter()
        .map(|(_, nodes)| nodes.len())
        .max()
        .unwrap_or(1);

    let mut nodes = Vec::new();
    let mut node_positions = HashMap::<String, (String, f64, f64, f64, f64)>::new();
    let node_gap_x = 340.0;
    let node_gap_y = 158.0;
    let margin_x = 76.0;
    let margin_y = 76.0;
    let mut max_rows = 1usize;

    for (layer_index, (_layer, layer_nodes)) in layers.iter_mut().enumerate() {
        layer_nodes.sort_by(|left, right| {
            right
                .1
                .count
                .cmp(&left.1.count)
                .then_with(|| left.1.label.cmp(&right.1.label))
        });
        max_rows = max_rows.max(layer_nodes.len());
        let layer_offset_y =
            (max_layer_rows.saturating_sub(layer_nodes.len()) as f64 * node_gap_y) / 2.0;
        let wave_offset_y = if layer_index % 2 == 1 && layer_nodes.len() > 1 {
            node_gap_y * 0.12
        } else {
            0.0
        };
        for (row_index, (_average_order, node)) in layer_nodes.drain(..).enumerate() {
            let max_line_length = if node.shape == "ellipse" { 18 } else { 24 };
            let lines = wrap_label(&node.label, max_line_length, 4);
            let width = match node.kind.as_str() {
                "state-change" => 230.0,
                "object-start" | "object-end" => 168.0,
                _ => 215.0,
            };
            let height = (62.0 + (lines.len().saturating_sub(1) as f64 * 14.0)).max(72.0);
            let x = margin_x + layer_index as f64 * node_gap_x;
            let y = margin_y + layer_offset_y + wave_offset_y + row_index as f64 * node_gap_y;
            let id = format!("n{}", nodes.len() + 1);
            node_positions.insert(node.label.clone(), (id.clone(), x, y, width, height));
            nodes.push(LayoutNode {
                id,
                label: node.label,
                kind: node.kind,
                shape: node.shape,
                color: node.color,
                object_type: node.object_type,
                count: node.count,
                x,
                y,
                width,
                height,
                lines,
            });
        }
    }

    let width = nodes
        .iter()
        .map(|node| node.x + node.width + margin_x)
        .fold(720.0, f64::max);
    let height = (margin_y * 2.0 + max_rows as f64 * node_gap_y).max(320.0);

    let edge_items = graph.edges.into_values().collect::<Vec<_>>();
    let mut parallel_totals = BTreeMap::<(String, String), usize>::new();
    for edge in &edge_items {
        *parallel_totals
            .entry((edge.source.clone(), edge.target.clone()))
            .or_default() += 1;
    }
    let mut parallel_seen = BTreeMap::<(String, String), usize>::new();

    let mut edges = edge_items
        .into_iter()
        .filter_map(|edge| {
            let (source_id, source_x, source_y, source_width, source_height) =
                node_positions.get(&edge.source)?.clone();
            let (target_id, target_x, target_y, target_width, target_height) =
                node_positions.get(&edge.target)?.clone();
            let parallel_key = (edge.source.clone(), edge.target.clone());
            let parallel_total = *parallel_totals.get(&parallel_key).unwrap_or(&1);
            let parallel_index = parallel_seen.entry(parallel_key).or_default();
            let lane_offset = parallel_edge_offset(*parallel_index, parallel_total);
            *parallel_index += 1;
            let points = routed_edge_points(
                source_x,
                source_y,
                source_width,
                source_height,
                target_x,
                target_y,
                target_width,
                target_height,
                source_id == target_id,
                lane_offset,
            );
            let (label_x, label_y) = edge_label_position(&points);
            let path = curved_edge_path(&points);
            let object_type = edge.object_type;
            let object_types = vec![WeightedObjectType {
                object_type: object_type.clone(),
                weight: edge.weight,
            }];
            let title = format!("{object_type}: {}", edge.weight);
            Some(LayoutEdge {
                id: String::new(),
                source: source_id,
                target: target_id,
                kind: "df".to_owned(),
                path,
                label: edge.weight.to_string(),
                title,
                weight: edge.weight,
                object_type,
                color: edge.color,
                directed: true,
                points,
                label_x,
                label_y,
                object_types,
            })
        })
        .collect::<Vec<_>>();

    edges.sort_by(|left, right| {
        right
            .weight
            .cmp(&left.weight)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.object_type.cmp(&right.object_type))
    });
    for (index, edge) in edges.iter_mut().enumerate() {
        edge.id = format!("e{}", index + 1);
    }

    LayoutGraph {
        title: graph.title,
        subtitle: graph.subtitle,
        width,
        height,
        nodes,
        edges,
    }
}

fn parallel_edge_offset(index: usize, total: usize) -> f64 {
    if total <= 1 {
        return 0.0;
    }

    (index as f64 - (total as f64 - 1.0) / 2.0) * 30.0
}

fn routed_edge_points(
    source_x: f64,
    source_y: f64,
    source_width: f64,
    source_height: f64,
    target_x: f64,
    target_y: f64,
    target_width: f64,
    target_height: f64,
    self_loop: bool,
    lane_offset: f64,
) -> Vec<LayoutPoint> {
    let source_mid_y = source_y + source_height / 2.0;
    let target_mid_y = target_y + target_height / 2.0;
    if self_loop {
        let x1 = source_x + source_width;
        let y1 = source_mid_y;
        return vec![
            LayoutPoint { x: x1, y: y1 },
            LayoutPoint {
                x: x1 + 44.0,
                y: y1 - 42.0 + lane_offset,
            },
            LayoutPoint {
                x: source_x + source_width / 2.0,
                y: source_y - 28.0 + lane_offset,
            },
            LayoutPoint {
                x: source_x,
                y: y1 - 16.0,
            },
        ];
    }

    let starts_before_target = source_x + source_width <= target_x;
    if starts_before_target {
        let x1 = source_x + source_width;
        let x2 = target_x;
        let mid_x = (x1 + x2) / 2.0;
        vec![
            LayoutPoint {
                x: x1,
                y: source_mid_y,
            },
            LayoutPoint {
                x: mid_x,
                y: source_mid_y + lane_offset,
            },
            LayoutPoint {
                x: mid_x,
                y: target_mid_y + lane_offset,
            },
            LayoutPoint {
                x: x2,
                y: target_mid_y,
            },
        ]
    } else {
        let x1 = source_x;
        let x2 = target_x + target_width;
        let mid_x = (x1 + x2) / 2.0;
        vec![
            LayoutPoint {
                x: x1,
                y: source_mid_y,
            },
            LayoutPoint {
                x: mid_x,
                y: source_mid_y + lane_offset,
            },
            LayoutPoint {
                x: mid_x,
                y: target_mid_y + lane_offset,
            },
            LayoutPoint {
                x: x2,
                y: target_mid_y,
            },
        ]
    }
}

fn curved_edge_path(points: &[LayoutPoint]) -> String {
    match points {
        [] => String::new(),
        [start] => format!("M {:.1} {:.1}", start.x, start.y),
        [start, end] => format!(
            "M {:.1} {:.1} L {:.1} {:.1}",
            start.x, start.y, end.x, end.y
        ),
        [start, control, end] => format!(
            "M {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}",
            start.x, start.y, control.x, control.y, end.x, end.y
        ),
        [start, control_a, control_b, end, ..] => format!(
            "M {:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
            start.x, start.y, control_a.x, control_a.y, control_b.x, control_b.y, end.x, end.y
        ),
    }
}

fn edge_label_position(points: &[LayoutPoint]) -> (f64, f64) {
    if points.is_empty() {
        return (0.0, 0.0);
    }
    let middle = points.len() / 2;
    if points.len() % 2 == 0 {
        (
            (points[middle - 1].x + points[middle].x) / 2.0,
            (points[middle - 1].y + points[middle].y) / 2.0 - 6.0,
        )
    } else {
        (points[middle].x, points[middle].y - 6.0)
    }
}

fn wrap_label(label: &str, max_line_length: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for chunk in label.split('\n') {
        let mut current = String::new();
        for word in chunk.split_whitespace() {
            for part in split_label_word(word, max_line_length) {
                let candidate = if current.is_empty() {
                    part.clone()
                } else {
                    format!("{current} {part}")
                };
                if candidate.len() <= max_line_length {
                    current = candidate;
                } else {
                    if !current.is_empty() {
                        lines.push(current);
                    }
                    current = part;
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(label.to_owned());
    }
    if lines.len() <= max_lines {
        return lines;
    }
    let mut trimmed = lines.into_iter().take(max_lines).collect::<Vec<_>>();
    if let Some(last) = trimmed.last_mut() {
        last.truncate(max_line_length.saturating_sub(3));
        last.push_str("...");
    }
    trimmed
}

fn split_label_word(word: &str, max_line_length: usize) -> Vec<String> {
    if word.len() <= max_line_length {
        return vec![word.to_owned()];
    }
    let mut parts = Vec::new();
    let mut start = 0usize;
    while start < word.len() {
        let mut end = (start + max_line_length).min(word.len());
        while !word.is_char_boundary(end) {
            end -= 1;
        }
        parts.push(word[start..end].to_owned());
        start = end;
    }
    parts
}

fn feature_table_to_csv(table: &FeatureTableData) -> String {
    let mut output = String::new();
    output.push_str("object_id");
    for column in &table.columns {
        output.push(',');
        output.push_str(&csv_escape(column));
    }
    output.push('\n');

    for row in &table.rows {
        output.push_str(&csv_escape(&row.object_id));
        for value in &row.values {
            output.push(',');
            output.push_str(&format_numeric_feature(*value));
        }
        output.push('\n');
    }

    output
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn format_numeric_feature(value: f64) -> String {
    if value.is_finite() && value.fract().abs() < 0.000_000_1 {
        (value as i64).to_string()
    } else {
        round_f64(value).to_string()
    }
}

fn cell_label(x: usize, y: usize) -> String {
    format!("S{}-{}", x + 1, y + 1)
}

fn attr_value_to_f64(value: &AttrValue) -> Option<f64> {
    match value {
        AttrValue::String(_) => None,
        AttrValue::Time(value) => Some(*value as f64),
        AttrValue::Integer(value) => Some(*value as f64),
        AttrValue::Float(value) if value.is_finite() => Some(*value),
        AttrValue::Float(_) => None,
        AttrValue::Boolean(value) => Some(usize::from(*value) as f64),
    }
}

fn pca_project(rows: &[Vec<f64>]) -> PcaProjection {
    let row_count = rows.len();
    let column_count = rows.first().map(Vec::len).unwrap_or_default();
    if row_count == 0 || column_count == 0 {
        return PcaProjection {
            points: Vec::new(),
            pc1_variance: 0.0,
            pc2_variance: 0.0,
            pc1_explained_ratio: 0.0,
            pc2_explained_ratio: 0.0,
        };
    }

    let mut means = vec![0.0; column_count];
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            means[index] += *value;
        }
    }
    for mean in &mut means {
        *mean /= row_count as f64;
    }

    let mut std_devs = vec![0.0; column_count];
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            let centered = value - means[index];
            std_devs[index] += centered * centered;
        }
    }
    for std_dev in &mut std_devs {
        *std_dev = (*std_dev / row_count.max(1) as f64).sqrt();
        if *std_dev <= f64::EPSILON {
            *std_dev = 1.0;
        }
    }

    let standardized = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(index, value)| (value - means[index]) / std_devs[index])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let divisor = row_count.saturating_sub(1).max(1) as f64;
    let mut covariance = vec![vec![0.0; column_count]; column_count];
    for row in &standardized {
        for left in 0..column_count {
            for right in left..column_count {
                covariance[left][right] += row[left] * row[right] / divisor;
            }
        }
    }
    for left in 0..column_count {
        for right in 0..left {
            covariance[left][right] = covariance[right][left];
        }
    }

    let total_variance = covariance
        .iter()
        .enumerate()
        .map(|(index, row)| row[index])
        .sum::<f64>()
        .max(0.0);
    let pc1 = power_iteration(&covariance, 80);
    let pc1_variance = rayleigh_quotient(&covariance, &pc1).max(0.0);
    let mut deflated = covariance.clone();
    for row in 0..column_count {
        for column in 0..column_count {
            deflated[row][column] -= pc1_variance * pc1[row] * pc1[column];
        }
    }
    let pc2 = if column_count > 1 {
        power_iteration(&deflated, 80)
    } else {
        vec![0.0; column_count]
    };
    let pc2_variance = if column_count > 1 {
        rayleigh_quotient(&covariance, &pc2).max(0.0)
    } else {
        0.0
    };

    let points = standardized
        .iter()
        .map(|row| (dot(row, &pc1), dot(row, &pc2)))
        .collect();

    PcaProjection {
        points,
        pc1_variance,
        pc2_variance,
        pc1_explained_ratio: if total_variance > f64::EPSILON {
            pc1_variance / total_variance
        } else {
            0.0
        },
        pc2_explained_ratio: if total_variance > f64::EPSILON {
            pc2_variance / total_variance
        } else {
            0.0
        },
    }
}

fn power_iteration(matrix: &[Vec<f64>], iterations: usize) -> Vec<f64> {
    let size = matrix.len();
    if size == 0 {
        return Vec::new();
    }

    let mut vector = (0..size)
        .map(|index| (index + 1) as f64 / size as f64)
        .collect::<Vec<_>>();
    normalize_vector(&mut vector);
    for _ in 0..iterations {
        let mut next = vec![0.0; size];
        for row in 0..size {
            for (column, value) in vector.iter().enumerate() {
                next[row] += matrix[row][column] * value;
            }
        }
        if vector_norm(&next) <= f64::EPSILON {
            break;
        }
        normalize_vector(&mut next);
        vector = next;
    }
    vector
}

fn rayleigh_quotient(matrix: &[Vec<f64>], vector: &[f64]) -> f64 {
    if matrix.is_empty() || vector.is_empty() {
        return 0.0;
    }
    let multiplied = matrix
        .iter()
        .map(|row| dot(row, vector))
        .collect::<Vec<_>>();
    dot(vector, &multiplied)
}

fn normalize_vector(vector: &mut [f64]) {
    let norm = vector_norm(vector);
    if norm <= f64::EPSILON {
        return;
    }
    for value in vector {
        *value /= norm;
    }
}

fn vector_norm(vector: &[f64]) -> f64 {
    vector.iter().map(|value| value * value).sum::<f64>().sqrt()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}

fn default_som_dimensions(
    point_count: usize,
    requested_width: Option<usize>,
    requested_height: Option<usize>,
) -> (usize, usize) {
    let fallback = ((point_count as f64).sqrt().ceil() as usize).clamp(3, 8);
    (
        requested_width.unwrap_or(fallback).clamp(2, 12),
        requested_height.unwrap_or(fallback).clamp(2, 12),
    )
}

fn train_som(points: &[(f64, f64)], width: usize, height: usize, epochs: usize) -> SomModel {
    let (min_x, max_x, min_y, max_y) = point_bounds(points);
    let mut weights = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let fx = if width <= 1 {
                0.5
            } else {
                x as f64 / (width - 1) as f64
            };
            let fy = if height <= 1 {
                0.5
            } else {
                y as f64 / (height - 1) as f64
            };
            weights.push((min_x + (max_x - min_x) * fx, min_y + (max_y - min_y) * fy));
        }
    }

    let max_radius = (width.max(height) as f64 / 2.0).max(1.0);
    for epoch in 0..epochs {
        let progress = if epochs <= 1 {
            1.0
        } else {
            epoch as f64 / (epochs - 1) as f64
        };
        let learning_rate = 0.5 * (1.0 - progress) + 0.05 * progress;
        let radius = max_radius * (1.0 - progress) + 0.75 * progress;
        let radius_sq = (radius * radius).max(0.01);
        for point in points {
            let (bmu_x, bmu_y) = best_matching_unit(point, &weights, width);
            for y in 0..height {
                for x in 0..width {
                    let grid_distance_sq =
                        (x.abs_diff(bmu_x).pow(2) + y.abs_diff(bmu_y).pow(2)) as f64;
                    let influence = (-grid_distance_sq / (2.0 * radius_sq)).exp();
                    let index = y * width + x;
                    weights[index].0 += learning_rate * influence * (point.0 - weights[index].0);
                    weights[index].1 += learning_rate * influence * (point.1 - weights[index].1);
                }
            }
        }
    }

    let assignments = points
        .iter()
        .map(|point| best_matching_unit(point, &weights, width))
        .collect();

    SomModel {
        width,
        assignments,
        weights,
    }
}

fn point_bounds(points: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in points {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }
    if !min_x.is_finite() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    if (max_x - min_x).abs() <= f64::EPSILON {
        min_x -= 0.5;
        max_x += 0.5;
    }
    if (max_y - min_y).abs() <= f64::EPSILON {
        min_y -= 0.5;
        max_y += 0.5;
    }
    (min_x, max_x, min_y, max_y)
}

fn best_matching_unit(point: &(f64, f64), weights: &[(f64, f64)], width: usize) -> (usize, usize) {
    let mut best_index = 0usize;
    let mut best_distance = f64::INFINITY;
    for (index, weight) in weights.iter().enumerate() {
        let distance = squared_distance(*point, *weight);
        if distance < best_distance {
            best_distance = distance;
            best_index = index;
        }
    }
    (best_index % width, best_index / width)
}

fn squared_distance(left: (f64, f64), right: (f64, f64)) -> f64 {
    let dx = left.0 - right.0;
    let dy = left.1 - right.1;
    dx * dx + dy * dy
}

fn round_f64(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        0.0
    }
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
    original_log: CompactOcelLog,
    log: CompactOcelLog,
    current_filter: OcelFilterRequest,
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
        let current_filter = OcelFilterRequest::all_for(&log);
        Ok(Self {
            original_log: log.clone(),
            log,
            current_filter,
        })
    }

    /// Imports an OCEL 2.0 JSON/XML file from raw bytes.
    ///
    /// The byte input may be plain UTF-8 JSON/XML or a gzip-compressed `.gz`
    /// file containing UTF-8 JSON/XML.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(input: &[u8], format_hint: Option<String>) -> Result<OcelDocument, JsValue> {
        let log = CompactOcelLog::from_bytes(input, format_hint.as_deref())?;
        let current_filter = OcelFilterRequest::all_for(&log);
        Ok(Self {
            original_log: log.clone(),
            log,
            current_filter,
        })
    }

    /// Returns summary counts as a JSON string.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> String {
        self.log.summary_json()
    }

    /// Returns summary counts for the unfiltered imported document as a JSON string.
    #[wasm_bindgen(js_name = originalSummaryJson)]
    pub fn original_summary_json(&self) -> String {
        self.original_log.summary_json()
    }

    /// Returns available event and object types for filtering as a JSON string.
    #[wasm_bindgen(js_name = filterOptionsJson)]
    pub fn filter_options_json(&self) -> String {
        serde_json::to_string(&self.original_log.filter_options())
            .expect("filter option serialization cannot fail")
    }

    /// Rebuilds the active log from the original, possibly state-enriched, log.
    #[wasm_bindgen(js_name = applyFilter)]
    pub fn apply_filter(&mut self, filter_json: &str) -> Result<String, JsValue> {
        let filter = serde_json::from_str::<OcelFilterRequest>(filter_json)
            .map_err(|err| JsValue::from_str(&format!("could not parse filter request: {err}")))?;
        self.current_filter = filter;
        self.log = self.original_log.filter(&self.current_filter);
        Ok(self.log.summary_json())
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
        let parsed_query = StateQuery::parse(query).map_err(JsValue::from)?;
        let attribute = parsed_query.attribute_name;
        let leading_object_type = parsed_query.leading_object_type;
        self.original_log
            .apply_state_query(query)
            .map_err(JsValue::from)?;
        self.log = self.original_log.filter(&self.current_filter);
        let assigned_events = self.log.count_events_with_attribute(&attribute);
        let total_events = self.log.events.len();

        let result = StateQueryResult {
            attribute,
            leading_object_type,
            assigned_events,
            total_events,
        };
        serde_json::to_string(&result)
            .map_err(|err| JsValue::from_str(&format!("could not serialize state result: {err}")))
    }

    /// Detects ranked intra-state and inter-state behavioral patterns.
    #[wasm_bindgen(js_name = statePatternsJson)]
    pub fn state_patterns_json(&self) -> Result<String, JsValue> {
        self.log.state_patterns_json().map_err(JsValue::from)
    }

    /// Extracts object-level features and detects execution-state cells with PCA and SOM.
    #[wasm_bindgen(js_name = stateDetectionJson)]
    pub fn state_detection_json(&self, request_json: &str) -> Result<String, JsValue> {
        let request =
            serde_json::from_str::<StateDetectionRequest>(request_json).map_err(|err| {
                JsValue::from_str(&format!("could not parse state detection request: {err}"))
            })?;
        self.log
            .state_detection_json(&request)
            .map_err(JsValue::from)
    }

    /// Returns details for one SOM cell, including a DFG and entering/exiting windows.
    #[wasm_bindgen(js_name = stateDetectionCellJson)]
    pub fn state_detection_cell_json(&self, request_json: &str) -> Result<String, JsValue> {
        let request =
            serde_json::from_str::<StateDetectionCellRequest>(request_json).map_err(|err| {
                JsValue::from_str(&format!(
                    "could not parse state detection cell request: {err}"
                ))
            })?;
        self.log
            .state_detection_cell_json(&request)
            .map_err(JsValue::from)
    }

    /// Returns the object-level numerical feature table as CSV.
    #[wasm_bindgen(js_name = stateFeatureTableCsv)]
    pub fn state_feature_table_csv(&self, request_json: &str) -> Result<String, JsValue> {
        let request =
            serde_json::from_str::<StateDetectionRequest>(request_json).map_err(|err| {
                JsValue::from_str(&format!("could not parse state detection request: {err}"))
            })?;
        self.log
            .state_feature_table_csv(&request)
            .map_err(JsValue::from)
    }

    /// Computes a flattened directly-follows graph for one object type.
    #[wasm_bindgen(js_name = directlyFollowsGraphJson)]
    pub fn directly_follows_graph_json(&self, object_type: &str) -> Result<String, JsValue> {
        self.log
            .directly_follows_graph_json(object_type)
            .map_err(JsValue::from)
    }

    /// Computes an object-centric directly-follows graph collated over all object types.
    #[wasm_bindgen(js_name = objectCentricDirectlyFollowsGraphJson)]
    pub fn object_centric_directly_follows_graph_json(&self) -> Result<String, JsValue> {
        self.log
            .object_centric_directly_follows_graph_json()
            .map_err(JsValue::from)
    }

    /// Computes an object-centric directly-follows graph for selected object types and frequencies.
    #[wasm_bindgen(js_name = filteredObjectCentricDirectlyFollowsGraphJson)]
    pub fn filtered_object_centric_directly_follows_graph_json(
        &self,
        request_json: &str,
    ) -> Result<String, JsValue> {
        let request = serde_json::from_str::<GraphFilterRequest>(request_json).map_err(|err| {
            JsValue::from_str(&format!("could not parse graph filter request: {err}"))
        })?;
        self.log
            .object_centric_directly_follows_graph_json_with_filter(&request)
            .map_err(JsValue::from)
    }

    /// Computes a state-aware object-centric directly-follows graph.
    #[wasm_bindgen(js_name = stateAwareObjectCentricDirectlyFollowsGraphJson)]
    pub fn state_aware_ocdfg_json(&self) -> Result<String, JsValue> {
        self.log.state_aware_ocdfg_json().map_err(JsValue::from)
    }

    /// Computes a state-aware OCDFG for selected object types and frequencies.
    #[wasm_bindgen(js_name = filteredStateAwareObjectCentricDirectlyFollowsGraphJson)]
    pub fn filtered_state_aware_ocdfg_json(&self, request_json: &str) -> Result<String, JsValue> {
        let request = serde_json::from_str::<GraphFilterRequest>(request_json).map_err(|err| {
            JsValue::from_str(&format!("could not parse graph filter request: {err}"))
        })?;
        self.log
            .state_aware_ocdfg_json_with_filter(&request)
            .map_err(JsValue::from)
    }
}

#[derive(Serialize)]
struct StateQueryResult {
    attribute: String,
    leading_object_type: String,
    assigned_events: usize,
    total_events: usize,
}

#[derive(Debug)]
struct StateQuery {
    attribute_name: String,
    leading_object_type: String,
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
        self.expect_keyword("FOR")?;
        self.expect_keyword("LEADING")?;
        self.expect_keyword("OBJECT")?;
        self.expect_keyword("TYPE")?;
        let leading_object_type = self.parse_type_name()?;
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
            leading_object_type,
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

    fn parse_type_name(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) | Some(Token::String(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected leading object type name in state query, found {other:?}"
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
                STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
                  WHEN object.is_blocked = 'Yes' THEN 'Blocked'
                  WHEN event.type LIKE '%Payment%' THEN 'Payment'
                  ELSE 'Normal'
                END
                "#,
            )
            .unwrap();

        let result: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(result["leading_object_type"], Value::from("Invoice"));
        assert!(result["assigned_events"].as_u64().unwrap() > 0);
        assert_eq!(
            log.summary().stateful_events,
            result["assigned_events"].as_u64().unwrap() as usize
        );
        assert_eq!(event_state(&log, "e11"), Some("Blocked".to_owned()));
        assert_eq!(event_state(&log, "e9"), Some("Normal".to_owned()));
        assert_eq!(event_state(&log, "e13"), Some("Payment".to_owned()));

        let exported = log.export_json().unwrap();
        let reparsed = CompactOcelLog::from_input(&exported, Some("json")).unwrap();
        assert_eq!(
            reparsed.summary().stateful_events,
            log.summary().stateful_events
        );
        assert_eq!(event_state(&reparsed, "e11"), Some("Blocked".to_owned()));
    }

    #[test]
    fn supports_event_field_values_as_state_results() {
        let mut log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        log.apply_state_query(
            r#"
            STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
              WHEN event.type = 'Insert Invoice' THEN event.type
              ELSE 'Other'
            END
            "#,
        )
        .unwrap();

        assert_eq!(event_state(&log, "e5"), Some("Insert Invoice".to_owned()));
        assert_eq!(event_state(&log, "e1"), None);
    }

    #[test]
    fn filters_active_log_by_event_and_object_types() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let filtered = log.filter(&OcelFilterRequest {
            event_types: vec![
                "Create Purchase Order".to_owned(),
                "Insert Invoice".to_owned(),
            ],
            object_types: vec!["Purchase Order".to_owned(), "Invoice".to_owned()],
        });
        let summary = filtered.summary();

        assert!(summary.events > 0);
        assert!(summary.events < log.summary().events);
        assert!(summary.objects > 0);
        assert!(summary.objects < log.summary().objects);
        assert_eq!(summary.event_types, 2);
        assert!(filtered.events.iter().all(|event| matches!(
            filtered.pool.resolve(event.type_name),
            "Create Purchase Order" | "Insert Invoice"
        )));
        assert!(filtered.objects.iter().all(|object| matches!(
            filtered.pool.resolve(object.type_name),
            "Purchase Order" | "Invoice"
        )));
    }

    #[test]
    fn computes_flattened_and_object_centric_dfg_layouts() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();

        let flattened: Value =
            serde_json::from_str(&log.directly_follows_graph_json("Purchase Order").unwrap())
                .unwrap();
        assert_eq!(
            flattened["title"],
            Value::from("Directly-Follows Graph: Purchase Order")
        );
        assert!(flattened["nodes"]
            .as_array()
            .is_some_and(|nodes| !nodes.is_empty()));
        assert!(flattened["nodes"].as_array().is_some_and(|nodes| {
            nodes.iter().any(|node| {
                node["kind"] == Value::from("object-start")
                    && node["shape"] == Value::from("ellipse")
                    && node["object_type"] == Value::from("Purchase Order")
            }) && nodes.iter().any(|node| {
                node["kind"] == Value::from("object-end")
                    && node["shape"] == Value::from("ellipse")
                    && node["object_type"] == Value::from("Purchase Order")
            })
        }));
        assert!(flattened["edges"].as_array().is_some_and(|edges| {
            edges.iter().any(|edge| {
                edge["object_type"] == Value::from("Purchase Order")
                    && edge["color"].as_str().is_some()
                    && edge["path"].as_str().is_some_and(|path| path.contains('C'))
            })
        }));

        let object_centric: Value =
            serde_json::from_str(&log.object_centric_directly_follows_graph_json().unwrap())
                .unwrap();
        assert!(object_centric["nodes"]
            .as_array()
            .is_some_and(|nodes| nodes.len() >= flattened["nodes"].as_array().unwrap().len()));
        assert!(object_centric["nodes"].as_array().is_some_and(|nodes| {
            nodes.iter().any(|node| {
                node["kind"] == Value::from("object-start")
                    && node["shape"] == Value::from("ellipse")
                    && node["object_type"] == Value::from("Invoice")
            })
        }));
        assert!(object_centric["edges"].as_array().is_some_and(|edges| {
            edges.iter().any(|edge| {
                edge["object_type"] == Value::from("Invoice")
                    && edge["color"].as_str().is_some()
                    && edge["object_types"].as_array().is_some_and(|types| {
                        types.len() == 1
                            && types[0]["object_type"] == Value::from("Invoice")
                            && types[0]["weight"] == edge["weight"]
                    })
            })
        }));

        let filtered: Value = serde_json::from_str(
            &log.object_centric_directly_follows_graph_json_with_filter(&GraphFilterRequest {
                object_types: Some(vec!["Invoice".to_owned()]),
                min_activity_frequency: Some(2),
                min_path_frequency: Some(2),
            })
            .unwrap(),
        )
        .unwrap();
        assert!(filtered["nodes"].as_array().is_some_and(|nodes| {
            nodes.iter().all(|node| {
                matches!(node["kind"].as_str(), Some("object-start" | "object-end"))
                    || node["count"].as_u64().is_some_and(|count| count >= 2)
            })
        }));
        assert!(filtered["edges"].as_array().is_some_and(|edges| {
            !edges.is_empty()
                && edges.iter().all(|edge| {
                    edge["object_type"] == Value::from("Invoice")
                        && edge["weight"].as_u64().is_some_and(|weight| weight >= 2)
                })
        }));
    }

    #[test]
    fn computes_state_aware_ocdfg_layout() {
        let mut log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        log.apply_state_query(
            r#"
            STATE state FOR LEADING OBJECT TYPE 'Purchase Order' AS CASE
              WHEN event.type = 'Create Purchase Order' THEN 'Created'
              ELSE 'Follow-Up'
            END
            "#,
        )
        .unwrap();

        let graph: Value = serde_json::from_str(&log.state_aware_ocdfg_json().unwrap()).unwrap();
        assert_eq!(
            graph["title"],
            Value::from("State-Aware Object-Centric Directly-Follows Graph")
        );
        assert!(graph["nodes"].as_array().is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node["kind"] == Value::from("state-change"))
        }));
        assert!(graph["nodes"].as_array().is_some_and(|nodes| {
            nodes.iter().any(|node| {
                node["kind"] == Value::from("object-start")
                    && node["shape"] == Value::from("ellipse")
                    && node["object_type"] == Value::from("Purchase Order")
            })
        }));
        assert!(graph["edges"].as_array().is_some_and(|edges| {
            edges.iter().any(|edge| {
                edge["object_type"] == Value::from("Purchase Order")
                    && edge["color"].as_str().is_some()
                    && edge["path"].as_str().is_some_and(|path| path.contains('C'))
            })
        }));
    }

    #[test]
    fn wasm_filter_preserves_state_from_original_log() {
        let mut document = OcelDocument::new(JSON_EXAMPLE, Some("json".to_owned())).unwrap();
        document
            .apply_state_query(
                r#"
                STATE state FOR LEADING OBJECT TYPE 'Purchase Order' AS CASE
                  WHEN event.type LIKE '%Invoice%' THEN 'Invoice'
                  ELSE 'Other'
                END
                "#,
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&document.summary_json()).unwrap()["stateful_events"],
            Value::from(5)
        );

        document
            .apply_filter(
                r#"{"event_types":["Create Purchase Order"],"object_types":["Purchase Order"]}"#,
            )
            .unwrap();
        let summary = serde_json::from_str::<Value>(&document.summary_json()).unwrap();
        let original = serde_json::from_str::<Value>(&document.original_summary_json()).unwrap();

        assert_eq!(summary["stateful_events"], summary["events"]);
        assert_eq!(original["stateful_events"], Value::from(5));
        assert!(summary["events"].as_u64().unwrap() < original["events"].as_u64().unwrap());
        assert!(!document.state_patterns_json().unwrap().is_empty());
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
            STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
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
    fn extracts_state_detection_feature_table_and_csv() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let request = StateDetectionRequest {
            object_type: "Purchase Order".to_owned(),
            window_size: Some(2),
            som_width: Some(3),
            som_height: Some(3),
            epochs: Some(20),
            color_attribute: None,
        };

        let table = log.state_feature_table("Purchase Order").unwrap();
        assert!(!table.rows.is_empty());
        assert!(table
            .columns
            .iter()
            .any(|column| column == "activity.Create Purchase Order"));
        assert!(table
            .columns
            .iter()
            .any(|column| column == "related_objects.Invoice"));
        assert_eq!(table.rows[0].values.len(), table.columns.len());

        let csv = log.state_feature_table_csv(&request).unwrap();
        assert!(csv.starts_with("object_id,"));
        assert!(csv.contains("activity.Create Purchase Order"));
        assert!(csv.contains("PO1"));
    }

    #[test]
    fn detects_execution_states_with_pca_and_som() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let request = StateDetectionRequest {
            object_type: "Purchase Order".to_owned(),
            window_size: Some(2),
            som_width: Some(3),
            som_height: Some(2),
            epochs: Some(25),
            color_attribute: None,
        };

        let json = log.state_detection_json(&request).unwrap();
        let analysis: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(analysis["object_type"], Value::from("Purchase Order"));
        assert_eq!(analysis["som_width"], Value::from(3));
        assert_eq!(analysis["som_height"], Value::from(2));
        assert!(analysis["feature_count"].as_u64().unwrap() > 0);
        assert!(analysis["window_count"].as_u64().unwrap() > 0);
        assert!(analysis["pca"]["pc1_explained_ratio"]
            .as_f64()
            .is_some_and(|value| value >= 0.0));
        assert_eq!(analysis["som"]["cells"].as_array().unwrap().len(), 6);
        assert!(analysis["windows"]
            .as_array()
            .is_some_and(|windows| !windows.is_empty()));
    }

    #[test]
    fn colors_state_detection_cells_and_explains_selected_cell() {
        let log = CompactOcelLog::from_input(JSON_EXAMPLE, Some("json")).unwrap();
        let request = StateDetectionRequest {
            object_type: "Purchase Order".to_owned(),
            window_size: Some(2),
            som_width: Some(3),
            som_height: Some(2),
            epochs: Some(25),
            color_attribute: Some("attribute::po_product".to_owned()),
        };

        let analysis: Value =
            serde_json::from_str(&log.state_detection_json(&request).unwrap()).unwrap();
        assert_eq!(
            analysis["color_attribute"],
            Value::from("attribute::po_product")
        );
        assert!(analysis["color_attributes"]
            .as_array()
            .is_some_and(|options| {
                options.iter().any(|option| {
                    option["id"] == Value::from("attribute::po_product")
                        && option["kind"] == Value::from("categorical")
                })
            }));
        assert!(analysis["som"]["cells"].as_array().is_some_and(|cells| {
            cells.iter().any(|cell| {
                cell["color_kind"] == Value::from("categorical")
                    && cell["color_label"]
                        .as_str()
                        .is_some_and(|label| label.contains("po_product"))
            })
        }));

        let detail_request = StateDetectionCellRequest {
            object_type: "Purchase Order".to_owned(),
            window_size: Some(2),
            som_width: Some(3),
            som_height: Some(2),
            epochs: Some(25),
            color_attribute: Some("attribute::po_product".to_owned()),
            cell_x: 0,
            cell_y: 0,
        };
        let detail: Value =
            serde_json::from_str(&log.state_detection_cell_json(&detail_request).unwrap()).unwrap();
        assert_eq!(detail["cell"]["label"], Value::from("S1-1"));
        assert!(detail["dfg"]["title"]
            .as_str()
            .is_some_and(|title| title.contains("State Detection Cell")));
        assert!(detail["entering_dfg"]["title"]
            .as_str()
            .is_some_and(|title| title.contains("Entering Windows")));
        assert!(detail["exiting_dfg"]["title"]
            .as_str()
            .is_some_and(|title| title.contains("Exiting Windows")));
        assert!(detail["entering_window_count"].as_u64().is_some());
        assert!(detail["exiting_window_count"].as_u64().is_some());
        assert!(detail["entering_windows"].as_array().is_some());
        assert!(detail["exiting_windows"].as_array().is_some());
    }

    #[test]
    fn detects_state_patterns_for_inventory_fixture() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../files/ocel2/inventory_management_simulated.json");
        let input = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
        let mut log = CompactOcelLog::from_input(&input, Some("json")).unwrap();
        log.apply_state_query(
            r#"STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
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
                let result = log
                    .apply_state_query(query)
                    .unwrap_or_else(|err| panic!("preset '{name}' failed on {fixture}: {err}"));
                let result: Value = serde_json::from_str(&result).unwrap();
                assert!(
                    result["assigned_events"]
                        .as_u64()
                        .is_some_and(|count| count > 0),
                    "preset '{name}' should assign at least one event in {fixture}"
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

    #[test]
    fn imports_compressed_ocel_fixtures() {
        for compressed_path in compressed_ocel_fixture_paths() {
            let compressed_name = compressed_path.display().to_string();
            let compressed_bytes = fs::read(&compressed_path)
                .unwrap_or_else(|err| panic!("failed to read {compressed_name}: {err}"));
            let compressed_file_name = compressed_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("compressed fixture should have a file name");
            let compressed_log =
                CompactOcelLog::from_bytes(&compressed_bytes, Some(compressed_file_name))
                    .unwrap_or_else(|err| panic!("failed to import {compressed_name}: {err}"));

            let uncompressed_file_name = compressed_file_name
                .strip_suffix(".gz")
                .expect("compressed fixture should end with .gz");
            let uncompressed_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../files/ocel2")
                .join(uncompressed_file_name);
            let uncompressed_name = uncompressed_path.display().to_string();
            let uncompressed_input = fs::read_to_string(&uncompressed_path)
                .unwrap_or_else(|err| panic!("failed to read {uncompressed_name}: {err}"));
            let uncompressed_log =
                CompactOcelLog::from_input(&uncompressed_input, Some(uncompressed_file_name))
                    .unwrap_or_else(|err| panic!("failed to import {uncompressed_name}: {err}"));

            assert_same_structural_summary(
                &compressed_log.summary(),
                &uncompressed_log.summary(),
                &format!("compressed import changed summary counts for {compressed_name}"),
            );

            let document =
                OcelDocument::from_bytes(&compressed_bytes, Some(compressed_file_name.to_owned()))
                    .unwrap_or_else(|err| {
                        panic!("failed to import compressed fixture through WASM API: {err:?}")
                    });
            let document_summary: Value = serde_json::from_str(&document.summary_json()).unwrap();
            assert_eq!(
                document_summary["events"].as_u64().unwrap() as usize,
                uncompressed_log.summary().events,
                "WASM compressed import changed event count for {compressed_name}"
            );
            assert_eq!(
                document_summary["objects"].as_u64().unwrap() as usize,
                uncompressed_log.summary().objects,
                "WASM compressed import changed object count for {compressed_name}"
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

    fn compressed_ocel_fixture_paths() -> Vec<PathBuf> {
        let fixture_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../files/ocel2_compressed");
        let mut paths = fs::read_dir(&fixture_dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_dir.display()))
            .map(|entry| {
                entry
                    .expect("failed to read compressed fixture directory entry")
                    .path()
            })
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| {
                        name.ends_with(".json.gz")
                            || name.ends_with(".xml.gz")
                            || name.ends_with(".jsonocel.gz")
                            || name.ends_with(".xmlocel.gz")
                    })
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
                        r#"STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
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
                        r#"STATE state FOR LEADING OBJECT TYPE 'Purchase Order' AS CASE
  WHEN object.po_quantity > 500 THEN 'Large PO'
  WHEN object.po_product = 'Notebooks' THEN 'Maverick Buying'
  ELSE 'Standard Purchase'
END"#,
                        &["Large PO", "Maverick Buying", "Standard Purchase"],
                    ),
                    (
                        "Actor and Automation",
                        r#"STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
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
                        r#"STATE state FOR LEADING OBJECT TYPE 'Container' AS CASE
  WHEN object.Status = 'shipped' THEN 'Shipped'
  WHEN object.Status = 'in transit' THEN 'In Transit'
  WHEN object.Status = 'full' THEN 'Loaded'
  WHEN object.Status = 'empty' THEN 'Empty'
  ELSE 'Planning'
END"#,
                        &["Loaded", "Empty", "Planning"],
                    ),
                    (
                        "Load Planning",
                        r#"STATE state FOR LEADING OBJECT TYPE 'Customer Order' AS CASE
  WHEN event.type = 'Book Vehicles' THEN 'Vehicle Booking'
  WHEN event.type LIKE '%Load%' THEN 'Transport Loading'
  WHEN object.AmountofGoods >= 900 THEN 'Large Order'
  ELSE 'Standard Load'
END"#,
                        &["Large Order", "Standard Load"],
                    ),
                    (
                        "Process Phase",
                        r#"STATE state FOR LEADING OBJECT TYPE 'Container' AS CASE
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
                        r#"STATE state FOR LEADING OBJECT TYPE 'packages' AS CASE
  WHEN event.type = 'failed delivery' THEN 'Delivery Failure'
  WHEN event.type = 'package delivered' THEN 'Delivered'
  WHEN event.type LIKE '%package%' OR event.type = 'send package' THEN 'Packaging'
  WHEN event.type LIKE '%pay%' OR event.type = 'payment reminder' THEN 'Payment'
  ELSE 'Order Handling'
END"#,
                        &["Delivery Failure", "Delivered", "Packaging"],
                    ),
                    (
                        "Value and Weight",
                        r#"STATE state FOR LEADING OBJECT TYPE 'items' AS CASE
  WHEN object.weight >= 10 THEN 'Heavy'
  WHEN object.price >= 1000 THEN 'High Value'
  WHEN object.price >= 250 THEN 'Medium Value'
  ELSE 'Standard'
END"#,
                        &["High Value", "Medium Value", "Standard"],
                    ),
                    (
                        "Exception Risk",
                        r#"STATE state FOR LEADING OBJECT TYPE 'orders' AS CASE
  WHEN event.type = 'item out of stock' THEN 'Stock Exception'
  WHEN event.type = 'reorder item' THEN 'Replenishment'
  WHEN event.type = 'payment reminder' THEN 'Payment Risk'
  WHEN event.type = 'failed delivery' THEN 'Delivery Risk'
  ELSE 'Nominal'
END"#,
                        &["Payment Risk", "Nominal"],
                    ),
                ],
            ),
            (
                "inventory_management_simulated.json",
                vec![
                    (
                        "Stock Status",
                        r#"STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
  WHEN event."Stock After" = 0 THEN 'Zero Stock'
  WHEN event."Stock After" < 30 THEN 'Low Stock'
  WHEN event."Stock After" >= 100 THEN 'High Stock'
  ELSE 'Available Stock'
END"#,
                        &["Zero Stock", "Low Stock", "High Stock", "Available Stock"],
                    ),
                    (
                        "Activity Phase",
                        r#"STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
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
                        r#"STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
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
