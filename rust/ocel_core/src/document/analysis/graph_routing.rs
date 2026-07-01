
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
