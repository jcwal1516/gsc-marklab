use crate::Spatial3dError;

pub(crate) fn validate_spd(matrix: [[f64; 3]; 3]) -> Result<(), Spatial3dError> {
    if matrix.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be finite".into(),
        ));
    }
    for (row_index, row) in matrix.iter().enumerate() {
        for (column_index, value) in row.iter().take(row_index).enumerate() {
            if (*value - matrix[column_index][row_index]).abs() > 1.0e-10 {
                return Err(Spatial3dError::Invalid(
                    "anisotropy matrix must be symmetric".into(),
                ));
            }
        }
    }
    let l00 = matrix[0][0].sqrt();
    if !l00.is_finite() || l00 <= 0.0 {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be positive definite".into(),
        ));
    }
    let l10 = matrix[1][0] / l00;
    let l20 = matrix[2][0] / l00;
    let l11 = (matrix[1][1] - l10 * l10).sqrt();
    if !l11.is_finite() || l11 <= 0.0 {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be positive definite".into(),
        ));
    }
    let l21 = (matrix[2][1] - l20 * l10) / l11;
    let l22 = (matrix[2][2] - l20 * l20 - l21 * l21).sqrt();
    if !l22.is_finite() || l22 <= 0.0 {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be positive definite".into(),
        ));
    }
    Ok(())
}

pub(crate) fn metric_ball_scale(matrix: [[f64; 3]; 3]) -> Result<f64, Spatial3dError> {
    let l00 = matrix[0][0].sqrt();
    let l10 = matrix[1][0] / l00;
    let l20 = matrix[2][0] / l00;
    let l11 = (matrix[1][1] - l10 * l10).sqrt();
    let l21 = (matrix[2][1] - l20 * l10) / l11;
    let l22 = (matrix[2][2] - l20 * l20 - l21 * l21).sqrt();
    let scale = l00 * l11 * l22;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(Spatial3dError::Numerical(
            "anisotropy determinant is nonpositive or non-finite".into(),
        ));
    }
    Ok(scale)
}

pub(crate) fn metric_is_identity(matrix: [[f64; 3]; 3]) -> bool {
    matrix == [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

pub(crate) fn distance(
    displacement: [f64; 3],
    metric: [[f64; 3]; 3],
    non_finite_message: &'static str,
) -> Result<f64, Spatial3dError> {
    let transformed = metric.map(|row| {
        row.iter()
            .zip(displacement)
            .map(|(coefficient, value)| coefficient * value)
            .sum::<f64>()
    });
    let squared = displacement
        .iter()
        .zip(transformed)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    if !squared.is_finite() || squared < 0.0 {
        return Err(Spatial3dError::Numerical(non_finite_message.into()));
    }
    Ok(squared.sqrt())
}
