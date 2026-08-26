use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::SimulationError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryPoint {
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryWindow {
    pub xmin_um: f64,
    pub ymin_um: f64,
    pub xmax_um: f64,
    pub ymax_um: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftHistogramNormalization {
    PairProbabilityDensity,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryMatchingSpec {
    pub window: SummaryWindow,
    pub observed: Vec<SummaryPoint>,
    pub generated: Vec<SummaryPoint>,
    pub radius_centers_um: Vec<f64>,
    pub summary_weights: Vec<f64>,
    pub bandwidth_um: f64,
    pub normalization: SoftHistogramNormalization,
    pub maximum_pair_bin_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SoftPairHistogramBin {
    pub radius_center_um: f64,
    pub value: f64,
    pub derivative_sum_wrt_pair_distance: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SoftPairHistogramResult {
    pub normalization: SoftHistogramNormalization,
    pub kernel: &'static str,
    pub bandwidth_um: f64,
    pub point_count: u32,
    pub pair_count: u64,
    pub pair_bin_visits: u64,
    pub bins: Vec<SoftPairHistogramBin>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SummaryLossComponent {
    pub radius_center_um: f64,
    pub weight: f64,
    pub observed: f64,
    pub generated: f64,
    pub squared_difference: f64,
    pub weighted_loss: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SummaryMatchingLossResult {
    pub total: f64,
    pub components: Vec<SummaryLossComponent>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SummaryMatchingResult {
    pub format: &'static str,
    pub version: u32,
    pub window: SummaryWindow,
    pub observed: SoftPairHistogramResult,
    pub generated: SoftPairHistogramResult,
    pub loss: SummaryMatchingLossResult,
    pub differentiability: &'static str,
    pub boundary_policy: &'static str,
    pub claim_status: &'static str,
}

pub fn summary_matching_loss(
    spec: SummaryMatchingSpec,
) -> Result<SummaryMatchingResult, SimulationError> {
    validate(&spec)?;
    let observed = soft_pair_histogram(
        &spec.observed,
        &spec.radius_centers_um,
        spec.bandwidth_um,
        spec.normalization,
        spec.maximum_pair_bin_visits,
    )?;
    let generated = soft_pair_histogram(
        &spec.generated,
        &spec.radius_centers_um,
        spec.bandwidth_um,
        spec.normalization,
        spec.maximum_pair_bin_visits,
    )?;
    let components = spec
        .summary_weights
        .iter()
        .zip(&observed.bins)
        .zip(&generated.bins)
        .map(|((weight, observed), generated)| {
            let squared_difference = (generated.value - observed.value).powi(2);
            SummaryLossComponent {
                radius_center_um: observed.radius_center_um,
                weight: *weight,
                observed: observed.value,
                generated: generated.value,
                squared_difference,
                weighted_loss: weight * squared_difference,
            }
        })
        .collect::<Vec<_>>();
    let total = components.iter().map(|row| row.weighted_loss).sum();
    Ok(SummaryMatchingResult {
        format: "marklab.soft_pair_summary_matching",
        version: 1,
        window: spec.window,
        observed,
        generated,
        loss: SummaryMatchingLossResult { total, components },
        differentiability: "analytic_in_pair_distances_away_from_coincident_points",
        boundary_policy: "no_edge_correction_declared_same_window_comparison",
        claim_status: "research_only_summary_surrogate_not_inferential_validation",
    })
}

pub fn soft_pair_histogram(
    points: &[SummaryPoint],
    radius_centers_um: &[f64],
    bandwidth_um: f64,
    normalization: SoftHistogramNormalization,
    maximum_pair_bin_visits: u64,
) -> Result<SoftPairHistogramResult, SimulationError> {
    let pairs = pair_count(points.len())?;
    let visits = pairs
        .checked_mul(radius_centers_um.len() as u64)
        .ok_or_else(|| SimulationError::Invalid("soft histogram work overflowed".into()))?;
    if visits > maximum_pair_bin_visits {
        return Err(SimulationError::Invalid(format!(
            "{visits} pair-bin visits exceed the declared resource bound"
        )));
    }
    let scale = bandwidth_um * (2.0 * std::f64::consts::PI).sqrt() * pairs as f64;
    let mut values = vec![0.0; radius_centers_um.len()];
    let mut derivatives = vec![0.0; radius_centers_um.len()];
    for left in 0..points.len() {
        for right in left + 1..points.len() {
            let distance = (points[left].x_um - points[right].x_um)
                .hypot(points[left].y_um - points[right].y_um);
            for (index, center) in radius_centers_um.iter().enumerate() {
                let standardized = (distance - center) / bandwidth_um;
                let kernel = (-0.5 * standardized.powi(2)).exp() / scale;
                values[index] += kernel;
                derivatives[index] += -standardized / bandwidth_um * kernel;
            }
        }
    }
    Ok(SoftPairHistogramResult {
        normalization,
        kernel: "gaussian_probability_density",
        bandwidth_um,
        point_count: points.len() as u32,
        pair_count: pairs,
        pair_bin_visits: visits,
        bins: radius_centers_um
            .iter()
            .enumerate()
            .map(|(index, center)| SoftPairHistogramBin {
                radius_center_um: *center,
                value: values[index],
                derivative_sum_wrt_pair_distance: derivatives[index],
            })
            .collect(),
    })
}

fn validate(spec: &SummaryMatchingSpec) -> Result<(), SimulationError> {
    let window = spec.window;
    let controls_valid = [
        window.xmin_um,
        window.ymin_um,
        window.xmax_um,
        window.ymax_um,
    ]
    .iter()
    .all(|value| value.is_finite())
        && window.xmax_um > window.xmin_um
        && window.ymax_um > window.ymin_um
        && (2..=100_000).contains(&spec.observed.len())
        && (2..=100_000).contains(&spec.generated.len())
        && (1..=1_024).contains(&spec.radius_centers_um.len())
        && spec.summary_weights.len() == spec.radius_centers_um.len()
        && spec.bandwidth_um.is_finite()
        && spec.bandwidth_um > 0.0
        && (1..=250_000_000).contains(&spec.maximum_pair_bin_visits)
        && spec
            .summary_weights
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        && spec.summary_weights.iter().any(|value| *value > 0.0)
        && spec
            .radius_centers_um
            .iter()
            .enumerate()
            .all(|(index, value)| {
                value.is_finite()
                    && *value >= 0.0
                    && (index == 0 || spec.radius_centers_um[index - 1] < *value)
            });
    if !controls_valid {
        return Err(SimulationError::Invalid(
            "soft summary window, radii, weights, or controls are invalid".into(),
        ));
    }
    validate_points(&spec.observed, window)?;
    validate_points(&spec.generated, window)?;
    let total_visits = pair_count(spec.observed.len())?
        .checked_add(pair_count(spec.generated.len())?)
        .and_then(|value| value.checked_mul(spec.radius_centers_um.len() as u64))
        .ok_or_else(|| SimulationError::Invalid("summary matching work overflowed".into()))?;
    if total_visits > spec.maximum_pair_bin_visits {
        return Err(SimulationError::Invalid(format!(
            "{total_visits} total pair-bin visits exceed the declared resource bound"
        )));
    }
    Ok(())
}

fn validate_points(points: &[SummaryPoint], window: SummaryWindow) -> Result<(), SimulationError> {
    let mut ids = BTreeSet::new();
    if points.iter().any(|point| {
        point.point_id.is_empty()
            || point.point_id.trim() != point.point_id
            || !ids.insert(point.point_id.as_str())
            || !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || !(window.xmin_um..=window.xmax_um).contains(&point.x_um)
            || !(window.ymin_um..=window.ymax_um).contains(&point.y_um)
    }) {
        return Err(SimulationError::Invalid(
            "soft summary points require unique IDs and finite in-window coordinates".into(),
        ));
    }
    Ok(())
}

fn pair_count(points: usize) -> Result<u64, SimulationError> {
    (points as u64)
        .checked_mul(points.saturating_sub(1) as u64)
        .map(|value| value / 2)
        .ok_or_else(|| SimulationError::Invalid("soft histogram pair count overflowed".into()))
}
