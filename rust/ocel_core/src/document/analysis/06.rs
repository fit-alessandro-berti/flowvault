
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
