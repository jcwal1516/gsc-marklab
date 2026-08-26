#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashSet};

use marklab_numerics::extreme_rank_length_envelope;
use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod cellular;
mod heterogeneous;
mod hodge;
mod hypergraph;
mod motif;
mod validation;

pub use cellular::{
    cellular_complex_workflow, CellularComplexArtifact, CellularComplexResult, CellularComplexSpec,
    CellularDomainInput, CellularInterfaceInput, CellularJunctionInput, CellularPerturbationResult,
    CellularSegmentationInput, CellularSegmentationPerturbation, OrientedCellularInterfaceInput,
};
pub use heterogeneous::{
    heterogeneous_graph_message_workflow, HeterogeneousEdge, HeterogeneousMessageResult,
    HeterogeneousMessageSpec, HeterogeneousNodeInput, MessageAggregation, SpatialNearRelationSpec,
};
pub use hodge::{
    simplicial_hodge_workflow, CanonicalHodgeEdge, HodgeDecompositionResult, HodgeEdgeInput,
    SimplicialHodgeResult, SimplicialHodgeSpec, SimplicialInventory,
};
pub use hypergraph::{
    hypergraph_signal_workflow, HyperedgeInput, HypergraphMemberInput, HypergraphNodeInput,
    HypergraphSignalResult, HypergraphSignalSpec, IncidenceEntry,
};
pub use motif::{
    typed_triangle_motif_workflow, MotifEdgeInput, MotifNodeInput, TypedTriangleMotifResult,
    TypedTriangleMotifSpec,
};
pub use validation::{
    validate_graph_mathematics_suite, GraphMathematicsValidationResult, GraphValidationEntry,
};

const MAXIMUM_NODES: usize = 128;
const MAXIMUM_JACOBI_ROTATIONS: usize = 1_000_000;

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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNullNodeStratum {
    pub node_id: String,
    pub stratum: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSpectrumNullSpec {
    pub graph: GraphSpectralSpec,
    pub node_strata: Vec<GraphNullNodeStratum>,
    pub permutations: usize,
    pub alpha: f64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphSpectrumNullBand {
    pub id: String,
    pub minimum: f64,
    pub maximum: f64,
    pub observed: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphSpectrumNullResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub permutation_unit: &'static str,
    pub stratum_count: usize,
    pub bands: Vec<GraphSpectrumNullBand>,
    pub null_band_energies: Vec<Vec<f64>>,
    pub p_global: f64,
    pub low_frequency_p_value: f64,
    pub observed_erl_depth: f64,
    pub critical_erl_depth: f64,
    pub permutations_completed: usize,
    pub alpha: f64,
    pub seed: u64,
    pub spectral_projection_work: u64,
    pub claim_status: &'static str,
}

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

fn spectral_filter(spectrum: &GraphSpectrum, response: impl Fn(f64) -> f64) -> Vec<f64> {
    let signal = spectrum
        .eigenvectors_by_mode
        .iter()
        .zip(&spectrum.coefficients)
        .fold(
            vec![0.0; spectrum.coefficients.len()],
            |mut signal, (mode, coefficient)| {
                for (value, basis) in signal.iter_mut().zip(mode) {
                    *value += coefficient * basis;
                }
                signal
            },
        );
    spectral_filter_signal(spectrum, &signal, response)
}

fn spectral_filter_signal(
    spectrum: &GraphSpectrum,
    signal: &[f64],
    response: impl Fn(f64) -> f64,
) -> Vec<f64> {
    let coefficients = spectrum
        .eigenvectors_by_mode
        .iter()
        .map(|mode| {
            mode.iter()
                .zip(signal)
                .map(|(basis, value)| basis * value)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let size = coefficients.len();
    (0..size)
        .map(|node| {
            spectrum
                .eigenvalues
                .iter()
                .zip(&spectrum.eigenvectors_by_mode)
                .zip(&coefficients)
                .map(|((eigenvalue, mode), coefficient)| {
                    response(*eigenvalue) * coefficient * mode[node]
                })
                .sum()
        })
        .collect()
}

pub fn graph_spectrum_null_test(
    spec: GraphSpectrumNullSpec,
) -> Result<GraphSpectrumNullResult, GraphError> {
    if !(20..=10_000).contains(&spec.permutations)
        || !spec.alpha.is_finite()
        || spec.alpha <= 0.0
        || spec.alpha >= 1.0
        || (spec.permutations + 1) as f64 * spec.alpha < 1.0
    {
        return Err(GraphError::Invalid(
            "spectrum null requires 20-10000 permutations and resolvable alpha in (0,1)".into(),
        ));
    }
    let graph = graph_spectral_workflow(spec.graph)?;
    if spec.node_strata.len() != graph.nodes.len() {
        return Err(GraphError::Invalid(
            "node strata must cover every canonical graph node exactly once".into(),
        ));
    }
    let declared = spec
        .node_strata
        .iter()
        .map(|row| (row.node_id.as_str(), row.stratum.as_str()))
        .collect::<BTreeMap<_, _>>();
    if declared.len() != graph.nodes.len()
        || declared.iter().any(|(node, stratum)| {
            node.is_empty() || stratum.is_empty() || stratum.trim() != *stratum
        })
    {
        return Err(GraphError::Invalid(
            "node strata require unique exact node IDs and nonempty exact labels".into(),
        ));
    }
    let mut groups = BTreeMap::<&str, Vec<usize>>::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        let stratum = declared.get(node.id.as_str()).ok_or_else(|| {
            GraphError::Invalid(format!("missing stratum for graph node {}", node.id))
        })?;
        groups.entry(*stratum).or_default().push(index);
    }
    let observed_signal = graph
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let observed = band_energy_curve(&graph, &observed_signal);
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut permuted_signal = observed_signal.clone();
    let mut null = Vec::with_capacity(spec.permutations);
    for _ in 0..spec.permutations {
        permuted_signal.copy_from_slice(&observed_signal);
        for indices in groups.values() {
            let mut donors = indices
                .iter()
                .map(|index| observed_signal[*index])
                .collect::<Vec<_>>();
            donors.shuffle(&mut rng);
            for (receiver, value) in indices.iter().zip(donors) {
                permuted_signal[*receiver] = value;
            }
        }
        null.push(band_energy_curve(&graph, &permuted_signal));
    }
    let envelope = extreme_rank_length_envelope(&observed, &null, spec.alpha)
        .map_err(|error| GraphError::Numerical(error.to_string()))?;
    let low_frequency_p_value = (1 + null.iter().filter(|curve| curve[0] >= observed[0]).count())
        as f64
        / (spec.permutations + 1) as f64;
    let bands = graph
        .frequency_bands
        .iter()
        .enumerate()
        .map(|(index, band)| GraphSpectrumNullBand {
            id: band.id.clone(),
            minimum: band.minimum,
            maximum: band.maximum,
            observed: observed[index],
            lower: envelope.lower[index],
            upper: envelope.upper[index],
        })
        .collect();
    let spectral_projection_work = (spec.permutations as u64 + 1)
        .checked_mul(graph.nodes.len() as u64)
        .and_then(|value| value.checked_mul(graph.nodes.len() as u64))
        .ok_or_else(|| GraphError::Invalid("spectrum-null work overflow".into()))?;
    Ok(GraphSpectrumNullResult {
        format: "marklab.graph_spectrum_null",
        version: 1,
        graph_digest: graph.graph_digest,
        permutation_unit: "complete_signal_rows_within_declared_strata",
        stratum_count: groups.len(),
        bands,
        null_band_energies: null,
        p_global: envelope.p_global,
        low_frequency_p_value,
        observed_erl_depth: envelope.observed_depth,
        critical_erl_depth: envelope.critical_depth,
        permutations_completed: spec.permutations,
        alpha: spec.alpha,
        seed: spec.seed,
        spectral_projection_work,
        claim_status: "experimental_restricted_graph_spectrum_null",
    })
}

fn band_energy_curve(graph: &GraphSpectralResult, signal: &[f64]) -> Vec<f64> {
    let coefficients = graph
        .spectrum
        .eigenvectors_by_mode
        .iter()
        .map(|mode| {
            mode.iter()
                .zip(signal)
                .map(|(basis, value)| basis * value)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    graph
        .frequency_bands
        .iter()
        .map(|band| {
            graph
                .spectrum
                .eigenvalues
                .iter()
                .zip(&coefficients)
                .filter(|(value, _)| **value >= band.minimum && **value < band.maximum)
                .map(|(_, value)| value.powi(2))
                .sum()
        })
        .collect()
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

pub fn chebyshev_apply(
    scaled_matrix: &[Vec<f64>],
    input: &[f64],
    coefficients: &[f64],
) -> Result<Vec<f64>, GraphError> {
    let size = input.len();
    if size == 0
        || scaled_matrix.len() != size
        || scaled_matrix
            .iter()
            .any(|row| row.len() != size || row.iter().any(|value| !value.is_finite()))
        || input.iter().any(|value| !value.is_finite())
        || coefficients.is_empty()
        || coefficients.iter().any(|value| !value.is_finite())
    {
        return Err(GraphError::Invalid(
            "Chebyshev apply requires finite square matrix, aligned vector, and coefficients"
                .into(),
        ));
    }
    let mut previous = input.to_vec();
    let mut output = previous
        .iter()
        .map(|value| 0.5 * coefficients[0] * value)
        .collect::<Vec<_>>();
    if coefficients.len() == 1 {
        return Ok(output);
    }
    let mut current = matrix_vector(scaled_matrix, input);
    for (target, value) in output.iter_mut().zip(&current) {
        *target += coefficients[1] * value;
    }
    for &coefficient in coefficients.iter().skip(2) {
        let product = matrix_vector(scaled_matrix, &current);
        let next = product
            .iter()
            .zip(&previous)
            .map(|(product, previous)| 2.0 * product - previous)
            .collect::<Vec<_>>();
        for (target, value) in output.iter_mut().zip(&next) {
            *target += coefficient * value;
        }
        previous = current;
        current = next;
    }
    if output.iter().any(|value| !value.is_finite()) {
        return Err(GraphError::Numerical(
            "Chebyshev recurrence produced non-finite output".into(),
        ));
    }
    Ok(output)
}

fn matrix_vector(matrix: &[Vec<f64>], vector: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(vector)
                .map(|(left, right)| left * right)
                .sum()
        })
        .collect()
}

fn heat_chebyshev_coefficients(time: f64, lambda_max: f64, order: usize) -> Vec<f64> {
    let samples = (8 * (order + 1)).max(1_024);
    (0..=order)
        .map(|degree| {
            2.0 / samples as f64
                * (0..samples)
                    .map(|sample| {
                        let theta = std::f64::consts::PI * (sample as f64 + 0.5) / samples as f64;
                        let eigenvalue = lambda_max * (theta.cos() + 1.0) / 2.0;
                        (-time * eigenvalue).exp() * (degree as f64 * theta).cos()
                    })
                    .sum::<f64>()
        })
        .collect()
}

fn heat_grid_error(time: f64, lambda_max: f64, coefficients: &[f64], points: usize) -> f64 {
    (0..points)
        .map(|index| {
            let scaled = -1.0 + 2.0 * index as f64 / (points - 1) as f64;
            let eigenvalue = lambda_max * (scaled + 1.0) / 2.0;
            let mut previous = 1.0;
            let mut approximation = 0.5 * coefficients[0];
            if coefficients.len() > 1 {
                let mut current = scaled;
                approximation += coefficients[1] * current;
                for coefficient in coefficients.iter().skip(2) {
                    let next = 2.0 * scaled * current - previous;
                    approximation += coefficient * next;
                    previous = current;
                    current = next;
                }
            }
            (approximation - (-time * eigenvalue).exp()).abs()
        })
        .fold(0.0_f64, f64::max)
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

struct EigenDecomposition {
    values: Vec<f64>,
    vectors: Vec<Vec<f64>>,
    rotations: usize,
}

fn symmetric_eigendecomposition(input: &[Vec<f64>]) -> Result<EigenDecomposition, GraphError> {
    let size = input.len();
    let mut matrix = input.to_vec();
    let mut vectors = vec![vec![0.0; size]; size];
    for (index, row) in vectors.iter_mut().enumerate() {
        row[index] = 1.0;
    }
    let mut rotations = 0;
    loop {
        let mut selected = (0, 1);
        let mut maximum = matrix[0][1].abs();
        for (row, values) in matrix.iter().enumerate() {
            for (column, value) in values.iter().enumerate().skip(row + 1) {
                if value.abs() > maximum {
                    maximum = value.abs();
                    selected = (row, column);
                }
            }
        }
        if maximum <= 1e-13 {
            break;
        }
        if rotations == MAXIMUM_JACOBI_ROTATIONS {
            return Err(GraphError::Numerical(
                "Jacobi eigendecomposition did not converge".into(),
            ));
        }
        rotations += 1;
        let (left, right) = selected;
        let angle =
            0.5 * (2.0 * matrix[left][right]).atan2(matrix[right][right] - matrix[left][left]);
        let (sine, cosine) = angle.sin_cos();
        let diagonal_left = matrix[left][left];
        let diagonal_right = matrix[right][right];
        let cross = matrix[left][right];
        for index in 0..size {
            if index != left && index != right {
                let old_left = matrix[index][left];
                let old_right = matrix[index][right];
                matrix[index][left] = cosine * old_left - sine * old_right;
                matrix[left][index] = matrix[index][left];
                matrix[index][right] = sine * old_left + cosine * old_right;
                matrix[right][index] = matrix[index][right];
            }
            let vector_left = vectors[index][left];
            let vector_right = vectors[index][right];
            vectors[index][left] = cosine * vector_left - sine * vector_right;
            vectors[index][right] = sine * vector_left + cosine * vector_right;
        }
        matrix[left][left] = cosine.powi(2) * diagonal_left - 2.0 * sine * cosine * cross
            + sine.powi(2) * diagonal_right;
        matrix[right][right] = sine.powi(2) * diagonal_left
            + 2.0 * sine * cosine * cross
            + cosine.powi(2) * diagonal_right;
        matrix[left][right] = 0.0;
        matrix[right][left] = 0.0;
    }
    let mut modes = (0..size)
        .map(|column| {
            let mut mode = vectors.iter().map(|row| row[column]).collect::<Vec<_>>();
            if mode
                .iter()
                .find(|value| value.abs() > 1e-12)
                .is_some_and(|value| *value < 0.0)
            {
                for value in &mut mode {
                    *value = -*value;
                }
            }
            (matrix[column][column], mode)
        })
        .collect::<Vec<_>>();
    modes.sort_by(|left, right| left.0.total_cmp(&right.0));
    let (mut eigenvalues, eigenvectors): (Vec<_>, Vec<_>) = modes.into_iter().unzip();
    for value in &mut eigenvalues {
        if value.abs() <= 1e-12 {
            *value = 0.0;
        }
    }
    Ok(EigenDecomposition {
        values: eigenvalues,
        vectors: eigenvectors,
        rotations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_node_laplacian_has_zero_and_two_spectrum() {
        let decomposition =
            symmetric_eigendecomposition(&[vec![1.0, -1.0], vec![-1.0, 1.0]]).expect("spectrum");
        assert!(decomposition.values[0].abs() < 1e-12);
        assert!((decomposition.values[1] - 2.0).abs() < 1e-12);
    }
}
