use serde::{Deserialize, Serialize};

use crate::{
    graph_sparse_radius_heat_workflow, GraphError, GraphNodeInput, GraphSparseRadiusHeatSpec,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusDiffusionWaveletSpec {
    pub nodes: Vec<GraphNodeInput>,
    pub radius_um: f64,
    pub times: Vec<f64>,
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
pub struct SparseRadiusDiffusionWaveletScaleResult {
    pub level: usize,
    pub time: f64,
    pub selected_order: usize,
    pub detail_signal: Vec<f64>,
    pub filtered_energy: f64,
    pub detail_energy: f64,
    pub approximation_l2_error_bound: f64,
    pub candidate_pair_evaluations: u64,
    pub matrix_vector_work: u64,
    pub working_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusDiffusionWaveletResult {
    pub format: String,
    pub version: u32,
    pub statistical_unit: String,
    pub graph_digest: String,
    pub graph_rule: String,
    pub transform: String,
    pub finite_result_policy: String,
    pub node_count: usize,
    pub edge_count: u64,
    pub isolated_node_count: usize,
    pub radius_um: f64,
    pub tolerance: f64,
    pub times: Vec<f64>,
    pub scales: Vec<SparseRadiusDiffusionWaveletScaleResult>,
    pub coarse_signal: Vec<f64>,
    pub coarse_energy: f64,
    pub reconstruction_max_abs_error: f64,
    pub total_candidate_pair_evaluations: u64,
    pub total_matrix_vector_work: u64,
    pub peak_working_bytes: u64,
    pub retained_bytes: u64,
    pub claim_status: String,
}

impl GraphSparseRadiusDiffusionWaveletResult {
    pub fn validate_for_spec(
        &self,
        spec: &GraphSparseRadiusDiffusionWaveletSpec,
    ) -> Result<(), GraphError> {
        let total_candidates = self.scales.iter().try_fold(0_u64, |total, scale| {
            total.checked_add(scale.candidate_pair_evaluations)
        });
        let total_matvec = self.scales.iter().try_fold(0_u64, |total, scale| {
            total.checked_add(scale.matrix_vector_work)
        });
        let peak_working = self
            .scales
            .iter()
            .map(|scale| scale.working_bytes)
            .max()
            .unwrap_or(0);
        if self.format != "marklab.graph_sparse_radius_diffusion_wavelet"
            || self.version != 1
            || self.statistical_unit != "one_specimen_graph"
            || self.graph_rule != "uniform_cell_exact_physical_radius"
            || self.transform != "telescoping_sparse_heat_filter_bank"
            || self.finite_result_policy != "reject_non_finite_input_or_output"
            || self.claim_status != "experimental_sparse_diffusion_wavelet"
            || self.node_count != spec.nodes.len()
            || self.radius_um.to_bits() != spec.radius_um.to_bits()
            || self.tolerance.to_bits() != spec.tolerance.to_bits()
            || self.times.len() != spec.times.len()
            || self
                .times
                .iter()
                .zip(&spec.times)
                .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
            || self.scales.len() != spec.times.len()
            || self.coarse_signal.len() != spec.nodes.len()
            || total_candidates != Some(self.total_candidate_pair_evaluations)
            || total_matvec != Some(self.total_matrix_vector_work)
            || self.total_candidate_pair_evaluations > spec.maximum_total_candidate_pairs
            || self.total_matrix_vector_work > spec.maximum_total_matrix_vector_work
            || peak_working != self.peak_working_bytes
            || self.peak_working_bytes > spec.maximum_working_bytes
            || self.retained_bytes > spec.maximum_retained_bytes
            || self.graph_digest.len() != 64
            || self
                .graph_digest
                .bytes()
                .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            || self.scales.iter().enumerate().any(|(level, scale)| {
                scale.level != level
                    || scale.time.to_bits() != spec.times[level].to_bits()
                    || scale.selected_order == 0
                    || scale.selected_order > spec.maximum_order
                    || scale.detail_signal.len() != spec.nodes.len()
                    || scale.candidate_pair_evaluations > spec.maximum_candidate_pairs
                    || scale.matrix_vector_work > spec.maximum_matrix_vector_work
                    || scale.working_bytes > spec.maximum_working_bytes
                    || [
                        scale.filtered_energy,
                        scale.detail_energy,
                        scale.approximation_l2_error_bound,
                    ]
                    .iter()
                    .chain(&scale.detail_signal)
                    .any(|value| !value.is_finite())
            })
            || [self.coarse_energy, self.reconstruction_max_abs_error]
                .iter()
                .chain(&self.coarse_signal)
                .any(|value| !value.is_finite())
        {
            return Err(GraphError::Invalid(
                "sparse radius diffusion wavelet result does not match its typed specification"
                    .into(),
            ));
        }
        Ok(())
    }
}

pub fn graph_sparse_radius_diffusion_wavelet_workflow(
    spec: GraphSparseRadiusDiffusionWaveletSpec,
) -> Result<GraphSparseRadiusDiffusionWaveletResult, GraphError> {
    validate_controls(&spec)?;
    let scale_count = spec.times.len() as u64;
    let planned_candidates = scale_count
        .checked_mul(spec.maximum_candidate_pairs)
        .ok_or_else(|| GraphError::Invalid("aggregate candidate work overflow".into()))?;
    if planned_candidates > spec.maximum_total_candidate_pairs {
        return Err(GraphError::Invalid(
            "aggregate candidate work exceeds caller maximum".into(),
        ));
    }
    let planned_matvec = scale_count
        .checked_mul(spec.maximum_matrix_vector_work)
        .ok_or_else(|| GraphError::Invalid("aggregate matrix-vector work overflow".into()))?;
    if planned_matvec > spec.maximum_total_matrix_vector_work {
        return Err(GraphError::Invalid(
            "aggregate matrix-vector work exceeds caller maximum".into(),
        ));
    }
    let retained_bytes = conservative_retained_bytes(spec.nodes.len(), spec.times.len())?;
    if retained_bytes > spec.maximum_retained_bytes {
        return Err(GraphError::Invalid(
            "sparse diffusion wavelet retained bytes exceed caller maximum".into(),
        ));
    }

    let mut canonical_nodes = spec.nodes.clone();
    canonical_nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let original = canonical_nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let mut previous = original.clone();
    let mut scales = Vec::with_capacity(spec.times.len());
    let mut graph_digest = None;
    let mut edge_count = None;
    let mut isolated_node_count = None;
    let mut total_candidate_pair_evaluations = 0_u64;
    let mut total_matrix_vector_work = 0_u64;
    let mut peak_working_bytes = 0_u64;
    for (level, &time) in spec.times.iter().enumerate() {
        let result = graph_sparse_radius_heat_workflow(heat_spec(&spec, time))?;
        if graph_digest
            .as_ref()
            .is_some_and(|digest| digest != &result.graph_digest)
            || edge_count.is_some_and(|edges| edges != result.edge_count)
            || isolated_node_count.is_some_and(|isolates| isolates != result.isolated_node_count)
        {
            return Err(GraphError::Numerical(
                "sparse wavelet scales did not retain one exact graph".into(),
            ));
        }
        graph_digest.get_or_insert_with(|| result.graph_digest.clone());
        edge_count.get_or_insert(result.edge_count);
        isolated_node_count.get_or_insert(result.isolated_node_count);
        let detail_signal = previous
            .iter()
            .zip(&result.filtered_signal)
            .map(|(less_diffused, more_diffused)| less_diffused - more_diffused)
            .collect::<Vec<_>>();
        let filtered_energy = squared_l2(&result.filtered_signal);
        let detail_energy = squared_l2(&detail_signal);
        if !filtered_energy.is_finite() || !detail_energy.is_finite() {
            return Err(GraphError::Numerical(
                "sparse wavelet energy is non-finite".into(),
            ));
        }
        total_candidate_pair_evaluations = total_candidate_pair_evaluations
            .checked_add(result.candidate_pair_evaluations)
            .ok_or_else(|| GraphError::Invalid("aggregate candidate work overflow".into()))?;
        total_matrix_vector_work = total_matrix_vector_work
            .checked_add(result.matrix_vector_work)
            .ok_or_else(|| GraphError::Invalid("aggregate matrix-vector work overflow".into()))?;
        peak_working_bytes = peak_working_bytes.max(result.working_bytes);
        previous.clone_from(&result.filtered_signal);
        scales.push(SparseRadiusDiffusionWaveletScaleResult {
            level,
            time,
            selected_order: result.selected_order,
            detail_signal,
            filtered_energy,
            detail_energy,
            approximation_l2_error_bound: result.diagnostic_l2_error_bound,
            candidate_pair_evaluations: result.candidate_pair_evaluations,
            matrix_vector_work: result.matrix_vector_work,
            working_bytes: result.working_bytes,
        });
    }
    let coarse_signal = previous;
    let coarse_energy = squared_l2(&coarse_signal);
    let mut reconstructed_signal = coarse_signal.clone();
    for scale in &scales {
        for (reconstructed, detail) in reconstructed_signal.iter_mut().zip(&scale.detail_signal) {
            *reconstructed += detail;
        }
    }
    let reconstruction_max_abs_error = reconstructed_signal
        .iter()
        .zip(&original)
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0_f64, f64::max);
    if !coarse_energy.is_finite() || !reconstruction_max_abs_error.is_finite() {
        return Err(GraphError::Numerical(
            "sparse wavelet reconstruction is non-finite".into(),
        ));
    }

    Ok(GraphSparseRadiusDiffusionWaveletResult {
        format: "marklab.graph_sparse_radius_diffusion_wavelet".into(),
        version: 1,
        statistical_unit: "one_specimen_graph".into(),
        graph_digest: graph_digest.expect("at least one scale is validated"),
        graph_rule: "uniform_cell_exact_physical_radius".into(),
        transform: "telescoping_sparse_heat_filter_bank".into(),
        finite_result_policy: "reject_non_finite_input_or_output".into(),
        node_count: spec.nodes.len(),
        edge_count: edge_count.expect("at least one scale is validated"),
        isolated_node_count: isolated_node_count.expect("at least one scale is validated"),
        radius_um: spec.radius_um,
        tolerance: spec.tolerance,
        times: spec.times,
        scales,
        coarse_signal,
        coarse_energy,
        reconstruction_max_abs_error,
        total_candidate_pair_evaluations,
        total_matrix_vector_work,
        peak_working_bytes,
        retained_bytes,
        claim_status: "experimental_sparse_diffusion_wavelet".into(),
    })
}

fn validate_controls(spec: &GraphSparseRadiusDiffusionWaveletSpec) -> Result<(), GraphError> {
    if spec.times.is_empty()
        || spec.times.len() > 16
        || spec
            .times
            .iter()
            .any(|time| !time.is_finite() || *time <= 0.0)
        || spec.times.windows(2).any(|pair| pair[0] >= pair[1])
        || spec.maximum_retained_bytes == 0
        || spec.maximum_total_candidate_pairs == 0
        || spec.maximum_total_matrix_vector_work == 0
    {
        return Err(GraphError::Invalid(
            "sparse diffusion wavelet controls or aggregate resource limits are invalid".into(),
        ));
    }
    Ok(())
}

fn heat_spec(spec: &GraphSparseRadiusDiffusionWaveletSpec, time: f64) -> GraphSparseRadiusHeatSpec {
    GraphSparseRadiusHeatSpec {
        nodes: spec.nodes.clone(),
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

fn squared_l2(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum()
}

fn conservative_retained_bytes(nodes: usize, scales: usize) -> Result<u64, GraphError> {
    (nodes as u64)
        .checked_mul(scales as u64)
        .and_then(|value| value.checked_mul(64))
        .and_then(|value| value.checked_add((nodes as u64).checked_mul(64)?))
        .and_then(|value| value.checked_add((scales as u64).checked_mul(2_048)?))
        .ok_or_else(|| GraphError::Invalid("sparse wavelet retained byte count overflow".into()))
}
