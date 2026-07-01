impl CompactOcelLog {

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
}
