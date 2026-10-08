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
    let frequency_bands = summarize_frequency_bands(
        &spec.bands,
        &laplacian,
        &eigenvalues,
        &eigenvectors,
        &coefficients,
        total_energy,
    )?;
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

fn summarize_frequency_bands(
    bands: &[FrequencyBandSpec],
    laplacian: &[Vec<f64>],
    eigenvalues: &[f64],
    eigenvectors: &[Vec<f64>],
    coefficients: &[f64],
    total_energy: f64,
) -> Result<Vec<FrequencyBandSummary>, GraphError> {
    let resolved_eigenvalues =
        resolved_band_eigenvalues(bands, laplacian, eigenvalues, eigenvectors)?;
    Ok(bands
        .iter()
        .map(|band| {
            let selected = resolved_eigenvalues
                .iter()
                .zip(coefficients)
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
        .collect())
}

pub(crate) fn resolved_band_eigenvalues(
    bands: &[FrequencyBandSpec],
    laplacian: &[Vec<f64>],
    eigenvalues: &[f64],
    eigenvectors: &[Vec<f64>],
) -> Result<Vec<f64>, GraphError> {
    let operator_scale = laplacian
        .iter()
        .map(|row| row.iter().map(|value| value.abs()).sum::<f64>())
        .fold(0.0_f64, f64::max);
    let roundoff_floor = 16.0 * f64::EPSILON * operator_scale.max(1.0);
    let error_bounds = eigenvalues
        .iter()
        .zip(eigenvectors)
        .map(|(eigenvalue, mode)| {
            let residual = laplacian
                .iter()
                .enumerate()
                .map(|(row_index, row)| {
                    let applied = row
                        .iter()
                        .zip(mode)
                        .map(|(entry, value)| entry * value)
                        .sum::<f64>();
                    (applied - eigenvalue * mode[row_index]).powi(2)
                })
                .sum::<f64>()
                .sqrt();
            residual + roundoff_floor
        })
        .collect::<Vec<_>>();

    let mut resolved_eigenvalues = Vec::with_capacity(eigenvalues.len());
    let mut start = 0;
    while start < eigenvalues.len() {
        let mut end = start + 1;
        while end < eigenvalues.len()
            && eigenvalues[end] - eigenvalues[end - 1] <= error_bounds[end] + error_bounds[end - 1]
        {
            end += 1;
        }
        let lower = (start..end)
            .map(|index| eigenvalues[index] - error_bounds[index])
            .fold(f64::INFINITY, f64::min);
        let upper = (start..end)
            .map(|index| eigenvalues[index] + error_bounds[index])
            .fold(f64::NEG_INFINITY, f64::max);
        let mut nearby_boundaries = Vec::new();
        for boundary in bands.iter().flat_map(|band| [band.minimum, band.maximum]) {
            if boundary >= lower && boundary <= upper && !nearby_boundaries.contains(&boundary) {
                nearby_boundaries.push(boundary);
            }
        }
        if nearby_boundaries.len() > 1 {
            return Err(GraphError::Numerical(
                "frequency band boundaries are closer than the resolved eigenvalue precision"
                    .into(),
            ));
        }
        let resolved = nearby_boundaries
            .first()
            .copied()
            .unwrap_or_else(|| eigenvalues[start..end].iter().sum::<f64>() / (end - start) as f64);
        resolved_eigenvalues.extend(std::iter::repeat_n(resolved, end - start));
        start = end;
    }

    Ok(resolved_eigenvalues)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn repeated_eigenspace_spec(ids: [&str; 6]) -> GraphSpectralSpec {
        let coordinates = [
            [0.0, 0.0],
            [1.0, 0.0],
            [2.0, 0.0],
            [3.0, 0.0],
            [3.5, 0.5],
            [3.5, -0.5],
        ];
        let signals = [0.0, 0.0, 0.0, 0.0, 1.0, -1.0];
        GraphSpectralSpec {
            nodes: ids
                .into_iter()
                .zip(coordinates)
                .zip(signals)
                .map(|((id, coordinates_um), signal)| GraphNodeInput {
                    id: id.into(),
                    coordinates_um,
                    signal,
                })
                .collect(),
            radius_um: 1.1,
            weight: GraphWeightSpec::Binary,
            laplacian: LaplacianSpec::Combinatorial,
            bands: vec![
                FrequencyBandSpec {
                    id: "below_three".into(),
                    minimum: 0.0,
                    maximum: 3.0,
                },
                FrequencyBandSpec {
                    id: "three_and_above".into(),
                    minimum: 3.0,
                    maximum: 7.0,
                },
            ],
            maximum_pairs: 15,
        }
    }

    #[test]
    fn repeated_eigenspace_band_energy_is_invariant_to_node_relabeling() {
        let first =
            graph_spectral_workflow(repeated_eigenspace_spec(["a", "b", "c", "d", "e", "f"]))
                .expect("first spectrum");
        let second =
            graph_spectral_workflow(repeated_eigenspace_spec(["a", "d", "f", "c", "b", "e"]))
                .expect("relabeled spectrum");

        for result in [&first, &second] {
            assert_eq!(result.frequency_bands[0].mode_count, 3);
            assert!(result.frequency_bands[0].energy.abs() <= 1e-10);
            assert_eq!(result.frequency_bands[1].mode_count, 3);
            assert!((result.frequency_bands[1].energy - 2.0).abs() <= 1e-10);
        }
    }
}
