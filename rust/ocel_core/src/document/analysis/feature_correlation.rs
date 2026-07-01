
fn transform_causal_value(value: f64, operation: &str) -> f64 {
    match normalized_causal_operation(operation).as_str() {
        "log10" => {
            if value > 0.0 {
                value.log10()
            } else {
                0.0
            }
        }
        "log_e" => {
            if value > 0.0 {
                value.ln()
            } else {
                0.0
            }
        }
        "sqrt" => value.max(0.0).sqrt(),
        _ => {
            if value.is_finite() {
                value
            } else {
                0.0
            }
        }
    }
}

#[derive(Default)]
struct StateFeatureGroup {
    count: usize,
    sum: f64,
}

fn state_feature_correlation_row(
    feature: &str,
    column_index: usize,
    rows: &[FeatureRow],
    row_states: &[Option<String>],
) -> StateCorrelationRow {
    let mut total_count = 0usize;
    let mut total_sum = 0.0;
    let mut total_sum_squares = 0.0;
    let mut by_state = BTreeMap::<String, StateFeatureGroup>::new();

    for (row, state) in rows.iter().zip(row_states.iter()) {
        let Some(state) = state else {
            continue;
        };
        let Some(value) = row.values.get(column_index).copied() else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }

        total_count += 1;
        total_sum += value;
        total_sum_squares += value * value;
        let group = by_state.entry(state.clone()).or_default();
        group.count += 1;
        group.sum += value;
    }

    if total_count == 0 {
        return StateCorrelationRow {
            feature: feature.to_owned(),
            state: String::new(),
            correlation: 0.0,
            strength: 0.0,
            sample_count: 0,
            state_count: 0,
            mean_in_state: 0.0,
            mean_outside_state: 0.0,
        };
    }

    let total_mean = total_sum / total_count as f64;
    let variance = (total_sum_squares / total_count as f64 - total_mean * total_mean).max(0.0);
    let std_dev = variance.sqrt();
    let mut best_state = String::new();
    let mut best_correlation = 0.0;
    let mut best_strength = 0.0;
    let mut best_state_count = 0usize;
    let mut best_mean_in_state = total_mean;
    let mut best_mean_outside_state = 0.0;

    for (state, group) in &by_state {
        let outside_count = total_count.saturating_sub(group.count);
        let mean_in_state = group.sum / group.count as f64;
        let mean_outside_state = if outside_count > 0 {
            (total_sum - group.sum) / outside_count as f64
        } else {
            0.0
        };
        let correlation = if total_count >= 2 && outside_count > 0 && std_dev > f64::EPSILON {
            let p = group.count as f64 / total_count as f64;
            let q = outside_count as f64 / total_count as f64;
            ((mean_in_state - mean_outside_state) * (p * q).sqrt() / std_dev).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let strength = correlation.abs();
        if best_state.is_empty()
            || strength > best_strength
            || ((strength - best_strength).abs() <= f64::EPSILON && state < &best_state)
        {
            best_state = state.clone();
            best_correlation = correlation;
            best_strength = strength;
            best_state_count = group.count;
            best_mean_in_state = mean_in_state;
            best_mean_outside_state = mean_outside_state;
        }
    }

    StateCorrelationRow {
        feature: feature.to_owned(),
        state: best_state,
        correlation: round_f64(best_correlation),
        strength: round_f64(best_strength),
        sample_count: total_count,
        state_count: best_state_count,
        mean_in_state: round_f64(best_mean_in_state),
        mean_outside_state: round_f64(best_mean_outside_state),
    }
}

fn average_vectors(vectors: &[Vec<f64>], row_count: usize) -> Vec<f64> {
    if vectors.is_empty() {
        return vec![0.0; row_count];
    }
    let mut average = vec![0.0; row_count];
    for vector in vectors {
        for (index, value) in vector.iter().take(row_count).enumerate() {
            average[index] += *value / vectors.len() as f64;
        }
    }
    average
}

fn standardized_vector(values: &[f64]) -> Vec<f64> {
    let (mean, std_dev) = vector_mean_std(values);
    if std_dev <= f64::EPSILON {
        return vec![0.0; values.len()];
    }
    values
        .iter()
        .map(|value| {
            if value.is_finite() {
                (value - mean) / std_dev
            } else {
                0.0
            }
        })
        .collect()
}

fn vector_mean_std(values: &[f64]) -> (f64, f64) {
    let finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if finite.is_empty() {
        return (0.0, 0.0);
    }
    let mean = finite.iter().sum::<f64>() / finite.len() as f64;
    let variance = finite
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>()
        / finite.len().max(1) as f64;
    (mean, variance.sqrt())
}
