use crate::GraphError;

pub(crate) const MAXIMUM_JACOBI_ROTATIONS: usize = 1_000_000;

pub(crate) struct EigenDecomposition {
    pub(crate) values: Vec<f64>,
    pub(crate) vectors: Vec<Vec<f64>>,
    pub(crate) rotations: usize,
}

pub(crate) fn symmetric_eigendecomposition(
    input: &[Vec<f64>],
) -> Result<EigenDecomposition, GraphError> {
    symmetric_eigendecomposition_with_limit(input, MAXIMUM_JACOBI_ROTATIONS)
}

pub(crate) fn symmetric_eigendecomposition_with_limit(
    input: &[Vec<f64>],
    maximum_rotations: usize,
) -> Result<EigenDecomposition, GraphError> {
    let size = input.len();
    if size < 2 || maximum_rotations == 0 {
        return Err(GraphError::Invalid(
            "symmetric eigendecomposition controls are invalid".into(),
        ));
    }
    let mut matrix = input.to_vec();
    let mut vectors = vec![vec![0.0; size]; size];
    for (index, row) in vectors.iter_mut().enumerate() {
        row[index] = 1.0;
    }
    let mut rotations = 0;
    loop {
        let mut selected = (0, 1);
        let mut maximum = matrix[0][1].abs();
        for (row, values) in matrix.iter().enumerate() {
            for (column, value) in values.iter().enumerate().skip(row + 1) {
                if value.abs() > maximum {
                    maximum = value.abs();
                    selected = (row, column);
                }
            }
        }
        if maximum <= 1e-13 {
            break;
        }
        if rotations == maximum_rotations {
            return Err(GraphError::Numerical(
                "Jacobi eigendecomposition did not converge".into(),
            ));
        }
        rotations += 1;
        let (left, right) = selected;
        let angle =
            0.5 * (2.0 * matrix[left][right]).atan2(matrix[right][right] - matrix[left][left]);
        let (sine, cosine) = angle.sin_cos();
        let diagonal_left = matrix[left][left];
        let diagonal_right = matrix[right][right];
        let cross = matrix[left][right];
        for index in 0..size {
            if index != left && index != right {
                let old_left = matrix[index][left];
                let old_right = matrix[index][right];
                matrix[index][left] = cosine * old_left - sine * old_right;
                matrix[left][index] = matrix[index][left];
                matrix[index][right] = sine * old_left + cosine * old_right;
                matrix[right][index] = matrix[index][right];
            }
            let vector_left = vectors[index][left];
            let vector_right = vectors[index][right];
            vectors[index][left] = cosine * vector_left - sine * vector_right;
            vectors[index][right] = sine * vector_left + cosine * vector_right;
        }
        matrix[left][left] = cosine.powi(2) * diagonal_left - 2.0 * sine * cosine * cross
            + sine.powi(2) * diagonal_right;
        matrix[right][right] = sine.powi(2) * diagonal_left
            + 2.0 * sine * cosine * cross
            + cosine.powi(2) * diagonal_right;
        matrix[left][right] = 0.0;
        matrix[right][left] = 0.0;
    }
    let mut modes = (0..size)
        .map(|column| {
            let mut mode = vectors.iter().map(|row| row[column]).collect::<Vec<_>>();
            if mode
                .iter()
                .find(|value| value.abs() > 1e-12)
                .is_some_and(|value| *value < 0.0)
            {
                for value in &mut mode {
                    *value = -*value;
                }
            }
            (matrix[column][column], mode)
        })
        .collect::<Vec<_>>();
    modes.sort_by(|left, right| left.0.total_cmp(&right.0));
    let (mut eigenvalues, eigenvectors): (Vec<_>, Vec<_>) = modes.into_iter().unzip();
    for value in &mut eigenvalues {
        if value.abs() <= 1e-12 {
            *value = 0.0;
        }
    }
    Ok(EigenDecomposition {
        values: eigenvalues,
        vectors: eigenvectors,
        rotations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_node_laplacian_has_zero_and_two_spectrum() {
        let decomposition =
            symmetric_eigendecomposition(&[vec![1.0, -1.0], vec![-1.0, 1.0]]).expect("spectrum");
        assert!(decomposition.values[0].abs() < 1e-12);
        assert!((decomposition.values[1] - 2.0).abs() < 1e-12);
    }
}
