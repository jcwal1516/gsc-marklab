use crate::BayesError;

pub(super) fn cholesky(matrix: &[f64], dimension: usize) -> Result<Vec<f64>, BayesError> {
    crate::linalg::cholesky(matrix, dimension)
        .ok_or_else(|| BayesError::InvalidSpec("pCCA covariance is not positive definite".into()))
}

pub(super) fn solve_cholesky(lower: &[f64], dimension: usize, right: &[f64]) -> Vec<f64> {
    let mut solution = vec![0.0; dimension];
    solve_cholesky_into(lower, dimension, right, &mut solution);
    solution
}

pub(super) fn solve_cholesky_into(
    lower: &[f64],
    dimension: usize,
    right: &[f64],
    solution: &mut [f64],
) {
    for row in 0..dimension {
        let mut value = right[row];
        for column in 0..row {
            value -= lower[row * dimension + column] * solution[column];
        }
        solution[row] = value / lower[row * dimension + row];
    }
    for row in (0..dimension).rev() {
        for column in row + 1..dimension {
            solution[row] -= lower[column * dimension + row] * solution[column];
        }
        solution[row] /= lower[row * dimension + row];
    }
}

pub(super) fn inverse_spd(matrix: &[f64], dimension: usize) -> Result<Vec<f64>, BayesError> {
    let lower = cholesky(matrix, dimension)?;
    let mut inverse = vec![0.0; dimension * dimension];
    for column in 0..dimension {
        let mut unit = vec![0.0; dimension];
        unit[column] = 1.0;
        let solution = solve_cholesky(&lower, dimension, &unit);
        for row in 0..dimension {
            inverse[row * dimension + column] = solution[row];
        }
    }
    Ok(inverse)
}

/// Deterministic thin SVD sufficient for the bounded 32-by-32 cross-covariance initializer.
pub(super) fn leading_singular_vectors(
    cross: &[f64],
    rows: usize,
    columns: usize,
    count: usize,
) -> Result<(Vec<f64>, Vec<f64>), BayesError> {
    if cross.len() != rows * columns || count == 0 || count > rows.min(columns) {
        return Err(BayesError::InvalidSpec(
            "pCCA singular-vector dimensions are invalid".into(),
        ));
    }
    if rows >= columns {
        one_sided_jacobi(cross, rows, columns, count)
    } else {
        let mut transpose = vec![0.0; cross.len()];
        for row in 0..rows {
            for column in 0..columns {
                transpose[column * rows + row] = cross[row * columns + column];
            }
        }
        let (transpose_left, transpose_right) = one_sided_jacobi(&transpose, columns, rows, count)?;
        Ok((transpose_right, transpose_left))
    }
}

// A scaled one-sided Jacobi SVD avoids the condition-number squaring of C'C. Each left vector is
// normalized directly from the same rotated C column as its paired right vector, including weak
// but representable singular directions.
fn one_sided_jacobi(
    matrix: &[f64],
    rows: usize,
    columns: usize,
    count: usize,
) -> Result<(Vec<f64>, Vec<f64>), BayesError> {
    debug_assert!(rows >= columns);
    let scale = matrix.iter().map(|value| value.abs()).fold(0.0, f64::max);
    if !scale.is_finite() {
        return Err(BayesError::InvalidSpec(
            "pCCA singular-vector initialization is nonfinite".into(),
        ));
    }
    let mut rotated = if scale == 0.0 {
        vec![0.0; matrix.len()]
    } else {
        matrix.iter().map(|value| value / scale).collect()
    };
    let mut right = vec![0.0; columns * columns];
    for diagonal in 0..columns {
        right[diagonal * columns + diagonal] = 1.0;
    }
    let mut converged = columns <= 1;
    for _ in 0..(100 * columns.max(1)) {
        let mut changed = false;
        for left_column in 0..columns {
            for right_column in left_column + 1..columns {
                let alpha = (0..rows)
                    .map(|row| rotated[row * columns + left_column].powi(2))
                    .sum::<f64>();
                let beta = (0..rows)
                    .map(|row| rotated[row * columns + right_column].powi(2))
                    .sum::<f64>();
                let gamma = (0..rows)
                    .map(|row| {
                        rotated[row * columns + left_column] * rotated[row * columns + right_column]
                    })
                    .sum::<f64>();
                let threshold =
                    f64::EPSILON * rows.max(columns) as f64 * alpha.sqrt() * beta.sqrt();
                if alpha == 0.0 || beta == 0.0 || gamma.abs() <= threshold {
                    continue;
                }
                let zeta = (beta - alpha) / (2.0 * gamma);
                let tangent = if zeta >= 0.0 {
                    1.0 / (zeta + (1.0 + zeta * zeta).sqrt())
                } else {
                    -1.0 / (-zeta + (1.0 + zeta * zeta).sqrt())
                };
                let cosine = 1.0 / (1.0 + tangent * tangent).sqrt();
                let sine = cosine * tangent;
                for row in 0..rows {
                    let left = rotated[row * columns + left_column];
                    let right_value = rotated[row * columns + right_column];
                    rotated[row * columns + left_column] = cosine * left - sine * right_value;
                    rotated[row * columns + right_column] = sine * left + cosine * right_value;
                }
                for row in 0..columns {
                    let left = right[row * columns + left_column];
                    let right_value = right[row * columns + right_column];
                    right[row * columns + left_column] = cosine * left - sine * right_value;
                    right[row * columns + right_column] = sine * left + cosine * right_value;
                }
                changed = true;
            }
        }
        if !changed {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(BayesError::InvalidSpec(
            "pCCA singular-vector initialization did not converge".into(),
        ));
    }
    if rotated.iter().chain(&right).any(|value| !value.is_finite()) {
        return Err(BayesError::InvalidSpec(
            "pCCA singular-vector initialization is nonfinite".into(),
        ));
    }
    let singular = (0..columns)
        .map(|column| {
            (0..rows)
                .map(|row| rotated[row * columns + column].powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .collect::<Vec<_>>();
    let mut order = (0..columns).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        singular[*right]
            .total_cmp(&singular[*left])
            .then_with(|| left.cmp(right))
    });
    let singular_tolerance = f64::EPSILON * rows.max(columns) as f64 * singular[order[0]];
    let mut left = vec![0.0; rows * count];
    let mut selected_right = vec![0.0; columns * count];
    for (component, &source) in order.iter().take(count).enumerate() {
        for row in 0..columns {
            selected_right[row * count + component] = right[row * columns + source];
        }
        if singular[source] > singular_tolerance {
            for row in 0..rows {
                left[row * count + component] = rotated[row * columns + source] / singular[source];
            }
        } else {
            fill_orthogonal_column(&mut left, rows, count, component);
        }
    }
    Ok((left, selected_right))
}

fn fill_orthogonal_column(matrix: &mut [f64], rows: usize, columns: usize, column: usize) {
    for candidate in 0..rows {
        let mut vector = vec![0.0; rows];
        vector[candidate] = 1.0;
        for previous in 0..column {
            let projection = (0..rows)
                .map(|row| vector[row] * matrix[row * columns + previous])
                .sum::<f64>();
            for row in 0..rows {
                vector[row] -= projection * matrix[row * columns + previous];
            }
        }
        let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
        if norm > 1e-12 {
            for row in 0..rows {
                matrix[row * columns + column] = vector[row] / norm;
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::leading_singular_vectors;

    #[test]
    fn leading_singular_subspace_is_invariant_to_small_finite_scale() {
        let cross = [2.0, 1.0, 1.0, 1.0];
        let (left, right) = leading_singular_vectors(&cross, 2, 2, 1).unwrap();
        for scale in [1e-8, 1e-16] {
            let scaled = cross.map(|value| value * scale);
            let (scaled_left, scaled_right) = leading_singular_vectors(&scaled, 2, 2, 1).unwrap();
            for row in 0..2 {
                for column in 0..2 {
                    assert!(
                        (left[row] * left[column] - scaled_left[row] * scaled_left[column]).abs()
                            <= 1e-12
                    );
                    assert!(
                        (right[row] * right[column] - scaled_right[row] * scaled_right[column])
                            .abs()
                            <= 1e-12
                    );
                }
            }
        }

        let ill_conditioned = [1.0, 1.0, 0.0, -1e-10];
        let (ill_left, ill_right) = leading_singular_vectors(&ill_conditioned, 2, 2, 2).unwrap();
        for component in 0..2 {
            let product = [
                ill_conditioned[0] * ill_right[component]
                    + ill_conditioned[1] * ill_right[2 + component],
                ill_conditioned[2] * ill_right[component]
                    + ill_conditioned[3] * ill_right[2 + component],
            ];
            let norm = product
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
            let signed_alignment =
                product[0] * ill_left[component] + product[1] * ill_left[2 + component];
            assert!(norm > 0.0);
            assert!(signed_alignment / norm >= 1.0 - 1e-12);
        }
    }
}
