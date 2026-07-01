
fn pearson_correlation(left: &[f64], right: &[f64]) -> (f64, usize) {
    let pairs = left
        .iter()
        .zip(right.iter())
        .filter_map(|(left, right)| {
            (left.is_finite() && right.is_finite()).then_some((*left, *right))
        })
        .collect::<Vec<_>>();
    let sample_count = pairs.len();
    if sample_count < 2 {
        return (0.0, sample_count);
    }
    let left_mean = pairs.iter().map(|(left, _)| left).sum::<f64>() / sample_count as f64;
    let right_mean = pairs.iter().map(|(_, right)| right).sum::<f64>() / sample_count as f64;
    let mut covariance = 0.0;
    let mut left_variance = 0.0;
    let mut right_variance = 0.0;
    for (left, right) in pairs {
        let left_centered = left - left_mean;
        let right_centered = right - right_mean;
        covariance += left_centered * right_centered;
        left_variance += left_centered * left_centered;
        right_variance += right_centered * right_centered;
    }
    let denominator = (left_variance * right_variance).sqrt();
    if denominator <= f64::EPSILON {
        return (0.0, sample_count);
    }
    ((covariance / denominator).clamp(-1.0, 1.0), sample_count)
}

fn approximate_correlation_p_value(correlation: f64, sample_count: usize) -> f64 {
    if sample_count < 3 {
        return 1.0;
    }
    let denominator = (1.0 - correlation * correlation).max(1e-12);
    let t_score = correlation.abs() * (((sample_count - 2) as f64) / denominator).sqrt();
    (-0.5 * t_score * t_score).exp().clamp(0.0, 1.0)
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
