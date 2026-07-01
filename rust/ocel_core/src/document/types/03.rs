
impl OcelFilterRequest {
    fn all_for(log: &CompactOcelLog) -> Self {
        let options = log.filter_options();
        Self {
            event_types: options.event_types,
            object_types: options.object_types,
            time_range: None,
            df_nodes: Vec::new(),
            df_edges: Vec::new(),
            text_attributes: Vec::new(),
            patterns: Vec::new(),
        }
    }

    fn has_object_predicates(&self) -> bool {
        !self.df_nodes.is_empty()
            || !self.df_edges.is_empty()
            || self
                .text_attributes
                .iter()
                .any(|attribute| !attribute.values.is_empty())
            || !self.patterns.is_empty()
    }

    fn has_time_predicate(&self) -> bool {
        self.time_range
            .as_ref()
            .is_some_and(|range| range.start_ms.is_some() || range.end_ms.is_some())
    }

    fn accepts_time(&self, time_ms: i64) -> bool {
        let Some(range) = &self.time_range else {
            return true;
        };
        if range.start_ms.is_some_and(|start_ms| time_ms < start_ms) {
            return false;
        }
        if range.end_ms.is_some_and(|end_ms| time_ms > end_ms) {
            return false;
        }
        true
    }
}

fn default_text_attribute_scope() -> String {
    "event".to_owned()
}

impl GraphFilterRequest {
    fn layout_filter(&self) -> GraphLayoutFilter {
        GraphLayoutFilter {
            min_activity_frequency: self.min_activity_frequency.unwrap_or_default(),
            min_path_frequency: self.min_path_frequency.unwrap_or_default(),
        }
    }
}
