use serde::{Deserialize, Serialize};

use crate::{
    chebyshev_apply,
    filter::{heat_chebyshev_coefficients, heat_grid_error, spectral_filter},
    graph_spectral_workflow, GraphError, GraphSpectralSpec,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphChebyshevHeatSpec {
    pub graph: GraphSpectralSpec,
    pub time: f64,
    pub tolerance: f64,
    pub maximum_order: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphChebyshevHeatResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub algorithm: &'static str,
    pub time: f64,
    pub spectral_interval: [f64; 2],
    pub selected_order: usize,
    pub coefficients: Vec<f64>,
    pub estimated_tail_bound: f64,
    pub verified_grid_error: f64,
    pub error_bound_kind: &'static str,
    pub approximate_signal: Vec<f64>,
    pub exact_signal: Vec<f64>,
    pub maximum_signal_error: f64,
    pub matrix_vector_products: usize,
    pub claim_status: &'static str,
}

pub fn graph_chebyshev_heat_workflow(
    spec: GraphChebyshevHeatSpec,
) -> Result<GraphChebyshevHeatResult, GraphError> {
    if !spec.time.is_finite()
        || spec.time <= 0.0
        || !spec.tolerance.is_finite()
        || spec.tolerance <= 0.0
        || spec.tolerance >= 1.0
        || !(1..=256).contains(&spec.maximum_order)
    {
        return Err(GraphError::Invalid(
            "Chebyshev heat requires positive time/tolerance and maximum order 1-256".into(),
        ));
    }
    let graph = graph_spectral_workflow(spec.graph)?;
    let lambda_max = *graph
        .spectrum
        .eigenvalues
        .last()
        .ok_or_else(|| GraphError::Numerical("missing graph spectrum".into()))?;
    if lambda_max <= 0.0 {
        return Err(GraphError::Invalid(
            "Chebyshev scaling requires positive maximum eigenvalue".into(),
        ));
    }
    let reference_order = (spec.maximum_order + 32).clamp(64, 512);
    let reference = heat_chebyshev_coefficients(spec.time, lambda_max, reference_order);
    let mut selected = None;
    for order in 1..=spec.maximum_order {
        let coefficients = reference[..=order].to_vec();
        let estimated_tail_bound = reference[(order + 1)..]
            .iter()
            .map(|value| value.abs())
            .sum::<f64>();
        let verified_grid_error = heat_grid_error(spec.time, lambda_max, &coefficients, 4_097);
        if estimated_tail_bound <= spec.tolerance && verified_grid_error <= spec.tolerance {
            selected = Some((
                order,
                coefficients,
                estimated_tail_bound,
                verified_grid_error,
            ));
            break;
        }
    }
    let (selected_order, coefficients, estimated_tail_bound, verified_grid_error) = selected
        .ok_or_else(|| {
            GraphError::Invalid(
                "requested Chebyshev heat tolerance was not achieved by maximum order".into(),
            )
        })?;
    let mut scaled = graph.laplacian.clone();
    for (row, values) in scaled.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            *value *= 2.0 / lambda_max;
            if row == column {
                *value -= 1.0;
            }
        }
    }
    let signal = graph
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let approximate_signal = chebyshev_apply(&scaled, &signal, &coefficients)?;
    let exact_signal = spectral_filter(&graph.spectrum, |value| (-spec.time * value).exp());
    let maximum_signal_error = approximate_signal
        .iter()
        .zip(&exact_signal)
        .map(|(approximate, exact)| (approximate - exact).abs())
        .fold(0.0_f64, f64::max);
    if maximum_signal_error > spec.tolerance {
        return Err(GraphError::Numerical(format!(
            "Chebyshev signal error {maximum_signal_error} exceeds requested tolerance {}",
            spec.tolerance
        )));
    }
    Ok(GraphChebyshevHeatResult {
        format: "marklab.graph_chebyshev_heat",
        version: 1,
        graph_digest: graph.graph_digest,
        algorithm: "adaptive_chebyshev_heat",
        time: spec.time,
        spectral_interval: [0.0, lambda_max],
        selected_order,
        coefficients,
        estimated_tail_bound,
        verified_grid_error,
        error_bound_kind: "dense_grid_plus_truncated_reference_tail",
        approximate_signal,
        exact_signal,
        maximum_signal_error,
        matrix_vector_products: selected_order,
        claim_status: "experimental_verified_small_graph_approximation",
    })
}
