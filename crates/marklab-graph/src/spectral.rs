use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::eigendecomposition::symmetric_eigendecomposition;

const MAXIMUM_NODES: usize = 128;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNodeInput {
    pub id: String,
    pub coordinates_um: [f64; 2],
    pub signal: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphWeightSpec {
    Binary,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LaplacianSpec {
    Combinatorial,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrequencyBandSpec {
    pub id: String,
    pub minimum: f64,
    pub maximum: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSpectralSpec {
    pub nodes: Vec<GraphNodeInput>,
    pub radius_um: f64,
    pub weight: GraphWeightSpec,
    pub laplacian: LaplacianSpec,
    pub bands: Vec<FrequencyBandSpec>,
    pub maximum_pairs: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CanonicalGraphNode {
    pub id: String,
    pub coordinates_um: [f64; 2],
    pub signal: f64,
    pub degree: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CanonicalGraphEdge {
    pub source_id: String,
    pub target_id: String,
    pub distance_um: f64,
    pub weight: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphSpectrum {
    pub eigenvalues: Vec<f64>,
    pub eigenvectors_by_mode: Vec<Vec<f64>>,
    pub coefficients: Vec<f64>,
    pub reconstruction_max_abs_error: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FrequencyBandSummary {
    pub id: String,
    pub minimum: f64,
    pub maximum: f64,
    pub mode_count: usize,
    pub energy: f64,
    pub energy_fraction: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphSpectralWork {
    pub pair_evaluations: u64,
    pub jacobi_rotations: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphSpectralResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub graph_rule: &'static str,
    pub radius_um: f64,
    pub weight: GraphWeightSpec,
    pub laplacian_kind: LaplacianSpec,
    pub nodes: Vec<CanonicalGraphNode>,
    pub edges: Vec<CanonicalGraphEdge>,
    pub laplacian: Vec<Vec<f64>>,
    pub spectrum: GraphSpectrum,
    pub frequency_bands: Vec<FrequencyBandSummary>,
    pub work: GraphSpectralWork,
    pub claim_status: &'static str,
}

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("invalid graph spectral specification: {0}")]
    Invalid(String),
    #[error("graph spectral numerical failure: {0}")]
    Numerical(String),
}

pub fn graph_spectral_workflow(
    mut spec: GraphSpectralSpec,
) -> Result<GraphSpectralResult, GraphError> {
    validate(&spec)?;
    spec.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let pair_evaluations = (spec.nodes.len() * (spec.nodes.len() - 1) / 2) as u64;
    if pair_evaluations > spec.maximum_pairs {
        return Err(GraphError::Invalid(format!(
            "pair count {pair_evaluations} exceeds caller maximum {}",
            spec.maximum_pairs
        )));
    }
    let mut edges = Vec::new();
    let mut degrees = vec![0.0; spec.nodes.len()];
    let mut laplacian = vec![vec![0.0; spec.nodes.len()]; spec.nodes.len()];
    for left in 0..spec.nodes.len() {
        for right in (left + 1)..spec.nodes.len() {
            let distance = spec.nodes[left]
                .coordinates_um
                .iter()
                .zip(spec.nodes[right].coordinates_um)
                .map(|(left, right)| (left - right).powi(2))
                .sum::<f64>()
                .sqrt();
            if distance <= spec.radius_um {
                let weight = 1.0;
                degrees[left] += weight;
                degrees[right] += weight;
                laplacian[left][right] = -weight;
                laplacian[right][left] = -weight;
                edges.push(CanonicalGraphEdge {
                    source_id: spec.nodes[left].id.clone(),
                    target_id: spec.nodes[right].id.clone(),
                    distance_um: distance,
                    weight,
                });
            }
        }
    }
    if edges.is_empty() || degrees.contains(&0.0) {
        return Err(GraphError::Invalid(
            "canonical graph must contain edges and no isolated node".into(),
        ));
    }
    for (index, degree) in degrees.iter().enumerate() {
        laplacian[index][index] = *degree;
    }
    let decomposition = symmetric_eigendecomposition(&laplacian)?;
    let eigenvalues = decomposition.values;
    let eigenvectors = decomposition.vectors;
    let jacobi_rotations = decomposition.rotations;
    let signal = spec
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let coefficients = eigenvectors
        .iter()
        .map(|mode| {
            mode.iter()
                .zip(&signal)
                .map(|(basis, value)| basis * value)
                .sum()
        })
        .collect::<Vec<f64>>();
    let reconstruction = (0..signal.len())
        .map(|node| {
            eigenvectors
                .iter()
                .zip(&coefficients)
                .map(|(mode, coefficient)| mode[node] * coefficient)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let reconstruction_max_abs_error = signal
        .iter()
        .zip(reconstruction)
        .map(|(expected, actual)| (expected - actual).abs())
        .fold(0.0_f64, f64::max);
    if reconstruction_max_abs_error > 1e-8 {
        return Err(GraphError::Numerical(format!(
            "Fourier reconstruction error {reconstruction_max_abs_error} exceeds 1e-8"
        )));
    }
    let total_energy = coefficients.iter().map(|value| value.powi(2)).sum::<f64>();
    let frequency_bands = spec
        .bands
        .iter()
        .map(|band| {
            let selected = eigenvalues
                .iter()
                .zip(&coefficients)
                .filter(|(value, _)| **value >= band.minimum && **value < band.maximum)
                .collect::<Vec<_>>();
            let energy = selected.iter().map(|(_, value)| value.powi(2)).sum::<f64>();
            FrequencyBandSummary {
                id: band.id.clone(),
                minimum: band.minimum,
                maximum: band.maximum,
                mode_count: selected.len(),
                energy,
                energy_fraction: if total_energy == 0.0 {
                    0.0
                } else {
                    energy / total_energy
                },
            }
        })
        .collect();
    let nodes = spec
        .nodes
        .iter()
        .zip(degrees)
        .map(|(node, degree)| CanonicalGraphNode {
            id: node.id.clone(),
            coordinates_um: node.coordinates_um,
            signal: node.signal,
            degree,
        })
        .collect::<Vec<_>>();
    let digest_bytes = serde_json::to_vec(&serde_json::json!({
        "nodes": &nodes,
        "edges": &edges,
        "radius_um": spec.radius_um,
        "weight": spec.weight,
        "laplacian": spec.laplacian,
    }))
    .map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(GraphSpectralResult {
        format: "marklab.graph_spectral",
        version: 1,
        graph_digest: format!("{:x}", Sha256::digest(digest_bytes)),
        graph_rule: "physical_radius",
        radius_um: spec.radius_um,
        weight: spec.weight,
        laplacian_kind: spec.laplacian,
        nodes,
        edges,
        laplacian,
        spectrum: GraphSpectrum {
            eigenvalues,
            eigenvectors_by_mode: eigenvectors,
            coefficients,
            reconstruction_max_abs_error,
        },
        frequency_bands,
        work: GraphSpectralWork {
            pair_evaluations,
            jacobi_rotations,
        },
        claim_status: "experimental_synthetic_graph_signal",
    })
}

fn validate(spec: &GraphSpectralSpec) -> Result<(), GraphError> {
    if !(2..=MAXIMUM_NODES).contains(&spec.nodes.len())
        || !spec.radius_um.is_finite()
        || spec.radius_um <= 0.0
        || spec.maximum_pairs == 0
        || spec.bands.is_empty()
    {
        return Err(GraphError::Invalid(
            "requires 2-128 nodes, positive radius/pair limit, and bands".into(),
        ));
    }
    let mut ids = HashSet::new();
    if spec.nodes.iter().any(|node| {
        node.id.is_empty()
            || node.id.trim() != node.id
            || !ids.insert(node.id.as_str())
            || node.coordinates_um.iter().any(|value| !value.is_finite())
            || !node.signal.is_finite()
    }) {
        return Err(GraphError::Invalid(
            "nodes require unique exact IDs and finite coordinates/signal".into(),
        ));
    }
    let mut band_ids = HashSet::new();
    for (index, band) in spec.bands.iter().enumerate() {
        if band.id.is_empty()
            || band.id.trim() != band.id
            || !band_ids.insert(band.id.as_str())
            || !band.minimum.is_finite()
            || !band.maximum.is_finite()
            || band.minimum < 0.0
            || band.minimum >= band.maximum
            || (index > 0 && band.minimum < spec.bands[index - 1].maximum)
        {
            return Err(GraphError::Invalid(
                "bands require unique IDs and ordered nonoverlapping finite intervals".into(),
            ));
        }
    }
    Ok(())
}
