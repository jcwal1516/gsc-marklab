pub(crate) fn cholesky(matrix: &[f64], dimension: usize) -> Option<Vec<f64>> {
    let mut lower = vec![0.0; matrix.len()];
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

pub(crate) fn solve_lower(lower: &[f64], dimension: usize, right: &[f64]) -> Vec<f64> {
    let mut solution = vec![0.0; dimension];
    for row in 0..dimension {
        let mut value = right[row];
        for column in 0..row {
            value -= lower[row * dimension + column] * solution[column];
        }
        solution[row] = value / lower[row * dimension + row];
    }
    solution
}

pub(crate) fn solve_lower_dot(lower: &[f64], dimension: usize, right: &[f64]) -> Vec<f64> {
    let mut solution = vec![0.0; dimension];
    for row in 0..dimension {
        let prior = (0..row)
            .map(|column| lower[row * dimension + column] * solution[column])
            .sum::<f64>();
        solution[row] = (right[row] - prior) / lower[row * dimension + row];
    }
    solution
}

pub(crate) fn solve_upper_from_lower_transpose_in_place(
    lower: &[f64],
    dimension: usize,
    right: &mut [f64],
) {
    for row in (0..dimension).rev() {
        for column in row + 1..dimension {
            right[row] -= lower[column * dimension + row] * right[column];
        }
        right[row] /= lower[row * dimension + row];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_cholesky(matrix: &[f64], dimension: usize) -> Option<Vec<f64>> {
        let mut lower = vec![0.0; matrix.len()];
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

    fn reference_solve_lower_dot(lower: &[f64], dimension: usize, right: &[f64]) -> Vec<f64> {
        let mut solution = vec![0.0; dimension];
        for row in 0..dimension {
            let prior = (0..row)
                .map(|column| lower[row * dimension + column] * solution[column])
                .sum::<f64>();
            solution[row] = (right[row] - prior) / lower[row * dimension + row];
        }
        solution
    }

    #[test]
    fn cholesky_matches_the_previous_loop_bit_for_bit() {
        let matrix = [4.0, 2.0, -2.0, 2.0, 10.0, 1.0, -2.0, 1.0, 6.0];
        let expected = reference_cholesky(&matrix, 3).unwrap();
        let actual = cholesky(&matrix, 3).unwrap();
        assert_eq!(
            actual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn triangular_solves_match_hand_oracles() {
        let lower = cholesky(&[4.0, 2.0, 2.0, 5.0], 2).unwrap();
        let right = [2.0, 7.0];
        let forward = solve_lower(&lower, 2, &right);
        assert_eq!(forward, [1.0, 3.0]);
        let mut solution = forward;
        solve_upper_from_lower_transpose_in_place(&lower, 2, &mut solution);
        assert!((solution[0] + 0.25).abs() <= f64::EPSILON);
        assert!((solution[1] - 1.5).abs() <= f64::EPSILON);
    }

    #[test]
    fn dot_product_forward_solve_matches_the_previous_loop_bit_for_bit() {
        let lower = cholesky(&[4.0, 2.0, -2.0, 2.0, 10.0, 1.0, -2.0, 1.0, 6.0], 3).unwrap();
        let right = [1.0, -3.5, 8.25];
        let expected = reference_solve_lower_dot(&lower, 3, &right);
        let actual = solve_lower_dot(&lower, 3, &right);
        assert_eq!(
            actual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn cholesky_rejects_nonpositive_or_nonfinite_pivots() {
        assert!(cholesky(&[1.0, 1.0, 1.0, 1.0], 2).is_none());
        assert!(cholesky(&[f64::NAN], 1).is_none());
    }
}
