
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
