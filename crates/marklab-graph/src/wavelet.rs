use serde::{Deserialize, Serialize};

use crate::{
    filter::spectral_filter, graph_spectral_workflow, CanonicalGraphNode, GraphError,
    GraphSpectralSpec,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphWaveletSpec {
    pub graph: GraphSpectralSpec,
    pub scales: Vec<f64>,
    pub lowpass_scale: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphWaveletScaleResult {
    pub scale: f64,
    pub coefficients: Vec<f64>,
    pub energy: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphWaveletResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub nodes: Vec<CanonicalGraphNode>,
    pub kernel: &'static str,
    pub lowpass_kernel: &'static str,
    pub scales: Vec<GraphWaveletScaleResult>,
    pub lowpass_scale: f64,
    pub scaling_coefficients: Vec<f64>,
    pub scaling_energy: f64,
    pub jacobi_rotations: usize,
    pub claim_status: &'static str,
}

pub fn graph_wavelet_workflow(spec: GraphWaveletSpec) -> Result<GraphWaveletResult, GraphError> {
    if spec.scales.is_empty()
        || spec.scales.len() > 64
        || spec
            .scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale <= 0.0)
        || spec.scales.windows(2).any(|pair| pair[0] >= pair[1])
        || !spec.lowpass_scale.is_finite()
        || spec.lowpass_scale <= 0.0
    {
        return Err(GraphError::Invalid(
            "wavelet scales must be 1-64 increasing positive values with positive lowpass scale"
                .into(),
        ));
    }
    let graph = graph_spectral_workflow(spec.graph)?;
    let scales = spec
        .scales
        .iter()
        .map(|&scale| {
            let coefficients = spectral_filter(&graph.spectrum, |eigenvalue| {
                let value = scale * eigenvalue;
                value * (-value).exp()
            });
            let energy = coefficients.iter().map(|value| value.powi(2)).sum();
            GraphWaveletScaleResult {
                scale,
                coefficients,
                energy,
            }
        })
        .collect::<Vec<_>>();
    let scaling_coefficients = spectral_filter(&graph.spectrum, |eigenvalue| {
        (-spec.lowpass_scale * eigenvalue).exp()
    });
    let scaling_energy = scaling_coefficients.iter().map(|value| value.powi(2)).sum();
    if scales
        .iter()
        .flat_map(|scale| {
            scale
                .coefficients
                .iter()
                .chain(std::iter::once(&scale.energy))
        })
        .chain(&scaling_coefficients)
        .chain(std::iter::once(&scaling_energy))
        .any(|value| !value.is_finite())
    {
        return Err(GraphError::Numerical(
            "spectral wavelet produced a non-finite result".into(),
        ));
    }
    Ok(GraphWaveletResult {
        format: "marklab.graph_spectral_wavelet",
        version: 1,
        graph_digest: graph.graph_digest,
        nodes: graph.nodes,
        kernel: "x_exp_minus_x",
        lowpass_kernel: "exp_minus_x",
        scales,
        lowpass_scale: spec.lowpass_scale,
        scaling_coefficients,
        scaling_energy,
        jacobi_rotations: graph.work.jacobi_rotations,
        claim_status: "experimental_exact_small_graph_wavelet",
    })
}
