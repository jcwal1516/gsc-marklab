use serde::Serialize;

use crate::{BayesError, FitState, GriddedLgcpPredictionResult};

#[derive(Debug, Serialize)]
pub struct GriddedLgcpSpatialPpcSummary {
    pub observed: f64,
    pub replicated_mean: f64,
    pub replicated_sd: f64,
    pub probability_replicated_at_least_observed: f64,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpSpatialPpcSummaries {
    pub cell_count_variance: GriddedLgcpSpatialPpcSummary,
    pub adjacent_mean_absolute_difference: GriddedLgcpSpatialPpcSummary,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpSpatialPpcResult {
    pub format: &'static str,
    pub version: u32,
    pub fit_state: FitState,
    pub replicate_count: u32,
    pub neighbor_pair_count: usize,
    pub summaries: GriddedLgcpSpatialPpcSummaries,
    pub prediction: GriddedLgcpPredictionResult,
    pub claim_status: &'static str,
}

pub fn gridded_lgcp_spatial_ppc(
    prediction: GriddedLgcpPredictionResult,
    observed_counts: &[u64],
) -> Result<GriddedLgcpSpatialPpcResult, BayesError> {
    let dimension = prediction.grid_x as usize * prediction.grid_y as usize;
    if prediction.fit_state != FitState::Complete
        || observed_counts.len() != dimension
        || prediction.patterns.is_empty()
    {
        return Err(BayesError::InvalidSpec(
            "spatial LGCP PPC requires a complete prediction and one observed count per cell"
                .into(),
        ));
    }
    let neighbors = neighbor_pairs(prediction.grid_x as usize, prediction.grid_y as usize);
    let observed_variance = population_variance(observed_counts);
    let observed_contrast = neighbor_contrast(observed_counts, &neighbors);
    let replicated_variance = prediction
        .patterns
        .iter()
        .map(|pattern| population_variance(&pattern.cell_counts))
        .collect::<Vec<_>>();
    let replicated_contrast = prediction
        .patterns
        .iter()
        .map(|pattern| neighbor_contrast(&pattern.cell_counts, &neighbors))
        .collect::<Vec<_>>();
    Ok(GriddedLgcpSpatialPpcResult {
        format: "marklab.bayesian_gridded_lgcp_spatial_ppc",
        version: 1,
        fit_state: prediction.fit_state,
        replicate_count: prediction.replicate_count,
        neighbor_pair_count: neighbors.len(),
        summaries: GriddedLgcpSpatialPpcSummaries {
            cell_count_variance: summarize(observed_variance, &replicated_variance),
            adjacent_mean_absolute_difference: summarize(observed_contrast, &replicated_contrast),
        },
        prediction,
        claim_status: "experimental_discretized_spatial_posterior_predictive",
    })
}

fn neighbor_pairs(grid_x: usize, grid_y: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::with_capacity((grid_x - 1) * grid_y + (grid_y - 1) * grid_x);
    for iy in 0..grid_y {
        for ix in 0..grid_x {
            let index = iy * grid_x + ix;
            if ix + 1 < grid_x {
                pairs.push((index, index + 1));
            }
            if iy + 1 < grid_y {
                pairs.push((index, index + grid_x));
            }
        }
    }
    pairs
}

fn population_variance(values: &[u64]) -> f64 {
    let mean = values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64;
    values
        .iter()
        .map(|value| (*value as f64 - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64
}

fn neighbor_contrast(values: &[u64], pairs: &[(usize, usize)]) -> f64 {
    pairs
        .iter()
        .map(|(left, right)| values[*left].abs_diff(values[*right]) as f64)
        .sum::<f64>()
        / pairs.len() as f64
}

fn summarize(observed: f64, replicated: &[f64]) -> GriddedLgcpSpatialPpcSummary {
    let mean = replicated.iter().sum::<f64>() / replicated.len() as f64;
    let sd = if replicated.len() > 1 {
        (replicated
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (replicated.len() - 1) as f64)
            .sqrt()
    } else {
        0.0
    };
    GriddedLgcpSpatialPpcSummary {
        observed,
        replicated_mean: mean,
        replicated_sd: sd,
        probability_replicated_at_least_observed: replicated
            .iter()
            .filter(|value| **value >= observed)
            .count() as f64
            / replicated.len() as f64,
    }
}
