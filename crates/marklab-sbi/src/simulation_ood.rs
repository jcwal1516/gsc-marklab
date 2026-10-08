use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::SbiError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationSummary {
    pub simulation_id: String,
    pub summary: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationOodObservation {
    pub observation_id: String,
    pub summary: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationOodSpec {
    pub feature_names: Vec<String>,
    pub reference_simulations: Vec<SimulationSummary>,
    pub calibration_simulations: Vec<SimulationSummary>,
    pub observed: SimulationOodObservation,
    pub k: u32,
    pub calibration_quantile: f64,
    pub maximum_distance_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SimulationOodCalibrationScore {
    pub simulation_id: String,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SimulationOodResult {
    pub format: &'static str,
    pub version: u32,
    pub method: &'static str,
    pub encoder: &'static str,
    pub feature_names: Vec<String>,
    pub reference_mean: Vec<f64>,
    pub reference_population_sd: Vec<f64>,
    pub k: u32,
    pub calibration_quantile: f64,
    pub calibration_rank: u32,
    pub calibration_scores: Vec<SimulationOodCalibrationScore>,
    pub threshold: f64,
    pub observation_id: String,
    pub observed_score: f64,
    pub conformal_p_value: f64,
    pub status: &'static str,
    pub distance_visits: u64,
    pub claim_status: &'static str,
}

pub fn detect_simulation_ood(mut spec: SimulationOodSpec) -> Result<SimulationOodResult, SbiError> {
    validate(&spec)?;
    spec.reference_simulations
        .sort_by(|left, right| left.simulation_id.cmp(&right.simulation_id));
    spec.calibration_simulations
        .sort_by(|left, right| left.simulation_id.cmp(&right.simulation_id));
    let dimension = spec.feature_names.len();
    let reference_mean = (0..dimension)
        .map(|feature| {
            spec.reference_simulations
                .iter()
                .map(|row| row.summary[feature])
                .sum::<f64>()
                / spec.reference_simulations.len() as f64
        })
        .collect::<Vec<_>>();
    let reference_sd = (0..dimension)
        .map(|feature| {
            (spec
                .reference_simulations
                .iter()
                .map(|row| (row.summary[feature] - reference_mean[feature]).powi(2))
                .sum::<f64>()
                / spec.reference_simulations.len() as f64)
                .sqrt()
        })
        .collect::<Vec<_>>();
    if reference_sd
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(SbiError::Invalid(
            "simulation OOD reference features must all vary".into(),
        ));
    }
    let reference = spec
        .reference_simulations
        .iter()
        .map(|row| standardize(&row.summary, &reference_mean, &reference_sd))
        .collect::<Vec<_>>();
    let mut calibration_scores = spec
        .calibration_simulations
        .iter()
        .map(|row| SimulationOodCalibrationScore {
            simulation_id: row.simulation_id.clone(),
            score: knn_score(
                &standardize(&row.summary, &reference_mean, &reference_sd),
                &reference,
                spec.k as usize,
            ),
        })
        .collect::<Vec<_>>();
    calibration_scores.sort_by(|left, right| left.simulation_id.cmp(&right.simulation_id));
    let mut ordered_scores = calibration_scores
        .iter()
        .map(|row| row.score)
        .collect::<Vec<_>>();
    ordered_scores.sort_by(f64::total_cmp);
    let rank = (spec.calibration_quantile * ordered_scores.len() as f64).ceil() as usize;
    let rank = rank.clamp(1, ordered_scores.len());
    let threshold = ordered_scores[rank - 1];
    let observed_score = knn_score(
        &standardize(&spec.observed.summary, &reference_mean, &reference_sd),
        &reference,
        spec.k as usize,
    );
    let conformal_p_value = (1 + calibration_scores
        .iter()
        .filter(|row| row.score >= observed_score)
        .count()) as f64
        / (calibration_scores.len() + 1) as f64;
    let distance_visits = (spec.calibration_simulations.len() as u64 + 1)
        * spec.reference_simulations.len() as u64
        * dimension as u64;
    Ok(SimulationOodResult {
        format: "marklab.simulation_ood",
        version: 1,
        method: "standardized_knn_distance_with_conformal_calibration",
        encoder: "reference_fit_featurewise_z_score",
        feature_names: spec.feature_names,
        reference_mean,
        reference_population_sd: reference_sd,
        k: spec.k,
        calibration_quantile: spec.calibration_quantile,
        calibration_rank: rank as u32,
        calibration_scores,
        threshold,
        observation_id: spec.observed.observation_id,
        observed_score,
        conformal_p_value,
        status: if observed_score > threshold {
            "out_of_support"
        } else {
            "in_support"
        },
        distance_visits,
        claim_status: "simulation_support_diagnostic_not_model_validity",
    })
}

fn validate(spec: &SimulationOodSpec) -> Result<(), SbiError> {
    let dimension = spec.feature_names.len();
    let counts = (1..=128).contains(&dimension)
        && (3..=100_000).contains(&spec.reference_simulations.len())
        && (4..=100_000).contains(&spec.calibration_simulations.len())
        && (1..=spec.reference_simulations.len()).contains(&(spec.k as usize))
        && spec.calibration_quantile.is_finite()
        && (0.0..1.0).contains(&spec.calibration_quantile)
        && (1..=250_000_000).contains(&spec.maximum_distance_visits);
    let visits = (spec.calibration_simulations.len() as u64 + 1)
        .checked_mul(spec.reference_simulations.len() as u64)
        .and_then(|value| value.checked_mul(dimension as u64))
        .ok_or_else(|| SbiError::Invalid("simulation OOD work overflowed".into()))?;
    let mut feature_names = BTreeSet::new();
    let features_valid = spec
        .feature_names
        .iter()
        .all(|name| !name.is_empty() && name.trim() == name && feature_names.insert(name));
    let mut ids = BTreeSet::new();
    let rows_valid = spec
        .reference_simulations
        .iter()
        .chain(&spec.calibration_simulations)
        .all(|row| {
            !row.simulation_id.is_empty()
                && row.simulation_id.trim() == row.simulation_id
                && ids.insert(row.simulation_id.as_str())
                && row.summary.len() == dimension
                && row.summary.iter().all(|value| value.is_finite())
        })
        && !spec.observed.observation_id.is_empty()
        && spec.observed.observation_id.trim() == spec.observed.observation_id
        && !ids.contains(spec.observed.observation_id.as_str())
        && spec.observed.summary.len() == dimension
        && spec.observed.summary.iter().all(|value| value.is_finite());
    if !counts || visits > spec.maximum_distance_visits || !features_valid || !rows_valid {
        return Err(SbiError::Invalid(
            "simulation OOD inputs, splits, or resources are invalid".into(),
        ));
    }
    Ok(())
}

fn standardize(values: &[f64], mean: &[f64], sd: &[f64]) -> Vec<f64> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (value - mean[index]) / sd[index])
        .collect()
}

fn knn_score(query: &[f64], reference: &[Vec<f64>], k: usize) -> f64 {
    let mut distances = reference
        .iter()
        .map(|row| {
            row.iter()
                .zip(query)
                .map(|(left, right)| (left - right).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .collect::<Vec<_>>();
    distances.sort_by(f64::total_cmp);
    distances[..k].iter().sum::<f64>() / k as f64
}
