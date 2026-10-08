use serde::{Deserialize, Serialize};

use crate::{
    graph_sparse_radius_diffusion_wavelet_workflow, graph_sparse_radius_heat_workflow, GraphError,
    GraphNodeInput, GraphSparseRadiusDiffusionWaveletSpec, GraphSparseRadiusHeatSpec,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusScatteringSpec {
    pub nodes: Vec<GraphNodeInput>,
    pub radius_um: f64,
    pub times: Vec<f64>,
    pub scattering_order: usize,
    pub tolerance: f64,
    pub maximum_order: usize,
    pub maximum_nodes: usize,
    pub maximum_candidate_pairs: u64,
    pub maximum_edges: u64,
    pub maximum_matrix_vector_work: u64,
    pub maximum_working_bytes: u64,
    pub maximum_retained_bytes: u64,
    pub maximum_total_candidate_pairs: u64,
    pub maximum_total_matrix_vector_work: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseRadiusFirstOrderScatteringResult {
    pub level: usize,
    pub time: f64,
    pub mean_absolute: f64,
    pub energy: f64,
    pub approximation_l2_error_bound: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseRadiusSecondOrderScatteringResult {
    pub first_level: usize,
    pub second_level: usize,
    pub first_time: f64,
    pub second_time: f64,
    pub mean_absolute: f64,
    pub energy: f64,
    pub approximation_l2_error_bound: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusScatteringResult {
    pub format: String,
    pub version: u32,
    pub statistical_unit: String,
    pub graph_digest: String,
    pub graph_rule: String,
    pub wavelet: String,
    pub nonlinearity: String,
    pub aggregation: String,
    pub finite_result_policy: String,
    pub node_count: usize,
    pub edge_count: u64,
    pub isolated_node_count: usize,
    pub radius_um: f64,
    pub times: Vec<f64>,
    pub scattering_order: usize,
    pub zero_order_mean: f64,
    pub zero_order_energy: f64,
    pub first_order: Vec<SparseRadiusFirstOrderScatteringResult>,
    pub second_order: Vec<SparseRadiusSecondOrderScatteringResult>,
    pub coarse_mean_absolute: f64,
    pub coarse_energy: f64,
    pub heat_applications: u64,
    pub total_candidate_pair_evaluations: u64,
    pub total_matrix_vector_work: u64,
    pub peak_working_bytes: u64,
    pub retained_bytes: u64,
    pub claim_status: String,
}

impl GraphSparseRadiusScatteringResult {
    pub fn validate_for_spec(
        &self,
        spec: &GraphSparseRadiusScatteringSpec,
    ) -> Result<(), GraphError> {
        let expected_second = if spec.scattering_order == 2 {
            spec.times.len() * spec.times.len().saturating_sub(1) / 2
        } else {
            0
        };
        if self.format != "marklab.graph_sparse_radius_scattering"
            || self.version != 1
            || self.statistical_unit != "one_specimen_graph"
            || self.graph_rule != "uniform_cell_exact_physical_radius"
            || self.wavelet != "telescoping_sparse_heat_filter_bank"
            || self.nonlinearity != "pointwise_absolute_value"
            || self.aggregation != "graph_node_mean_and_energy"
            || self.finite_result_policy != "reject_non_finite_input_or_output"
            || self.claim_status != "experimental_sparse_graph_scattering"
            || self.node_count != spec.nodes.len()
            || self.radius_um.to_bits() != spec.radius_um.to_bits()
            || self.scattering_order != spec.scattering_order
            || self.times.len() != spec.times.len()
            || self
                .times
                .iter()
                .zip(&spec.times)
                .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
            || self.first_order.len() != spec.times.len()
            || self.second_order.len() != expected_second
            || self.heat_applications != planned_heat_applications(spec)
            || self.total_candidate_pair_evaluations > spec.maximum_total_candidate_pairs
            || self.total_matrix_vector_work > spec.maximum_total_matrix_vector_work
            || self.peak_working_bytes > spec.maximum_working_bytes
            || self.retained_bytes > spec.maximum_retained_bytes
            || self.graph_digest.len() != 64
            || self
                .graph_digest
                .bytes()
                .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            || [
                self.zero_order_mean,
                self.zero_order_energy,
                self.coarse_mean_absolute,
                self.coarse_energy,
            ]
            .iter()
            .any(|value| !value.is_finite())
            || self.first_order.iter().enumerate().any(|(level, row)| {
                row.level != level
                    || row.time.to_bits() != spec.times[level].to_bits()
                    || [
                        row.mean_absolute,
                        row.energy,
                        row.approximation_l2_error_bound,
                    ]
                    .iter()
                    .any(|value| !value.is_finite())
            })
            || self.second_order.iter().any(|row| {
                row.first_level >= row.second_level
                    || row.second_level >= spec.times.len()
                    || row.first_time.to_bits() != spec.times[row.first_level].to_bits()
                    || row.second_time.to_bits() != spec.times[row.second_level].to_bits()
                    || [
                        row.mean_absolute,
                        row.energy,
                        row.approximation_l2_error_bound,
                    ]
                    .iter()
                    .any(|value| !value.is_finite())
            })
        {
            return Err(GraphError::Invalid(
                "sparse radius scattering result does not match its typed specification".into(),
            ));
        }
        Ok(())
    }
}

pub fn graph_sparse_radius_scattering_workflow(
    spec: GraphSparseRadiusScatteringSpec,
) -> Result<GraphSparseRadiusScatteringResult, GraphError> {
    validate_controls(&spec)?;
    let heat_applications = planned_heat_applications(&spec);
    admit_total_work(&spec, heat_applications)?;
    let retained_bytes = conservative_retained_bytes(spec.nodes.len(), spec.times.len())?;
    if retained_bytes > spec.maximum_retained_bytes {
        return Err(GraphError::Invalid(
            "sparse scattering retained bytes exceed caller maximum".into(),
        ));
    }

    let wavelet = graph_sparse_radius_diffusion_wavelet_workflow(wavelet_spec(&spec)?)?;
    let mut total_candidate_pair_evaluations = wavelet.total_candidate_pair_evaluations;
    let mut total_matrix_vector_work = wavelet.total_matrix_vector_work;
    let mut peak_working_bytes = wavelet.peak_working_bytes;
    let original_signal = spec
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let first_moduli = wavelet
        .scales
        .iter()
        .map(|scale| {
            scale
                .detail_signal
                .iter()
                .map(|value| value.abs())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let first_order = wavelet
        .scales
        .iter()
        .zip(&first_moduli)
        .map(|(scale, modulus)| SparseRadiusFirstOrderScatteringResult {
            level: scale.level,
            time: scale.time,
            mean_absolute: mean(modulus),
            energy: energy(modulus),
            approximation_l2_error_bound: scale.approximation_l2_error_bound,
        })
        .collect::<Vec<_>>();

    let mut canonical_nodes = spec.nodes.clone();
    canonical_nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let mut second_order = Vec::new();
    if spec.scattering_order == 2 {
        for (first_level, first_modulus) in first_moduli
            .iter()
            .enumerate()
            .take(spec.times.len().saturating_sub(1))
        {
            for (node, value) in canonical_nodes.iter_mut().zip(first_modulus) {
                node.signal = *value;
            }
            let mut previous = None::<(Vec<f64>, f64)>;
            for second_level in first_level..spec.times.len() {
                let result = graph_sparse_radius_heat_workflow(heat_spec(
                    &spec,
                    canonical_nodes.clone(),
                    spec.times[second_level],
                ))?;
                if result.graph_digest != wavelet.graph_digest {
                    return Err(GraphError::Numerical(
                        "sparse scattering propagation changed the exact graph".into(),
                    ));
                }
                total_candidate_pair_evaluations = total_candidate_pair_evaluations
                    .checked_add(result.candidate_pair_evaluations)
                    .ok_or_else(|| {
                        GraphError::Invalid("aggregate candidate work overflow".into())
                    })?;
                total_matrix_vector_work = total_matrix_vector_work
                    .checked_add(result.matrix_vector_work)
                    .ok_or_else(|| {
                        GraphError::Invalid("aggregate matrix-vector work overflow".into())
                    })?;
                peak_working_bytes = peak_working_bytes.max(result.working_bytes);
                if let Some((less_diffused, previous_bound)) = previous {
                    let modulus = less_diffused
                        .iter()
                        .zip(&result.filtered_signal)
                        .map(|(left, right)| (left - right).abs())
                        .collect::<Vec<_>>();
                    second_order.push(SparseRadiusSecondOrderScatteringResult {
                        first_level,
                        second_level,
                        first_time: spec.times[first_level],
                        second_time: spec.times[second_level],
                        mean_absolute: mean(&modulus),
                        energy: energy(&modulus),
                        approximation_l2_error_bound: previous_bound
                            + result.diagnostic_l2_error_bound,
                    });
                }
                previous = Some((result.filtered_signal, result.diagnostic_l2_error_bound));
            }
        }
    }
    if total_candidate_pair_evaluations > spec.maximum_total_candidate_pairs
        || total_matrix_vector_work > spec.maximum_total_matrix_vector_work
    {
        return Err(GraphError::Invalid(
            "sparse scattering observed work exceeds caller maximum".into(),
        ));
    }
    let result = GraphSparseRadiusScatteringResult {
        format: "marklab.graph_sparse_radius_scattering".into(),
        version: 1,
        statistical_unit: "one_specimen_graph".into(),
        graph_digest: wavelet.graph_digest,
        graph_rule: "uniform_cell_exact_physical_radius".into(),
        wavelet: "telescoping_sparse_heat_filter_bank".into(),
        nonlinearity: "pointwise_absolute_value".into(),
        aggregation: "graph_node_mean_and_energy".into(),
        finite_result_policy: "reject_non_finite_input_or_output".into(),
        node_count: wavelet.node_count,
        edge_count: wavelet.edge_count,
        isolated_node_count: wavelet.isolated_node_count,
        radius_um: spec.radius_um,
        times: spec.times.clone(),
        scattering_order: spec.scattering_order,
        zero_order_mean: mean(&original_signal),
        zero_order_energy: energy(&original_signal),
        first_order,
        second_order,
        coarse_mean_absolute: mean_absolute(&wavelet.coarse_signal),
        coarse_energy: wavelet.coarse_energy,
        heat_applications,
        total_candidate_pair_evaluations,
        total_matrix_vector_work,
        peak_working_bytes,
        retained_bytes,
        claim_status: "experimental_sparse_graph_scattering".into(),
    };
    result.validate_for_spec(&spec)?;
    Ok(result)
}

fn validate_controls(spec: &GraphSparseRadiusScatteringSpec) -> Result<(), GraphError> {
    if spec.times.is_empty()
        || spec.times.len() > 16
        || spec
            .times
            .iter()
            .any(|time| !time.is_finite() || *time <= 0.0)
        || spec.times.windows(2).any(|pair| pair[0] >= pair[1])
        || !(1..=2).contains(&spec.scattering_order)
        || spec.maximum_retained_bytes == 0
        || spec.maximum_total_candidate_pairs == 0
        || spec.maximum_total_matrix_vector_work == 0
    {
        return Err(GraphError::Invalid(
            "sparse scattering controls or aggregate resource limits are invalid".into(),
        ));
    }
    Ok(())
}

fn planned_heat_applications(spec: &GraphSparseRadiusScatteringSpec) -> u64 {
    let scales = spec.times.len() as u64;
    if spec.scattering_order == 1 {
        scales
    } else {
        scales + scales.saturating_sub(1) * (scales + 2) / 2
    }
}

fn admit_total_work(
    spec: &GraphSparseRadiusScatteringSpec,
    heat_applications: u64,
) -> Result<(), GraphError> {
    let candidates = heat_applications
        .checked_mul(spec.maximum_candidate_pairs)
        .ok_or_else(|| GraphError::Invalid("aggregate candidate work overflow".into()))?;
    if candidates > spec.maximum_total_candidate_pairs {
        return Err(GraphError::Invalid(
            "aggregate candidate work exceeds caller maximum".into(),
        ));
    }
    let matvec = heat_applications
        .checked_mul(spec.maximum_matrix_vector_work)
        .ok_or_else(|| GraphError::Invalid("aggregate matrix-vector work overflow".into()))?;
    if matvec > spec.maximum_total_matrix_vector_work {
        return Err(GraphError::Invalid(
            "aggregate matrix-vector work exceeds caller maximum".into(),
        ));
    }
    Ok(())
}

fn wavelet_spec(
    spec: &GraphSparseRadiusScatteringSpec,
) -> Result<GraphSparseRadiusDiffusionWaveletSpec, GraphError> {
    let scales = spec.times.len() as u64;
    Ok(GraphSparseRadiusDiffusionWaveletSpec {
        nodes: spec.nodes.clone(),
        radius_um: spec.radius_um,
        times: spec.times.clone(),
        tolerance: spec.tolerance,
        maximum_order: spec.maximum_order,
        maximum_nodes: spec.maximum_nodes,
        maximum_candidate_pairs: spec.maximum_candidate_pairs,
        maximum_edges: spec.maximum_edges,
        maximum_matrix_vector_work: spec.maximum_matrix_vector_work,
        maximum_working_bytes: spec.maximum_working_bytes,
        maximum_retained_bytes: spec.maximum_retained_bytes,
        maximum_total_candidate_pairs: scales
            .checked_mul(spec.maximum_candidate_pairs)
            .ok_or_else(|| GraphError::Invalid("first-order candidate work overflow".into()))?,
        maximum_total_matrix_vector_work: scales
            .checked_mul(spec.maximum_matrix_vector_work)
            .ok_or_else(|| GraphError::Invalid("first-order matrix-vector work overflow".into()))?,
    })
}

fn heat_spec(
    spec: &GraphSparseRadiusScatteringSpec,
    nodes: Vec<GraphNodeInput>,
    time: f64,
) -> GraphSparseRadiusHeatSpec {
    GraphSparseRadiusHeatSpec {
        nodes,
        radius_um: spec.radius_um,
        time,
        tolerance: spec.tolerance,
        maximum_order: spec.maximum_order,
        maximum_nodes: spec.maximum_nodes,
        maximum_candidate_pairs: spec.maximum_candidate_pairs,
        maximum_edges: spec.maximum_edges,
        maximum_matrix_vector_work: spec.maximum_matrix_vector_work,
        maximum_working_bytes: spec.maximum_working_bytes,
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn mean_absolute(values: &[f64]) -> f64 {
    values.iter().map(|value| value.abs()).sum::<f64>() / values.len() as f64
}

fn energy(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum()
}

fn conservative_retained_bytes(nodes: usize, scales: usize) -> Result<u64, GraphError> {
    (nodes as u64)
        .checked_mul(scales as u64)
        .and_then(|value| value.checked_mul(96))
        .and_then(|value| value.checked_add((nodes as u64).checked_mul(256)?))
        .and_then(|value| value.checked_add((scales as u64).checked_mul(2_048)?))
        .ok_or_else(|| GraphError::Invalid("sparse scattering retained byte count overflow".into()))
}
