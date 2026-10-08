use serde::{Deserialize, Serialize};

use crate::{
    graph_spectral_workflow, CanonicalGraphNode, GraphError, GraphSpectralSpec, LaplacianSpec,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionPairSpec {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphHeatSpec {
    pub graph: GraphSpectralSpec,
    pub times: Vec<f64>,
    pub diffusion_pairs: Vec<DiffusionPairSpec>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiffusionDistanceResult {
    pub source_id: String,
    pub target_id: String,
    pub distance: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphHeatAtTime {
    pub time: f64,
    pub kernel: Vec<Vec<f64>>,
    pub applied_signal: Vec<f64>,
    pub heat_kernel_signature: Vec<f64>,
    pub diffusion_distances: Vec<DiffusionDistanceResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphHeatResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub nodes: Vec<CanonicalGraphNode>,
    pub laplacian_kind: LaplacianSpec,
    pub distance_measure: &'static str,
    pub times: Vec<GraphHeatAtTime>,
    pub pair_evaluations: u64,
    pub jacobi_rotations: usize,
    pub kernel_entries_evaluated: u64,
    pub claim_status: &'static str,
}

pub fn graph_heat_workflow(spec: GraphHeatSpec) -> Result<GraphHeatResult, GraphError> {
    if spec.times.is_empty()
        || spec.times.len() > 64
        || spec
            .times
            .iter()
            .any(|time| !time.is_finite() || *time < 0.0)
        || spec.times.windows(2).any(|pair| pair[0] >= pair[1])
        || spec.diffusion_pairs.is_empty()
        || spec.diffusion_pairs.len() > 4_096
    {
        return Err(GraphError::Invalid(
            "heat times must be 1-64 finite increasing nonnegative values with 1-4096 pairs".into(),
        ));
    }
    let graph = graph_spectral_workflow(spec.graph)?;
    let node_index = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<std::collections::BTreeMap<_, _>>();
    let pairs = spec
        .diffusion_pairs
        .iter()
        .map(|pair| {
            let source = node_index
                .get(pair.source_id.as_str())
                .copied()
                .ok_or_else(|| {
                    GraphError::Invalid(format!("unknown diffusion source: {}", pair.source_id))
                })?;
            let target = node_index
                .get(pair.target_id.as_str())
                .copied()
                .ok_or_else(|| {
                    GraphError::Invalid(format!("unknown diffusion target: {}", pair.target_id))
                })?;
            if source == target {
                return Err(GraphError::Invalid(
                    "diffusion pair endpoints must be distinct".into(),
                ));
            }
            Ok((pair, source, target))
        })
        .collect::<Result<Vec<_>, GraphError>>()?;
    let size = graph.nodes.len();
    let signal = graph
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let mut results = Vec::with_capacity(spec.times.len());
    for &time in &spec.times {
        let attenuation = graph
            .spectrum
            .eigenvalues
            .iter()
            .map(|value| (-time * value).exp())
            .collect::<Vec<_>>();
        let mut kernel = vec![vec![0.0; size]; size];
        for (mode, eigenvector) in graph.spectrum.eigenvectors_by_mode.iter().enumerate() {
            for row in 0..size {
                for column in 0..size {
                    kernel[row][column] +=
                        attenuation[mode] * eigenvector[row] * eigenvector[column];
                }
            }
        }
        let applied_signal = kernel
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&signal)
                    .map(|(value, signal)| value * signal)
                    .sum()
            })
            .collect::<Vec<f64>>();
        let heat_kernel_signature = (0..size).map(|index| kernel[index][index]).collect();
        let diffusion_distances = pairs
            .iter()
            .map(|(pair, source, target)| DiffusionDistanceResult {
                source_id: pair.source_id.clone(),
                target_id: pair.target_id.clone(),
                distance: kernel[*source]
                    .iter()
                    .zip(&kernel[*target])
                    .map(|(left, right)| (left - right).powi(2))
                    .sum::<f64>()
                    .sqrt(),
            })
            .collect();
        if kernel
            .iter()
            .flatten()
            .chain(&applied_signal)
            .any(|value| !value.is_finite())
        {
            return Err(GraphError::Numerical(
                "exact heat evaluation produced a non-finite result".into(),
            ));
        }
        results.push(GraphHeatAtTime {
            time,
            kernel,
            applied_signal,
            heat_kernel_signature,
            diffusion_distances,
        });
    }
    let kernel_entries_evaluated = (spec.times.len() as u64)
        .checked_mul(size as u64)
        .and_then(|value| value.checked_mul(size as u64))
        .and_then(|value| value.checked_mul(size as u64))
        .ok_or_else(|| GraphError::Invalid("heat work overflow".into()))?;
    Ok(GraphHeatResult {
        format: "marklab.graph_heat",
        version: 1,
        graph_digest: graph.graph_digest,
        nodes: graph.nodes,
        laplacian_kind: graph.laplacian_kind,
        distance_measure: "uniform_node_l2_between_heat_rows",
        times: results,
        pair_evaluations: graph.work.pair_evaluations,
        jacobi_rotations: graph.work.jacobi_rotations,
        kernel_entries_evaluated,
        claim_status: "experimental_exact_small_graph_heat",
    })
}
