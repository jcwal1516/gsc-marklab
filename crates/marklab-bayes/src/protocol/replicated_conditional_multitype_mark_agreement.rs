//! Typed messages and scientific checks for replicated conditional multitype mark agreement.

use super::posterior_validation::summary_valid;
use super::replicated_arbitrary_window_lgcp_agreement::compare_scalar;
use super::replicated_arbitrary_window_lgcp_agreement::compare_vector;
use super::replicated_arbitrary_window_lgcp_agreement::ScalarAgreement;
use super::replicated_arbitrary_window_lgcp_agreement::VectorAgreement;
use super::replicated_conditional_multitype_mark::Output as FitOutput;
use crate::backend_versions::JAX_VERSION;
use crate::AgreementStatus;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairRow {
    pub type_a: String,
    pub type_b: String,
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatientPairRow {
    pub patient_id: String,
    pub group: String,
    pub type_a: String,
    pub type_b: String,
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScaleRow {
    pub level: String,
    pub component: String,
    pub scale: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternRow {
    pub pattern_id: String,
    pub observed_same_type_edges: u64,
    pub expected_same_type_edges: McseScalarSummary,
    pub observed_type_counts: Vec<u64>,
    pub expected_type_counts: Vec<McseScalarSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreRows {
    pub independent_pattern_label_composite_log_score: f64,
    pub spatial_hierarchical_composite_log_score: McseScalarSummary,
    pub mean_composite_log_score_improvement: f64,
}

#[derive(Debug, Deserialize)]
pub struct FitView {
    pub input_sha256: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub baseline_affinities: Vec<PairRow>,
    pub group_affinity_shifts: Vec<PairRow>,
    pub patient_affinities: Vec<PatientPairRow>,
    pub hierarchy_scales: Vec<ScaleRow>,
    pub comparison: ScoreRows,
    pub pattern_checks: Vec<PatternRow>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub maximum_tree_depth: u32,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub baseline_affinities: Vec<PairRow>,
    pub group_affinity_shifts: Vec<PairRow>,
    pub patient_affinities: Vec<PatientPairRow>,
    pub hierarchy_scales: Vec<ScaleRow>,
    pub comparison: ScoreRows,
    pub pattern_checks: Vec<PatternRow>,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub baseline_affinities: VectorAgreement,
    pub group_affinity_shifts: VectorAgreement,
    pub patient_affinities: VectorAgreement,
    pub hierarchy_scales: VectorAgreement,
    pub composite_log_score: ScalarAgreement,
    pub pattern_same_type_edges: VectorAgreement,
    pub pattern_type_counts: VectorAgreement,
    pub all_pass: bool,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub pymc: FitOutput,
    pub numpyro: NumpyroResult,
    pub comparison: Comparison,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: [&'static str; 5],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

pub fn validate_numpyro(
    result: &NumpyroResult,
    pymc: &FitView,
    source_request_sha256: &str,
    request_sha256: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesError> {
    let rows_valid = result
        .baseline_affinities
        .iter()
        .chain(&result.group_affinity_shifts)
        .map(|row| &row.contrast)
        .chain(result.patient_affinities.iter().map(|row| &row.contrast))
        .chain(result.hierarchy_scales.iter().map(|row| &row.scale))
        .chain(result.pattern_checks.iter().flat_map(|row| {
            std::iter::once(&row.expected_same_type_edges).chain(row.expected_type_counts.iter())
        }))
        .chain(std::iter::once(
            &result.comparison.spatial_hierarchical_composite_log_score,
        ))
        .all(summary_valid);
    let identities_valid = rows_match(&result.baseline_affinities, &pymc.baseline_affinities)
        && rows_match(&result.group_affinity_shifts, &pymc.group_affinity_shifts)
        && result
            .patient_affinities
            .iter()
            .zip(&pymc.patient_affinities)
            .all(|(left, right)| {
                left.patient_id == right.patient_id
                    && left.group == right.group
                    && left.type_a == right.type_a
                    && left.type_b == right.type_b
            })
        && result
            .hierarchy_scales
            .iter()
            .zip(&pymc.hierarchy_scales)
            .all(|(left, right)| left.level == right.level && left.component == right.component)
        && result
            .pattern_checks
            .iter()
            .zip(&pymc.pattern_checks)
            .all(|(left, right)| {
                left.pattern_id == right.pattern_id
                    && left.observed_same_type_edges == right.observed_same_type_edges
                    && left.observed_type_counts == right.observed_type_counts
                    && left.expected_type_counts.len() == right.expected_type_counts.len()
            });
    let complete = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= 1.01
        && result.diagnostics.ess_bulk >= 400.0
        && result.diagnostics.ess_tail >= 400.0
        && result.diagnostics.minimum_ebfmi >= 0.2
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_replicated_conditional_multitype_mark_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != pymc.input_sha256
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_request_sha256
        || result.maximum_tree_depth != depth
        || result.sampling.chains != pymc.sampling.chains
        || result.sampling.tune_per_chain != pymc.sampling.tune_per_chain
        || result.sampling.draws_per_chain != pymc.sampling.draws_per_chain
        || result.sampling.completed_draws != pymc.sampling.completed_draws
        || result.baseline_affinities.len() != pymc.baseline_affinities.len()
        || result.group_affinity_shifts.len() != pymc.group_affinity_shifts.len()
        || result.patient_affinities.len() != pymc.patient_affinities.len()
        || result.hierarchy_scales.len() != pymc.hierarchy_scales.len()
        || result.pattern_checks.len() != pymc.pattern_checks.len()
        || result
            .comparison
            .independent_pattern_label_composite_log_score
            .to_bits()
            != pymc
                .comparison
                .independent_pattern_label_composite_log_score
                .to_bits()
        || !rows_valid
        || !identities_valid
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesError::WorkerContract(
            "replicated conditional-mark NumPyro result differs".into(),
        ));
    }
    Ok(())
}

fn rows_match(left: &[PairRow], right: &[PairRow]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| left.type_a == right.type_a && left.type_b == right.type_b)
}

pub fn compare(
    left: &FitView,
    right: &NumpyroResult,
    standardized: f64,
    tolerance: f64,
) -> Comparison {
    let baseline_affinities = compare_vector(
        left.baseline_affinities.iter().map(|row| &row.contrast),
        right.baseline_affinities.iter().map(|row| &row.contrast),
        standardized,
        tolerance,
    );
    let group_affinity_shifts = compare_vector(
        left.group_affinity_shifts.iter().map(|row| &row.contrast),
        right.group_affinity_shifts.iter().map(|row| &row.contrast),
        standardized,
        tolerance,
    );
    let patient_affinities = compare_vector(
        left.patient_affinities.iter().map(|row| &row.contrast),
        right.patient_affinities.iter().map(|row| &row.contrast),
        standardized,
        tolerance,
    );
    let hierarchy_scales = compare_vector(
        left.hierarchy_scales.iter().map(|row| &row.scale),
        right.hierarchy_scales.iter().map(|row| &row.scale),
        standardized,
        tolerance,
    );
    let composite_log_score = compare_scalar(
        &left.comparison.spatial_hierarchical_composite_log_score,
        &right.comparison.spatial_hierarchical_composite_log_score,
        standardized,
        tolerance,
    );
    let pattern_same_type_edges = compare_vector(
        left.pattern_checks
            .iter()
            .map(|row| &row.expected_same_type_edges),
        right
            .pattern_checks
            .iter()
            .map(|row| &row.expected_same_type_edges),
        standardized,
        tolerance,
    );
    let pattern_type_counts = compare_vector(
        left.pattern_checks
            .iter()
            .flat_map(|row| row.expected_type_counts.iter()),
        right
            .pattern_checks
            .iter()
            .flat_map(|row| row.expected_type_counts.iter()),
        standardized,
        tolerance,
    );
    let all_pass = baseline_affinities.passes
        && group_affinity_shifts.passes
        && patient_affinities.passes
        && hierarchy_scales.passes
        && composite_log_score.passes
        && pattern_same_type_edges.passes
        && pattern_type_counts.passes;
    Comparison {
        baseline_affinities,
        group_affinity_shifts,
        patient_affinities,
        hierarchy_scales,
        composite_log_score,
        pattern_same_type_edges,
        pattern_type_counts,
        all_pass,
    }
}
