use crate::errors::{MarklabError, Result};
use crate::registration::landmarks::LandmarkPair;

#[derive(Clone, Debug, PartialEq)]
pub struct Transform2D {
    pub transform_type: String,
    pub m00: f64,
    pub m01: f64,
    pub m02: f64,
    pub m10: f64,
    pub m11: f64,
    pub m12: f64,
}

impl Transform2D {
    #[cfg(test)]
    pub fn identity(transform_type: impl Into<String>) -> Self {
        Self {
            transform_type: transform_type.into(),
            m00: 1.0,
            m01: 0.0,
            m02: 0.0,
            m10: 0.0,
            m11: 1.0,
            m12: 0.0,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.m00 * x + self.m01 * y + self.m02,
            self.m10 * x + self.m11 * y + self.m12,
        )
    }
}

/// Fits the Task 3 MVP transform: uniform scale plus translation only.
///
/// This preserves the spec-compatible `fit_similarity` API name, but it does
/// not estimate rotation. The returned `transform_type` is `scale_translation`
/// to make that limitation explicit in downstream summaries.
pub fn fit_similarity(landmarks: &[LandmarkPair]) -> Result<Transform2D> {
    validate_landmarks(landmarks)?;
    if landmarks.is_empty() {
        return Err(MarklabError::Compute(
            "at least one landmark is required for similarity".into(),
        ));
    }

    let count = landmarks.len() as f64;
    let source_x_mean = landmarks
        .iter()
        .map(|landmark| landmark.source_x_um)
        .sum::<f64>()
        / count;
    let source_y_mean = landmarks
        .iter()
        .map(|landmark| landmark.source_y_um)
        .sum::<f64>()
        / count;
    let target_x_mean = landmarks
        .iter()
        .map(|landmark| landmark.target_x_um)
        .sum::<f64>()
        / count;
    let target_y_mean = landmarks
        .iter()
        .map(|landmark| landmark.target_y_um)
        .sum::<f64>()
        / count;

    let mut numerator = 0.0;
    let mut denominator = 0.0;
    let mut source_scale = 0.0_f64;
    for landmark in landmarks {
        let source_dx = landmark.source_x_um - source_x_mean;
        let source_dy = landmark.source_y_um - source_y_mean;
        let target_dx = landmark.target_x_um - target_x_mean;
        let target_dy = landmark.target_y_um - target_y_mean;
        numerator += source_dx * target_dx + source_dy * target_dy;
        denominator += source_dx * source_dx + source_dy * source_dy;
        source_scale = source_scale.max(source_dx.abs()).max(source_dy.abs());
    }

    let degenerate_threshold =
        (source_scale * source_scale * landmarks.len() as f64).max(f64::MIN_POSITIVE) * 1.0e-12;
    if denominator <= degenerate_threshold {
        return Err(MarklabError::Compute(
            "source landmarks must span nonzero distance for similarity".into(),
        ));
    }

    let scale = numerator / denominator;
    Ok(Transform2D {
        transform_type: "scale_translation".to_string(),
        m00: scale,
        m01: 0.0,
        m02: target_x_mean - scale * source_x_mean,
        m10: 0.0,
        m11: scale,
        m12: target_y_mean - scale * source_y_mean,
    })
}

pub fn fit_affine(landmarks: &[LandmarkPair]) -> Result<Transform2D> {
    validate_landmarks(landmarks)?;
    if landmarks.len() < 3 {
        return Err(MarklabError::Compute(
            "at least three landmarks are required for affine".into(),
        ));
    }

    let mut normal = [[0.0; 3]; 3];
    let mut target_x = [0.0; 3];
    let mut target_y = [0.0; 3];

    for landmark in landmarks {
        let row = [landmark.source_x_um, landmark.source_y_um, 1.0];
        for i in 0..3 {
            target_x[i] += row[i] * landmark.target_x_um;
            target_y[i] += row[i] * landmark.target_y_um;
            for j in 0..3 {
                normal[i][j] += row[i] * row[j];
            }
        }
    }

    let x_coefficients = solve_3x3(normal, target_x)?;
    let y_coefficients = solve_3x3(normal, target_y)?;

    Ok(Transform2D {
        transform_type: "affine".to_string(),
        m00: x_coefficients[0],
        m01: x_coefficients[1],
        m02: x_coefficients[2],
        m10: y_coefficients[0],
        m11: y_coefficients[1],
        m12: y_coefficients[2],
    })
}

pub(crate) fn validate_landmarks(landmarks: &[LandmarkPair]) -> Result<()> {
    if landmarks.iter().all(LandmarkPair::is_finite) {
        Ok(())
    } else {
        Err(MarklabError::Compute(
            "landmark coordinates must be finite".into(),
        ))
    }
}

fn solve_3x3(mut matrix: [[f64; 3]; 3], mut rhs: [f64; 3]) -> Result<[f64; 3]> {
    let matrix_scale = matrix
        .iter()
        .flat_map(|row| row.iter())
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let singular_threshold = matrix_scale.max(1.0) * 1.0e-12;

    for pivot_col in 0..3 {
        let pivot_row = (pivot_col..3)
            .max_by(|&a, &b| {
                matrix[a][pivot_col]
                    .abs()
                    .total_cmp(&matrix[b][pivot_col].abs())
            })
            .expect("non-empty pivot range");
        if matrix[pivot_row][pivot_col].abs() <= singular_threshold {
            return Err(MarklabError::Compute(
                "landmark geometry is singular for affine transform".into(),
            ));
        }
        if pivot_row != pivot_col {
            matrix.swap(pivot_col, pivot_row);
            rhs.swap(pivot_col, pivot_row);
        }

        let pivot = matrix[pivot_col][pivot_col];
        for value in matrix[pivot_col].iter_mut().skip(pivot_col) {
            *value /= pivot;
        }
        rhs[pivot_col] /= pivot;

        let pivot_row_values = matrix[pivot_col];
        for (row_index, row_values) in matrix.iter_mut().enumerate() {
            if row_index == pivot_col {
                continue;
            }
            let factor = row_values[pivot_col];
            for (value, pivot_value) in row_values
                .iter_mut()
                .zip(pivot_row_values.iter())
                .skip(pivot_col)
            {
                *value -= factor * pivot_value;
            }
            rhs[row_index] -= factor * rhs[pivot_col];
        }
    }

    if rhs.iter().all(|value| value.is_finite()) {
        Ok(rhs)
    } else {
        Err(MarklabError::Compute(
            "affine solve produced non-finite coefficients".into(),
        ))
    }
}
