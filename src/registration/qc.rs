use crate::common::stats::{
    mean_all_finite, median_sorted_average_even, percentile_nearest_rank_sorted,
};
use crate::errors::{MarklabError, Result};
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
        return Err(MarklabError::Compute(
            "at least one landmark is required for registration QC".into(),
        ));
    }
    if !claim_distance_multiplier.is_finite() || claim_distance_multiplier <= 0.0 {
        return Err(MarklabError::Compute(
            "claim distance multiplier must be finite and positive".into(),
        ));
    }

    let mut residuals = Vec::with_capacity(landmarks.len());
    for landmark in landmarks {
        let (x, y) = transform.apply(landmark.source_x_um, landmark.source_y_um);
        if !x.is_finite() || !y.is_finite() {
            return Err(MarklabError::Compute(
                "transform produced non-finite coordinates".into(),
            ));
        }
        let dx = x - landmark.target_x_um;
        let dy = y - landmark.target_y_um;
        residuals.push(dx.hypot(dy));
    }

    residuals.sort_by(f64::total_cmp);
    let mean_squared_residual_um =
        mean_all_finite(residuals.iter().map(|residual| residual * residual)).ok_or_else(|| {
            MarklabError::Compute("registration residual mean is undefined".into())
        })?;
    let rmse_um = mean_squared_residual_um.sqrt();
    let median_residual_um = median_sorted_average_even(&residuals)
        .ok_or_else(|| MarklabError::Compute("registration residual median is undefined".into()))?;
    let max_residual_um = *residuals
        .last()
        .expect("registration QC has at least one residual");
    let p95_residual_um = percentile_nearest_rank_sorted(&residuals, 0.95).ok_or_else(|| {
        MarklabError::Compute("registration residual percentile is undefined".into())
    })?;

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
