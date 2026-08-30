use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    filter::spectral_filter_signal, graph_spectral_workflow, GraphError, GraphSpectralSpec,
    GraphSpectrum,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSignalPerturbation {
    pub id: String,
    pub signal_delta: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphScatteringSpec {
    pub graph: GraphSpectralSpec,
    pub scales: Vec<f64>,
    pub maximum_order: usize,
    pub stability_ratio_tolerance: f64,
    pub perturbations: Vec<GraphSignalPerturbation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphScatteringFeature {
    pub path: Vec<f64>,
    pub order: usize,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphScatteringStability {
    pub perturbation_id: String,
    pub perturbation_magnitude: f64,
    pub feature_delta: f64,
    pub ratio: f64,
    pub tolerance: f64,
    pub passed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphScatteringResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub wavelet_kernel: &'static str,
    pub pooling: &'static str,
    pub scales: Vec<f64>,
    pub maximum_order: usize,
    pub features: Vec<GraphScatteringFeature>,
    pub stability: Vec<GraphScatteringStability>,
    pub claim_status: &'static str,
}

pub fn graph_scattering_workflow(
    spec: GraphScatteringSpec,
) -> Result<GraphScatteringResult, GraphError> {
    if spec.scales.is_empty()
        || spec.scales.len() > 16
        || spec
            .scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale <= 0.0)
        || spec.scales.windows(2).any(|pair| pair[0] >= pair[1])
        || spec.maximum_order > 2
        || !spec.stability_ratio_tolerance.is_finite()
        || spec.stability_ratio_tolerance <= 0.0
        || spec.perturbations.is_empty()
        || spec.perturbations.len() > 128
    {
        return Err(GraphError::Invalid(
            "scattering requires 1-16 increasing scales, order 0-2, positive tolerance, and perturbations"
                .into(),
        ));
    }
    let graph = graph_spectral_workflow(spec.graph)?;
    let signal = graph
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let features = scattering_features(&graph.spectrum, &signal, &spec.scales, spec.maximum_order);
    let mut perturbation_ids = HashSet::new();
    let stability = spec
        .perturbations
        .iter()
        .map(|perturbation| {
            if perturbation.id.is_empty()
                || perturbation.id.trim() != perturbation.id
                || !perturbation_ids.insert(perturbation.id.as_str())
                || perturbation.signal_delta.len() != signal.len()
                || perturbation
                    .signal_delta
                    .iter()
                    .any(|value| !value.is_finite())
            {
                return Err(GraphError::Invalid(
                    "perturbations require unique IDs and finite node-aligned deltas".into(),
                ));
            }
            let magnitude = perturbation
                .signal_delta
                .iter()
                .map(|value| value.powi(2))
                .sum::<f64>()
                .sqrt();
            if magnitude == 0.0 {
                return Err(GraphError::Invalid(
                    "stability perturbation magnitude must be positive".into(),
                ));
            }
            let perturbed_signal = signal
                .iter()
                .zip(&perturbation.signal_delta)
                .map(|(signal, delta)| signal + delta)
                .collect::<Vec<_>>();
            let perturbed = scattering_features(
                &graph.spectrum,
                &perturbed_signal,
                &spec.scales,
                spec.maximum_order,
            );
            let feature_delta = features
                .iter()
                .zip(&perturbed)
                .map(|(left, right)| (left.value - right.value).powi(2))
                .sum::<f64>()
                .sqrt();
            let ratio = feature_delta / magnitude;
            Ok(GraphScatteringStability {
                perturbation_id: perturbation.id.clone(),
                perturbation_magnitude: magnitude,
                feature_delta,
                ratio,
                tolerance: spec.stability_ratio_tolerance,
                passed: ratio <= spec.stability_ratio_tolerance,
            })
        })
        .collect::<Result<Vec<_>, GraphError>>()?;
    Ok(GraphScatteringResult {
        format: "marklab.graph_scattering",
        version: 1,
        graph_digest: graph.graph_digest,
        wavelet_kernel: "x_exp_minus_x",
        pooling: "arithmetic_mean_over_canonical_nodes",
        scales: spec.scales,
        maximum_order: spec.maximum_order,
        features,
        stability,
        claim_status: "experimental_declared_perturbation_stability_only",
    })
}

fn scattering_features(
    spectrum: &GraphSpectrum,
    signal: &[f64],
    scales: &[f64],
    maximum_order: usize,
) -> Vec<GraphScatteringFeature> {
    let mut states = vec![(Vec::<usize>::new(), signal.to_vec())];
    let mut features = Vec::new();
    for order in 0..=maximum_order {
        let mut next = Vec::new();
        for (path, state_signal) in states {
            features.push(GraphScatteringFeature {
                path: path.iter().map(|index| scales[*index]).collect(),
                order,
                value: state_signal.iter().sum::<f64>() / state_signal.len() as f64,
            });
            if order == maximum_order {
                continue;
            }
            let minimum_scale_index = path.last().map_or(0, |index| index + 1);
            for (scale_index, &scale) in scales.iter().enumerate().skip(minimum_scale_index) {
                let propagated = spectral_filter_signal(spectrum, &state_signal, |eigenvalue| {
                    let value = scale * eigenvalue;
                    value * (-value).exp()
                })
                .into_iter()
                .map(f64::abs)
                .collect();
                let mut next_path = path.clone();
                next_path.push(scale_index);
                next.push((next_path, propagated));
            }
        }
        states = next;
    }
    features
}
