use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    graph_sparse_radius_heat_workflow, GraphError, GraphNodeInput, GraphSparseRadiusHeatResult,
    GraphSparseRadiusHeatSpec,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusHeatStabilitySpec {
    pub nodes: Vec<GraphNodeInput>,
    pub radius_um: f64,
    pub time: f64,
    pub tolerance: f64,
    pub maximum_order: usize,
    pub maximum_nodes: usize,
    pub maximum_candidate_pairs: u64,
    pub maximum_edges: u64,
    pub maximum_matrix_vector_work: u64,
    pub maximum_working_bytes: u64,
    pub perturbation_replicates: usize,
    pub maximum_coordinate_jitter_um: f64,
    pub seed: u64,
    pub maximum_total_candidate_pairs: u64,
    pub maximum_total_matrix_vector_work: u64,
    pub maximum_relative_l2_change: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseRadiusHeatPerturbationResult {
    pub replicate: usize,
    pub graph_digest: String,
    pub edge_count: u64,
    pub isolated_node_count: usize,
    pub maximum_coordinate_displacement_um: f64,
    pub relative_filtered_l2_change: f64,
    pub candidate_pair_evaluations: u64,
    pub matrix_vector_work: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusHeatStabilityResult {
    pub format: String,
    pub version: u32,
    pub statistical_unit: String,
    pub reference: String,
    pub perturbation_rule: String,
    pub finite_result_policy: String,
    pub seed: u64,
    pub perturbation_replicates: usize,
    pub maximum_coordinate_jitter_um: f64,
    pub maximum_relative_l2_change_allowed: f64,
    pub baseline: GraphSparseRadiusHeatResult,
    pub perturbations: Vec<SparseRadiusHeatPerturbationResult>,
    pub total_candidate_pair_evaluations: u64,
    pub total_matrix_vector_work: u64,
    pub maximum_relative_l2_change: f64,
    pub minimum_edge_count: u64,
    pub maximum_edge_count: u64,
    pub stable_under_declared_threshold: bool,
    pub claim_status: String,
}

impl GraphSparseRadiusHeatStabilityResult {
    pub fn validate_for_spec(
        &self,
        spec: &GraphSparseRadiusHeatStabilitySpec,
    ) -> Result<(), GraphError> {
        self.baseline
            .validate_for_spec(&heat_spec(spec, spec.nodes.clone()))?;
        let total_candidates = self
            .perturbations
            .iter()
            .try_fold(self.baseline.candidate_pair_evaluations, |total, result| {
                total.checked_add(result.candidate_pair_evaluations)
            });
        let total_matvec = self
            .perturbations
            .iter()
            .try_fold(self.baseline.matrix_vector_work, |total, result| {
                total.checked_add(result.matrix_vector_work)
            });
        let observed_maximum_change = self
            .perturbations
            .iter()
            .map(|result| result.relative_filtered_l2_change)
            .fold(0.0_f64, f64::max);
        let observed_minimum_edges = self
            .perturbations
            .iter()
            .map(|result| result.edge_count)
            .fold(self.baseline.edge_count, u64::min);
        let observed_maximum_edges = self
            .perturbations
            .iter()
            .map(|result| result.edge_count)
            .fold(self.baseline.edge_count, u64::max);
        if self.format != "marklab.graph_sparse_radius_heat_stability"
            || self.version != 1
            || self.statistical_unit != "one_specimen_graph"
            || self.reference != "unperturbed_coordinates_with_fixed_node_ids_and_signals"
            || self.perturbation_rule != "sha256_uniform_independent_axis_jitter"
            || self.finite_result_policy != "reject_non_finite_input_or_output"
            || self.claim_status != "coordinate_perturbation_stability_diagnostic"
            || self.seed != spec.seed
            || self.perturbation_replicates != spec.perturbation_replicates
            || self.perturbations.len() != spec.perturbation_replicates
            || self.maximum_coordinate_jitter_um.to_bits()
                != spec.maximum_coordinate_jitter_um.to_bits()
            || self.maximum_relative_l2_change_allowed.to_bits()
                != spec.maximum_relative_l2_change.to_bits()
            || total_candidates != Some(self.total_candidate_pair_evaluations)
            || total_matvec != Some(self.total_matrix_vector_work)
            || self.total_candidate_pair_evaluations > spec.maximum_total_candidate_pairs
            || self.total_matrix_vector_work > spec.maximum_total_matrix_vector_work
            || observed_maximum_change.to_bits() != self.maximum_relative_l2_change.to_bits()
            || observed_minimum_edges != self.minimum_edge_count
            || observed_maximum_edges != self.maximum_edge_count
            || self.stable_under_declared_threshold
                != (self.maximum_relative_l2_change <= spec.maximum_relative_l2_change)
            || self
                .perturbations
                .iter()
                .enumerate()
                .any(|(index, result)| {
                    result.replicate != index
                        || result.graph_digest.len() != 64
                        || result
                            .graph_digest
                            .bytes()
                            .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
                        || result.edge_count > spec.maximum_edges
                        || result.candidate_pair_evaluations > spec.maximum_candidate_pairs
                        || result.matrix_vector_work > spec.maximum_matrix_vector_work
                        || !result.maximum_coordinate_displacement_um.is_finite()
                        || !result.relative_filtered_l2_change.is_finite()
                })
        {
            return Err(GraphError::Invalid(
                "sparse radius heat stability result does not match its typed specification".into(),
            ));
        }
        Ok(())
    }
}

pub fn graph_sparse_radius_heat_stability_workflow(
    spec: GraphSparseRadiusHeatStabilitySpec,
) -> Result<GraphSparseRadiusHeatStabilityResult, GraphError> {
    validate_controls(&spec)?;
    let run_count = (spec.perturbation_replicates as u64)
        .checked_add(1)
        .ok_or_else(|| GraphError::Invalid("stability run count overflow".into()))?;
    let planned_candidate_work = run_count
        .checked_mul(spec.maximum_candidate_pairs)
        .ok_or_else(|| GraphError::Invalid("aggregate candidate work overflow".into()))?;
    if planned_candidate_work > spec.maximum_total_candidate_pairs {
        return Err(GraphError::Invalid(
            "aggregate candidate work exceeds caller maximum".into(),
        ));
    }
    let planned_matrix_vector_work = run_count
        .checked_mul(spec.maximum_matrix_vector_work)
        .ok_or_else(|| GraphError::Invalid("aggregate matrix-vector work overflow".into()))?;
    if planned_matrix_vector_work > spec.maximum_total_matrix_vector_work {
        return Err(GraphError::Invalid(
            "aggregate matrix-vector work exceeds caller maximum".into(),
        ));
    }

    let baseline_spec = heat_spec(&spec, spec.nodes.clone());
    let baseline = graph_sparse_radius_heat_workflow(baseline_spec)?;
    let baseline_norm = baseline
        .filtered_signal
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if !baseline_norm.is_finite() {
        return Err(GraphError::Numerical(
            "baseline filtered signal norm is non-finite".into(),
        ));
    }

    let mut total_candidate_pair_evaluations = baseline.candidate_pair_evaluations;
    let mut total_matrix_vector_work = baseline.matrix_vector_work;
    let mut minimum_edge_count = baseline.edge_count;
    let mut maximum_edge_count = baseline.edge_count;
    let mut maximum_relative_l2_change = 0.0_f64;
    let mut perturbations = Vec::with_capacity(spec.perturbation_replicates);
    for replicate in 0..spec.perturbation_replicates {
        let (nodes, maximum_coordinate_displacement_um) = perturb_nodes(&spec, replicate)?;
        let result = graph_sparse_radius_heat_workflow(heat_spec(&spec, nodes))?;
        let difference_norm = result
            .filtered_signal
            .iter()
            .zip(&baseline.filtered_signal)
            .map(|(perturbed, reference)| {
                let difference = perturbed - reference;
                difference * difference
            })
            .sum::<f64>()
            .sqrt();
        let relative_filtered_l2_change = if baseline_norm == 0.0 {
            if difference_norm == 0.0 {
                0.0
            } else {
                return Err(GraphError::Numerical(
                    "relative stability is undefined for a zero baseline signal".into(),
                ));
            }
        } else {
            difference_norm / baseline_norm
        };
        if !relative_filtered_l2_change.is_finite() {
            return Err(GraphError::Numerical(
                "relative filtered signal change is non-finite".into(),
            ));
        }
        total_candidate_pair_evaluations = total_candidate_pair_evaluations
            .checked_add(result.candidate_pair_evaluations)
            .ok_or_else(|| GraphError::Invalid("aggregate candidate work overflow".into()))?;
        total_matrix_vector_work = total_matrix_vector_work
            .checked_add(result.matrix_vector_work)
            .ok_or_else(|| GraphError::Invalid("aggregate matrix-vector work overflow".into()))?;
        if total_candidate_pair_evaluations > spec.maximum_total_candidate_pairs {
            return Err(GraphError::Invalid(
                "aggregate candidate work exceeds caller maximum".into(),
            ));
        }
        if total_matrix_vector_work > spec.maximum_total_matrix_vector_work {
            return Err(GraphError::Invalid(
                "aggregate matrix-vector work exceeds caller maximum".into(),
            ));
        }
        minimum_edge_count = minimum_edge_count.min(result.edge_count);
        maximum_edge_count = maximum_edge_count.max(result.edge_count);
        maximum_relative_l2_change = maximum_relative_l2_change.max(relative_filtered_l2_change);
        perturbations.push(SparseRadiusHeatPerturbationResult {
            replicate,
            graph_digest: result.graph_digest,
            edge_count: result.edge_count,
            isolated_node_count: result.isolated_node_count,
            maximum_coordinate_displacement_um,
            relative_filtered_l2_change,
            candidate_pair_evaluations: result.candidate_pair_evaluations,
            matrix_vector_work: result.matrix_vector_work,
        });
    }

    Ok(GraphSparseRadiusHeatStabilityResult {
        format: "marklab.graph_sparse_radius_heat_stability".into(),
        version: 1,
        statistical_unit: "one_specimen_graph".into(),
        reference: "unperturbed_coordinates_with_fixed_node_ids_and_signals".into(),
        perturbation_rule: "sha256_uniform_independent_axis_jitter".into(),
        finite_result_policy: "reject_non_finite_input_or_output".into(),
        seed: spec.seed,
        perturbation_replicates: spec.perturbation_replicates,
        maximum_coordinate_jitter_um: spec.maximum_coordinate_jitter_um,
        maximum_relative_l2_change_allowed: spec.maximum_relative_l2_change,
        baseline,
        perturbations,
        total_candidate_pair_evaluations,
        total_matrix_vector_work,
        maximum_relative_l2_change,
        minimum_edge_count,
        maximum_edge_count,
        stable_under_declared_threshold: maximum_relative_l2_change
            <= spec.maximum_relative_l2_change,
        claim_status: "coordinate_perturbation_stability_diagnostic".into(),
    })
}

fn validate_controls(spec: &GraphSparseRadiusHeatStabilitySpec) -> Result<(), GraphError> {
    if !(1..=64).contains(&spec.perturbation_replicates)
        || !spec.maximum_coordinate_jitter_um.is_finite()
        || spec.maximum_coordinate_jitter_um < 0.0
        || !spec.maximum_relative_l2_change.is_finite()
        || spec.maximum_relative_l2_change < 0.0
        || spec.maximum_total_candidate_pairs == 0
        || spec.maximum_total_matrix_vector_work == 0
    {
        return Err(GraphError::Invalid(
            "sparse heat stability controls or aggregate resource limits are invalid".into(),
        ));
    }
    Ok(())
}

fn heat_spec(
    spec: &GraphSparseRadiusHeatStabilitySpec,
    nodes: Vec<GraphNodeInput>,
) -> GraphSparseRadiusHeatSpec {
    GraphSparseRadiusHeatSpec {
        nodes,
        radius_um: spec.radius_um,
        time: spec.time,
        tolerance: spec.tolerance,
        maximum_order: spec.maximum_order,
        maximum_nodes: spec.maximum_nodes,
        maximum_candidate_pairs: spec.maximum_candidate_pairs,
        maximum_edges: spec.maximum_edges,
        maximum_matrix_vector_work: spec.maximum_matrix_vector_work,
        maximum_working_bytes: spec.maximum_working_bytes,
    }
}

fn perturb_nodes(
    spec: &GraphSparseRadiusHeatStabilitySpec,
    replicate: usize,
) -> Result<(Vec<GraphNodeInput>, f64), GraphError> {
    let mut maximum_displacement = 0.0_f64;
    let mut nodes = spec.nodes.clone();
    for node in &mut nodes {
        let mut squared_displacement = 0.0;
        for axis in 0..2 {
            let jitter = deterministic_axis_jitter(
                spec.seed,
                replicate,
                &node.id,
                axis,
                spec.maximum_coordinate_jitter_um,
            );
            node.coordinates_um[axis] += jitter;
            squared_displacement += jitter * jitter;
        }
        if node.coordinates_um.iter().any(|value| !value.is_finite()) {
            return Err(GraphError::Invalid(
                "coordinate perturbation produced a non-finite coordinate".into(),
            ));
        }
        maximum_displacement = maximum_displacement.max(squared_displacement.sqrt());
    }
    Ok((nodes, maximum_displacement))
}

fn deterministic_axis_jitter(
    seed: u64,
    replicate: usize,
    node_id: &str,
    axis: usize,
    maximum: f64,
) -> f64 {
    if maximum == 0.0 {
        return 0.0;
    }
    let mut digest = Sha256::new();
    digest.update(b"marklab-sparse-radius-heat-stability-jitter-v1\0");
    digest.update(seed.to_be_bytes());
    digest.update((replicate as u64).to_be_bytes());
    digest.update((node_id.len() as u64).to_be_bytes());
    digest.update(node_id.as_bytes());
    digest.update((axis as u64).to_be_bytes());
    let bytes = digest.finalize();
    let bits = u64::from_be_bytes(
        bytes[..8]
            .try_into()
            .expect("SHA-256 prefix has eight bytes"),
    );
    let unit = (bits >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64));
    (2.0 * unit - 1.0) * maximum
}
