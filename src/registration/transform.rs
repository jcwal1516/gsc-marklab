use crate::common::stats::mean_all_finite;
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

/// Fit an orientation-preserving two-dimensional rigid transformation.
///
/// The least-squares fit estimates rotation and translation only. It cannot
/// absorb scale changes or reflections. Source and target covariance terms
/// are normalized before accumulation to avoid overflow without changing the
/// fitted angle.
pub fn fit_rigid(landmarks: &[LandmarkPair]) -> Result<Transform2D> {
    validate_landmarks(landmarks)?;
    if landmarks.len() < 2 {
        return Err(MarklabError::Compute(
            "at least two landmarks are required for rigid registration".into(),
        ));
    }

    let source_x_mean = mean_all_finite(landmarks.iter().map(|point| point.source_x_um))
        .ok_or_else(|| MarklabError::Compute("rigid source x centroid is undefined".into()))?;
    let source_y_mean = mean_all_finite(landmarks.iter().map(|point| point.source_y_um))
        .ok_or_else(|| MarklabError::Compute("rigid source y centroid is undefined".into()))?;
    let target_x_mean = mean_all_finite(landmarks.iter().map(|point| point.target_x_um))
        .ok_or_else(|| MarklabError::Compute("rigid target x centroid is undefined".into()))?;
    let target_y_mean = mean_all_finite(landmarks.iter().map(|point| point.target_y_um))
        .ok_or_else(|| MarklabError::Compute("rigid target y centroid is undefined".into()))?;

    let source_scale = landmarks.iter().fold(0.0_f64, |scale, point| {
        scale
            .max((point.source_x_um - source_x_mean).abs())
            .max((point.source_y_um - source_y_mean).abs())
    });
    if source_scale == 0.0 || !source_scale.is_finite() {
        return Err(MarklabError::Compute(
            "source landmarks must span nonzero distance for rigid registration".into(),
        ));
    }
    let target_scale = landmarks.iter().fold(0.0_f64, |scale, point| {
        scale
            .max((point.target_x_um - target_x_mean).abs())
            .max((point.target_y_um - target_y_mean).abs())
    });
    if !target_scale.is_finite() {
        return Err(MarklabError::Compute(
            "rigid target landmark spread is non-finite".into(),
        ));
    }

    let (a, b) = if target_scale == 0.0 {
        (0.0, 0.0)
    } else {
        landmarks
            .iter()
            .try_fold((0.0, 0.0), |(a, b), point| {
                let source_x = (point.source_x_um - source_x_mean) / source_scale;
                let source_y = (point.source_y_um - source_y_mean) / source_scale;
                let target_x = (point.target_x_um - target_x_mean) / target_scale;
                let target_y = (point.target_y_um - target_y_mean) / target_scale;
                let next_a = a + source_x * target_x + source_y * target_y;
                let next_b = b + source_x * target_y - source_y * target_x;
                (next_a.is_finite() && next_b.is_finite()).then_some((next_a, next_b))
            })
            .ok_or_else(|| {
                MarklabError::Compute("rigid covariance accumulation is non-finite".into())
            })?
    };
    let theta = b.atan2(a);
    let cosine = theta.cos();
    let sine = theta.sin();
    let translation_x = target_x_mean - cosine * source_x_mean + sine * source_y_mean;
    let translation_y = target_y_mean - sine * source_x_mean - cosine * source_y_mean;
    let coefficients = [cosine, -sine, translation_x, sine, cosine, translation_y];
    if coefficients.iter().any(|value| !value.is_finite()) {
        return Err(MarklabError::Compute(
            "rigid fit produced non-finite coefficients".into(),
        ));
    }

    Ok(Transform2D {
        transform_type: "rigid".into(),
        m00: coefficients[0],
        m01: coefficients[1],
        m02: coefficients[2],
        m10: coefficients[3],
        m11: coefficients[4],
        m12: coefficients[5],
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
