use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    BackendContract, BayesError, DirichletMultinomialGroupModelIr,
    DirichletMultinomialGroupWorkerRequest, FitState, HierarchicalSbcFailure,
    HierarchicalSbcParameterDiagnostics, NutsSamplingSpec, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct DirichletMultinomialGroupSbcCalibrationPolicy {
    pub replicates: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub maximum_tree_depth: u32,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DirichletMultinomialGroupSbcResourceLimits {
    pub maximum_replicates: u32,
    pub maximum_simulated_cells: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroDirichletMultinomialGroupSbcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: DirichletMultinomialGroupWorkerRequest,
    pub calibration: DirichletMultinomialGroupSbcCalibrationPolicy,
    pub resources: DirichletMultinomialGroupSbcResourceLimits,
}

impl NumpyroDirichletMultinomialGroupSbcWorkerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_request: DirichletMultinomialGroupWorkerRequest,
        replicates: u32,
        minimum_rank_uniformity_p_value: f64,
        minimum_coverage: f64,
        maximum_coverage: f64,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !(20..=100).contains(&replicates)
            || !minimum_rank_uniformity_p_value.is_finite()
            || !(0.0..0.1).contains(&minimum_rank_uniformity_p_value)
            || !minimum_coverage.is_finite()
            || !maximum_coverage.is_finite()
            || !(0.0..=1.0).contains(&minimum_coverage)
            || !(0.0..=1.0).contains(&maximum_coverage)
            || minimum_coverage > maximum_coverage
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial SBC controls or identities are invalid".into(),
            ));
        }
        let cells = source_request
            .patients
            .iter()
            .flat_map(|patient| &patient.counts)
            .try_fold(0_u64, |total, count| total.checked_add(*count))
            .ok_or_else(|| BayesError::InvalidSpec("SBC cell count overflow".into()))?;
        let simulated_cells = u64::from(replicates)
            .checked_mul(cells)
            .ok_or_else(|| BayesError::InvalidSpec("SBC work overflow".into()))?;
        let total_iterations = u64::from(replicates)
            .checked_mul(u64::from(source_request.sampling.chains))
            .and_then(|value| {
                value.checked_mul(
                    u64::from(source_request.sampling.tune_per_chain)
                        + u64::from(source_request.sampling.draws_per_chain),
                )
            })
            .ok_or_else(|| BayesError::InvalidSpec("SBC iterations overflow".into()))?;
        let maximum_simulated_cells = 200_000_000;
        let maximum_total_iterations = 1_000_000;
        if simulated_cells > maximum_simulated_cells || total_iterations > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial SBC exceeds cell or iteration limits".into(),
            ));
        }
        let source_request_sha256 = crate::sha256_hex(&serde_json::to_vec(&source_request)?);
        Ok(Self {
            format: REQUEST_FORMAT,
            version: 1,
            backend: BackendContract {
                name: "numpyro",
                version: NUMPYRO_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            jax_version: JAX_VERSION,
            source_request_sha256,
            source_request,
            calibration: DirichletMultinomialGroupSbcCalibrationPolicy {
                replicates,
                rank_bins: 10,
                interval_probability: 0.9,
                maximum_tree_depth: 12,
                minimum_rank_uniformity_p_value,
                minimum_coverage,
                maximum_coverage,
            },
            resources: DirichletMultinomialGroupSbcResourceLimits {
                maximum_replicates: 100,
                maximum_simulated_cells,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
        })
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialGroupSbcReplicate {
    pub replicate: u32,
    pub true_baseline_logits: Vec<f64>,
    pub true_group_log_ratio_effects: Vec<f64>,
    pub true_concentration: f64,
    pub true_class_probability_differences: Vec<f64>,
    pub baseline_logit_ranks: Vec<u32>,
    pub group_log_ratio_effect_ranks: Vec<u32>,
    pub concentration_rank: u32,
    pub class_probability_difference_ranks: Vec<u32>,
    pub baseline_logit_covered: Vec<bool>,
    pub group_log_ratio_effect_covered: Vec<bool>,
    pub concentration_covered: bool,
    pub class_probability_difference_covered: Vec<bool>,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialGroupSbcDiagnostics {
    pub baseline_logits: Vec<HierarchicalSbcParameterDiagnostics>,
    pub group_log_ratio_effects: Vec<HierarchicalSbcParameterDiagnostics>,
    pub concentration: HierarchicalSbcParameterDiagnostics,
    pub class_probability_differences: Vec<HierarchicalSbcParameterDiagnostics>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroDirichletMultinomialGroupSbcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub replicates: Vec<DirichletMultinomialGroupSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: DirichletMultinomialGroupSbcDiagnostics,
}

impl NumpyroDirichletMultinomialGroupSbcWorkerResult {
    pub fn validate(
        &self,
        request: &NumpyroDirichletMultinomialGroupSbcWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let classes = request.source_request.model.class_ids.len();
        let free = classes - 1;
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.jax_version != JAX_VERSION
            || self.request_sha256 != request_sha256
            || self.replicates.len() + self.failures.len()
                != request.calibration.replicates as usize
            || self.diagnostics.baseline_logits.len() != free
            || self.diagnostics.group_log_ratio_effects.len() != free
            || self.diagnostics.class_probability_differences.len() != classes
        {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial SBC result identity or dimensions mismatch".into(),
            ));
        }
        let posterior_draws = request.source_request.sampling.chains
            * request.source_request.sampling.draws_per_chain;
        let policy = &request.source_request.diagnostic_policy;
        let mut dispositions = BTreeSet::new();
        for row in &self.replicates {
            let vector_dimensions = row.true_baseline_logits.len() == free
                && row.true_group_log_ratio_effects.len() == free
                && row.true_class_probability_differences.len() == classes
                && row.baseline_logit_ranks.len() == free
                && row.group_log_ratio_effect_ranks.len() == free
                && row.class_probability_difference_ranks.len() == classes
                && row.baseline_logit_covered.len() == free
                && row.group_log_ratio_effect_covered.len() == free
                && row.class_probability_difference_covered.len() == classes;
            if row.replicate >= request.calibration.replicates
                || !dispositions.insert(row.replicate)
                || !vector_dimensions
                || row
                    .baseline_logit_ranks
                    .iter()
                    .chain(&row.group_log_ratio_effect_ranks)
                    .chain(&row.class_probability_difference_ranks)
                    .chain(std::iter::once(&row.concentration_rank))
                    .any(|rank| *rank > posterior_draws)
                || row
                    .true_baseline_logits
                    .iter()
                    .chain(&row.true_group_log_ratio_effects)
                    .chain(&row.true_class_probability_differences)
                    .chain([
                        &row.true_concentration,
                        &row.r_hat,
                        &row.ess_bulk,
                        &row.ess_tail,
                        &row.minimum_ebfmi,
                    ])
                    .any(|value| !value.is_finite())
                || row.true_concentration <= 0.0
                || row
                    .true_class_probability_differences
                    .iter()
                    .any(|value| !(-1.0..1.0).contains(value))
                || row.r_hat > policy.maximum_r_hat
                || row.ess_bulk < policy.minimum_bulk_ess
                || row.ess_tail < policy.minimum_tail_ess
                || row.minimum_ebfmi < policy.minimum_ebfmi
                || row.divergences > policy.maximum_divergences
                || row.max_tree_depth_hits > policy.maximum_tree_depth_hits
            {
                return Err(BayesError::WorkerContract(
                    "Dirichlet-multinomial SBC replicate is invalid".into(),
                ));
            }
        }
        for failure in &self.failures {
            if failure.replicate >= request.calibration.replicates
                || !dispositions.insert(failure.replicate)
                || failure.reason.is_empty()
                || failure.reason.len() > 512
            {
                return Err(BayesError::WorkerContract(
                    "Dirichlet-multinomial SBC failure disposition is invalid".into(),
                ));
            }
        }
        let diagnostics = self
            .diagnostics
            .baseline_logits
            .iter()
            .chain(&self.diagnostics.group_log_ratio_effects)
            .chain(std::iter::once(&self.diagnostics.concentration))
            .chain(&self.diagnostics.class_probability_differences)
            .collect::<Vec<_>>();
        for diagnostic in &diagnostics {
            if diagnostic.rank_histogram.len() != request.calibration.rank_bins as usize
                || diagnostic.rank_histogram.iter().sum::<u32>() != self.replicates.len() as u32
                || ![
                    diagnostic.rank_uniformity_p_value,
                    diagnostic.coverage_90,
                    diagnostic.mean_normalized_rank,
                ]
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            {
                return Err(BayesError::WorkerContract(
                    "Dirichlet-multinomial SBC diagnostics are invalid".into(),
                ));
            }
        }
        let passes = self.failures.is_empty()
            && diagnostics.iter().all(|diagnostic| {
                diagnostic.rank_uniformity_p_value
                    >= request.calibration.minimum_rank_uniformity_p_value
                    && (request.calibration.minimum_coverage..=request.calibration.maximum_coverage)
                        .contains(&diagnostic.coverage_90)
            });
        if dispositions.len() != request.calibration.replicates as usize
            || (self.fit_state == FitState::Complete) != passes
        {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial SBC disposition or fit state is invalid".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: NumpyroDirichletMultinomialGroupSbcWorkerRequest,
    ) -> DirichletMultinomialGroupSbcResult {
        DirichletMultinomialGroupSbcResult {
            format: "marklab.bayesian_dirichlet_multinomial_group_sbc",
            version: 1,
            backend: self.backend,
            jax_version: self.jax_version,
            model: request.source_request.model,
            sampling: request.source_request.sampling,
            calibration: request.calibration,
            resources: request.resources,
            fit_state: self.fit_state,
            replicates: self.replicates,
            failures: self.failures,
            diagnostics: self.diagnostics,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_simulation_calibration"
            } else {
                "diagnostic_only_failed_calibration"
            },
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub model: DirichletMultinomialGroupModelIr,
    pub sampling: NutsSamplingSpec,
    pub calibration: DirichletMultinomialGroupSbcCalibrationPolicy,
    pub resources: DirichletMultinomialGroupSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<DirichletMultinomialGroupSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: DirichletMultinomialGroupSbcDiagnostics,
    pub claim_status: &'static str,
    pub request_sha256: String,
}
