
fn duration_stats(mut durations: Vec<i64>) -> DurationStats {
    durations.sort_unstable();
    let sample_count = durations.len();
    let avg_duration_ms = if durations.is_empty() {
        None
    } else {
        Some(round_f64(
            durations
                .iter()
                .map(|duration| *duration as f64)
                .sum::<f64>()
                / durations.len() as f64,
        ))
    };

    DurationStats {
        sample_count,
        min_duration_ms: durations.first().copied(),
        median_duration_ms: durations.get(durations.len() / 2).copied(),
        avg_duration_ms,
        max_duration_ms: durations.last().copied(),
    }
}

fn is_recovery_transition(from_state: &str, to_state: &str) -> bool {
    if from_state == to_state {
        return false;
    }
    let to = to_state.to_ascii_lowercase();
    to.contains("normal") || to.contains("available") || to.contains("standard")
}

fn lifecycle_state_bands(events: &[LifecycleEventDetail]) -> Vec<LifecycleStateBand> {
    let mut bands = Vec::new();
    let mut start_index = None::<usize>;
    let mut current_state = None::<String>;

    for (index, event) in events.iter().enumerate() {
        let Some(state) = &event.state else {
            if let Some(start) = start_index.take() {
                if let Some(state) = current_state.take() {
                    bands.push(lifecycle_state_band(&events[start..index], state));
                }
            }
            continue;
        };

        if current_state.as_ref() != Some(state) {
            if let Some(start) = start_index.replace(index) {
                if let Some(previous_state) = current_state.replace(state.clone()) {
                    bands.push(lifecycle_state_band(&events[start..index], previous_state));
                }
            } else {
                current_state = Some(state.clone());
            }
        }
    }

    if let Some(start) = start_index {
        if let Some(state) = current_state {
            bands.push(lifecycle_state_band(&events[start..], state));
        }
    }

    bands
}

fn lifecycle_state_band(events: &[LifecycleEventDetail], state: String) -> LifecycleStateBand {
    let first = events
        .first()
        .expect("state band cannot be built from an empty event slice");
    let last = events
        .last()
        .expect("state band cannot be built from an empty event slice");
    LifecycleStateBand {
        state,
        start_time_ms: first.time_ms,
        end_time_ms: last.time_ms,
        event_count: events.len(),
        start_event_id: first.event_id.clone(),
        end_event_id: last.event_id.clone(),
    }
}

fn numeric_attr_value(value: &AttrValue) -> Option<f64> {
    match value {
        AttrValue::Integer(number) => Some(*number as f64),
        AttrValue::Float(number) if number.is_finite() => Some(*number),
        _ => None,
    }
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
