use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{graph_spectral_workflow, CanonicalGraphNode, GraphError, GraphSpectralSpec};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDiffusionWaveletSpec {
    pub graph: GraphSpectralSpec,
    pub tolerance: f64,
    pub maximum_levels: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiffusionWaveletLevel {
    pub level: usize,
    pub diffusion_power: u64,
    pub retained_rank: usize,
    pub retained_mode_indices: Vec<usize>,
    pub detail_mode_indices: Vec<usize>,
    pub scaling_basis: Vec<Vec<f64>>,
    pub detail_basis: Vec<Vec<f64>>,
    pub compressed_operator_diagonal: Vec<f64>,
    pub approximation_error: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiffusionWaveletTree {
    pub graph_digest: String,
    pub lazy_operator: &'static str,
    pub tolerance: f64,
    pub levels: Vec<DiffusionWaveletLevel>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiffusionWaveletTransformResult {
    pub coarse_mode_indices: Vec<usize>,
    pub coarse: Vec<f64>,
    pub detail_by_level: Vec<Vec<f64>>,
    pub reconstruction_max_abs_error: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphDiffusionWaveletResult {
    pub format: &'static str,
    pub version: u32,
    pub nodes: Vec<CanonicalGraphNode>,
    pub tree: DiffusionWaveletTree,
    pub transform: DiffusionWaveletTransformResult,
    pub claim_status: &'static str,
}

pub fn graph_diffusion_wavelet_workflow(
    spec: GraphDiffusionWaveletSpec,
) -> Result<GraphDiffusionWaveletResult, GraphError> {
    if !spec.tolerance.is_finite()
        || spec.tolerance <= 0.0
        || spec.tolerance >= 1.0
        || !(1..=16).contains(&spec.maximum_levels)
    {
        return Err(GraphError::Invalid(
            "diffusion wavelet requires tolerance in (0,1) and 1-16 levels".into(),
        ));
    }
    let graph = graph_spectral_workflow(spec.graph)?;
    let lambda_max = *graph
        .spectrum
        .eigenvalues
        .last()
        .ok_or_else(|| GraphError::Numerical("missing graph spectrum".into()))?;
    let lazy_eigenvalues = graph
        .spectrum
        .eigenvalues
        .iter()
        .map(|value| (1.0 - value / lambda_max).clamp(0.0, 1.0))
        .collect::<Vec<_>>();
    let mut previous_modes = (0..graph.nodes.len()).collect::<Vec<_>>();
    let mut levels = Vec::new();
    for level in 0..spec.maximum_levels {
        let power = 1_u64 << level;
        let retained = previous_modes
            .iter()
            .copied()
            .filter(|mode| lazy_eigenvalues[*mode].powf(power as f64) > spec.tolerance)
            .collect::<Vec<_>>();
        if retained.is_empty() {
            return Err(GraphError::Numerical(
                "diffusion compression removed every scaling mode".into(),
            ));
        }
        let retained_set = retained.iter().copied().collect::<HashSet<_>>();
        let detail = previous_modes
            .iter()
            .copied()
            .filter(|mode| !retained_set.contains(mode))
            .collect::<Vec<_>>();
        let approximation_error = detail
            .iter()
            .map(|mode| lazy_eigenvalues[*mode].powf(power as f64))
            .fold(0.0_f64, f64::max);
        levels.push(DiffusionWaveletLevel {
            level,
            diffusion_power: power,
            retained_rank: retained.len(),
            retained_mode_indices: retained.clone(),
            detail_mode_indices: detail.clone(),
            scaling_basis: retained
                .iter()
                .map(|mode| graph.spectrum.eigenvectors_by_mode[*mode].clone())
                .collect(),
            detail_basis: detail
                .iter()
                .map(|mode| graph.spectrum.eigenvectors_by_mode[*mode].clone())
                .collect(),
            compressed_operator_diagonal: retained
                .iter()
                .map(|mode| lazy_eigenvalues[*mode].powf(power as f64))
                .collect(),
            approximation_error,
        });
        let stabilized = retained.len() == previous_modes.len();
        previous_modes = retained;
        if previous_modes.len() == 1 || stabilized {
            break;
        }
    }
    let detail_by_level = levels
        .iter()
        .map(|level| {
            level
                .detail_mode_indices
                .iter()
                .map(|mode| graph.spectrum.coefficients[*mode])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let coarse = previous_modes
        .iter()
        .map(|mode| graph.spectrum.coefficients[*mode])
        .collect::<Vec<_>>();
    let mut reconstruction = vec![0.0; graph.nodes.len()];
    for (&mode, coefficient) in previous_modes.iter().zip(&coarse) {
        for (value, basis) in reconstruction
            .iter_mut()
            .zip(&graph.spectrum.eigenvectors_by_mode[mode])
        {
            *value += coefficient * basis;
        }
    }
    for (level, coefficients) in levels.iter().zip(&detail_by_level) {
        for (&mode, coefficient) in level.detail_mode_indices.iter().zip(coefficients) {
            for (value, basis) in reconstruction
                .iter_mut()
                .zip(&graph.spectrum.eigenvectors_by_mode[mode])
            {
                *value += coefficient * basis;
            }
        }
    }
    let reconstruction_max_abs_error = reconstruction
        .iter()
        .zip(graph.nodes.iter().map(|node| node.signal))
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0_f64, f64::max);
    if reconstruction_max_abs_error > 1e-8 {
        return Err(GraphError::Numerical(format!(
            "diffusion wavelet reconstruction error {reconstruction_max_abs_error} exceeds 1e-8"
        )));
    }
    Ok(GraphDiffusionWaveletResult {
        format: "marklab.graph_diffusion_wavelet",
        version: 1,
        nodes: graph.nodes,
        tree: DiffusionWaveletTree {
            graph_digest: graph.graph_digest,
            lazy_operator: "identity_minus_laplacian_over_exact_lambda_max",
            tolerance: spec.tolerance,
            levels,
        },
        transform: DiffusionWaveletTransformResult {
            coarse_mode_indices: previous_modes,
            coarse,
            detail_by_level,
            reconstruction_max_abs_error,
        },
        claim_status: "experimental_exact_symmetric_diffusion_wavelet",
    })
}
