
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

fn pattern_filter_matches_instance(filter: &PatternFilter, instance: &PatternInstance) -> bool {
    let expected_family = match filter.family.as_str() {
        "intra" => PatternFamily::Intra,
        "inter" => PatternFamily::Inter,
        _ => return false,
    };
    if instance.family != expected_family
        || instance.leading_object_type != filter.leading_object_type
        || instance.sequence != filter.sequence
    {
        return false;
    }

    let state_matches = match expected_family {
        PatternFamily::Intra => filter
            .state
            .as_deref()
            .is_some_and(|state| state == instance.state.as_str()),
        PatternFamily::Inter => {
            let from_state = filter.from_state.as_deref().or(filter.state.as_deref());
            from_state.is_some_and(|state| state == instance.state.as_str())
                && filter.to_state.as_deref() == instance.to_state.as_deref()
        }
    };
    if !state_matches {
        return false;
    }

    pattern_edge_filter_pairs(&filter.eo_edges) == pattern_instance_edge_pairs(&instance.eo_edges)
        && pattern_edge_filter_pairs(&filter.oo_edges)
            == pattern_instance_edge_pairs(&instance.oo_edges)
}

fn pattern_edge_filter_pairs(edges: &[PatternEdgeFilter]) -> Vec<(String, String)> {
    let mut pairs = edges
        .iter()
        .map(|edge| (edge.source.clone(), edge.target.clone()))
        .collect::<Vec<_>>();
    pairs.sort();
    pairs.dedup();
    pairs
}

fn pattern_instance_edge_pairs(edges: &BTreeMap<(String, String), usize>) -> Vec<(String, String)> {
    edges.keys().cloned().collect()
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
