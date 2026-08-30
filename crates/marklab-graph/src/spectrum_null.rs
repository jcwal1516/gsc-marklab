use std::collections::BTreeMap;

use marklab_numerics::extreme_rank_length_envelope;
use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

use crate::{graph_spectral_workflow, GraphError, GraphSpectralResult, GraphSpectralSpec};

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
