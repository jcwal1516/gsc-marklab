//! Small dense linear algebra on row-major `f64` matrices.

/// Lower Cholesky factor `L` with `L Lᵀ = matrix`, or `None` if `matrix` is not
/// numerically positive definite.
///
/// `matrix` is a row-major `dimension × dimension` array; only its lower triangle is read.
pub fn cholesky(matrix: &[f64], dimension: usize) -> Option<Vec<f64>> {
    let mut lower = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in 0..=row {
            let mut value = matrix[row * dimension + column];
            for inner in 0..column {
                value -= lower[row * dimension + inner] * lower[column * dimension + inner];
            }
            if row == column {
                if !value.is_finite() || value <= 0.0 {
                    return None;
                }
                lower[row * dimension + column] = value.sqrt();
            } else {
                lower[row * dimension + column] = value / lower[column * dimension + column];
            }
        }
    }
    Some(lower)
}

/// Solve `L x = right` by forward substitution.
pub fn solve_lower(lower: &[f64], dimension: usize, right: &[f64]) -> Vec<f64> {
    let mut solution = right[..dimension].to_vec();
    for row in 0..dimension {
        for column in 0..row {
            solution[row] -= lower[row * dimension + column] * solution[column];
        }
        solution[row] /= lower[row * dimension + row];
    }
    solution
}

/// Solve `Lᵀ x = right` by back substitution, overwriting `right` with `x`.
pub fn solve_lower_transpose_in_place(lower: &[f64], dimension: usize, right: &mut [f64]) {
    for row in (0..dimension).rev() {
        for column in row + 1..dimension {
            right[row] -= lower[column * dimension + row] * right[column];
        }
        right[row] /= lower[row * dimension + row];
    }
}

/// Solve `L Lᵀ x = right` given the Cholesky factor `L`.
pub fn cholesky_solve(lower: &[f64], dimension: usize, right: &[f64]) -> Vec<f64> {
    let mut solution = solve_lower(lower, dimension, right);
    solve_lower_transpose_in_place(lower, dimension, &mut solution);
    solution
}

/// Solve the square system `matrix x = rhs` by Gauss–Jordan elimination with partial
/// pivoting.
///
/// Returns `None` when a pivot is at most `1e-12` times the largest absolute entry (at
/// least one), i.e. when the system is numerically rank deficient.
pub fn gauss_jordan_solve(matrix: &[Vec<f64>], rhs: &[f64]) -> Option<Vec<f64>> {
    let size = rhs.len();
    let scale = matrix
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let mut augmented = matrix
        .iter()
        .zip(rhs)
        .map(|(row, value)| {
            let mut row = row.clone();
            row.push(*value);
            row
        })
        .collect::<Vec<_>>();
    for pivot in 0..size {
        let selected = (pivot..size).max_by(|left, right| {
            augmented[*left][pivot]
                .abs()
                .total_cmp(&augmented[*right][pivot].abs())
        })?;
        if augmented[selected][pivot].abs() <= 1e-12 * scale {
            return None;
        }
        augmented.swap(pivot, selected);
        let diagonal = augmented[pivot][pivot];
        for value in &mut augmented[pivot][pivot..=size] {
            *value /= diagonal;
        }
        let pivot_values = augmented[pivot][pivot..=size].to_vec();
        for (row_index, row) in augmented.iter_mut().enumerate() {
            if row_index == pivot {
                continue;
            }
            let factor = row[pivot];
            for (value, pivot_value) in row[pivot..=size].iter_mut().zip(&pivot_values) {
                *value -= factor * pivot_value;
            }
        }
    }
    Some(augmented.iter().map(|row| row[size]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual - expected).abs() <= 1e-12, "{actual} != {expected}");
        }
    }

    #[test]
    fn cholesky_matches_a_hand_factorization() {
        // [[4, 2, -2], [2, 10, 1], [-2, 1, 6]] = L Lᵀ with L below.
        let lower = cholesky(&[4.0, 2.0, -2.0, 2.0, 10.0, 1.0, -2.0, 1.0, 6.0], 3).unwrap();

        let last = (6.0_f64 - 1.0 - 4.0 / 9.0).sqrt();
        assert_close(
            &lower,
            &[2.0, 0.0, 0.0, 1.0, 3.0, 0.0, -1.0, 2.0 / 3.0, last],
        );
    }

    #[test]
    fn cholesky_rejects_indefinite_or_non_finite_matrices() {
        assert!(cholesky(&[1.0, 2.0, 2.0, 1.0], 2).is_none());
        assert!(cholesky(&[0.0], 1).is_none());
        assert!(cholesky(&[f64::NAN], 1).is_none());
    }

    #[test]
    fn triangular_and_cholesky_solves_match_hand_oracles() {
        let lower = cholesky(&[4.0, 2.0, 2.0, 5.0], 2).unwrap();

        assert_close(&solve_lower(&lower, 2, &[2.0, 7.0]), &[1.0, 3.0]);
        let mut back = vec![1.0, 3.0];
        solve_lower_transpose_in_place(&lower, 2, &mut back);
        assert_close(&back, &[-0.25, 1.5]);
        assert_close(&cholesky_solve(&lower, 2, &[2.0, 7.0]), &[-0.25, 1.5]);
    }

    #[test]
    fn gauss_jordan_pivots_and_solves() {
        let matrix = vec![vec![0.0, 2.0], vec![3.0, 1.0]];

        assert_close(
            &gauss_jordan_solve(&matrix, &[4.0, 5.0]).unwrap(),
            &[1.0, 2.0],
        );
    }

    #[test]
    fn gauss_jordan_rank_tolerance_scales_with_the_matrix() {
        let singular = vec![vec![1.0e6, 2.0e6], vec![0.5e6, 1.0e6 + 1.0e-7]];

        assert!(gauss_jordan_solve(&singular, &[1.0, 1.0]).is_none());
        assert!(gauss_jordan_solve(&[], &[]).is_some_and(|solution| solution.is_empty()));
    }
}
