use crate::errors::{MmrspaceError, Result};
use crate::output::RegistrationSummary;
use crate::registration::landmarks::LandmarkPair;
use crate::registration::transform::{validate_landmarks, Transform2D};

pub fn registration_qc(
    landmarks: &[LandmarkPair],
    transform: &Transform2D,
    claim_distance_multiplier: f64,
) -> Result<RegistrationSummary> {
    validate_landmarks(landmarks)?;
    if landmarks.is_empty() {
        return Err(MmrspaceError::Compute(
            "at least one landmark is required for registration QC".into(),
        ));
    }
    if !claim_distance_multiplier.is_finite() || claim_distance_multiplier <= 0.0 {
        return Err(MmrspaceError::Compute(
            "claim distance multiplier must be finite and positive".into(),
        ));
    }

    let mut residuals = Vec::with_capacity(landmarks.len());
    for landmark in landmarks {
        let (x, y) = transform.apply(landmark.source_x_um, landmark.source_y_um);
        if !x.is_finite() || !y.is_finite() {
            return Err(MmrspaceError::Compute(
                "transform produced non-finite coordinates".into(),
            ));
        }
        let dx = x - landmark.target_x_um;
        let dy = y - landmark.target_y_um;
        residuals.push(dx.hypot(dy));
    }

    residuals.sort_by(f64::total_cmp);
    let mean_squared_residual_um = residuals
        .iter()
        .map(|residual| residual * residual)
        .sum::<f64>()
        / residuals.len() as f64;
    let rmse_um = mean_squared_residual_um.sqrt();
    let median_residual_um = median(&residuals);
    let max_residual_um = *residuals
        .last()
        .expect("registration QC has at least one residual");
    let p95_residual_um = percentile_nearest_rank(&residuals, 0.95);

    Ok(RegistrationSummary {
        transform_type: transform.transform_type.clone(),
        landmark_count: landmarks.len(),
        rmse_um,
        median_residual_um,
        max_residual_um,
        p95_residual_um,
        usable_min_distance_um: p95_residual_um * claim_distance_multiplier,
        status: "ok".to_string(),
    })
}

fn median(sorted_values: &[f64]) -> f64 {
    let midpoint = sorted_values.len() / 2;
    if sorted_values.len().is_multiple_of(2) {
        (sorted_values[midpoint - 1] + sorted_values[midpoint]) / 2.0
    } else {
        sorted_values[midpoint]
    }
}

fn percentile_nearest_rank(sorted_values: &[f64], percentile: f64) -> f64 {
    let rank = (percentile * sorted_values.len() as f64).ceil() as usize;
    sorted_values[rank.saturating_sub(1).min(sorted_values.len() - 1)]
}
