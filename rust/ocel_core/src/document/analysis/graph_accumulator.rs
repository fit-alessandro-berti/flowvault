
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
