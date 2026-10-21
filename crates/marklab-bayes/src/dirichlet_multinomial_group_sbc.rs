use crate::is_worker_timeout;
use crate::validation::is_sha256;

use serde::{Deserialize, Serialize};

use crate::backend_versions::PYTHON_VERSION;
use crate::sbc::{
    validate_sbc_replicates, SbcCalibrationPolicy, SbcControls, SbcReplicateRow, SbcSamplerRow,
};
use crate::ClaimStatus;
use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    BackendContract, BayesError, DirichletMultinomialGroupModelIr,
    DirichletMultinomialGroupWorkerRequest, FitState, HierarchicalSbcFailure, NutsSamplingSpec,
    SbcParameterDiagnostics, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_result";

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
    pub calibration: SbcCalibrationPolicy,
    pub resources: DirichletMultinomialGroupSbcResourceLimits,
}

impl NumpyroDirichletMultinomialGroupSbcWorkerRequest {
    pub fn new(
        source_request: DirichletMultinomialGroupWorkerRequest,
        controls: SbcControls,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        let calibration = SbcCalibrationPolicy::new(controls)
            .filter(|_| {
                is_sha256(&environment_lock_sha256)
                    && is_sha256(&worker_sha256)
                    && is_worker_timeout(timeout_seconds)
            })
            .ok_or_else(|| {
                BayesError::InvalidSpec(
                    "Dirichlet-multinomial SBC controls or identities are invalid".into(),
                )
            })?;
        let replicates = calibration.replicates;
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
        let source_request_sha256 = crate::json_sha256(&source_request)?;
        Ok(Self {
            format: REQUEST_FORMAT,
            version: 1,
            backend: BackendContract {
                name: "numpyro",
                version: NUMPYRO_VERSION,
                python_version: PYTHON_VERSION,
                environment_lock_sha256,
                worker_sha256,
            },
            jax_version: JAX_VERSION,
            source_request_sha256,
            source_request,
            calibration,
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

impl SbcReplicateRow for DirichletMultinomialGroupSbcReplicate {
    fn replicate(&self) -> u32 {
        self.replicate
    }

    /// Ranks in diagnostics order: baseline logits, group effects, concentration, then
    /// class-probability differences.
    fn ranks(&self) -> Vec<(u32, bool)> {
        let pairs = |ranks: &[u32], covered: &[bool]| -> Vec<(u32, bool)> {
            ranks.iter().copied().zip(covered.iter().copied()).collect()
        };
        let mut ranks = pairs(&self.baseline_logit_ranks, &self.baseline_logit_covered);
        ranks.extend(pairs(
            &self.group_log_ratio_effect_ranks,
            &self.group_log_ratio_effect_covered,
        ));
        ranks.push((self.concentration_rank, self.concentration_covered));
        ranks.extend(pairs(
            &self.class_probability_difference_ranks,
            &self.class_probability_difference_covered,
        ));
        ranks
    }

    fn truths_valid(&self) -> bool {
        self.true_baseline_logits
            .iter()
            .chain(&self.true_group_log_ratio_effects)
            .chain(&self.true_class_probability_differences)
            .chain(std::iter::once(&self.true_concentration))
            .all(|value| value.is_finite())
            && self.true_concentration > 0.0
            && self
                .true_class_probability_differences
                .iter()
                .all(|value| (-1.0..1.0).contains(value))
    }

    fn sampler(&self) -> SbcSamplerRow {
        SbcSamplerRow {
            r_hat: self.r_hat,
            ess_bulk: self.ess_bulk,
            ess_tail: self.ess_tail,
            minimum_ebfmi: Some(self.minimum_ebfmi),
            divergences: self.divergences,
            max_tree_depth_hits: self.max_tree_depth_hits,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialGroupSbcDiagnostics {
    pub baseline_logits: Vec<SbcParameterDiagnostics>,
    pub group_log_ratio_effects: Vec<SbcParameterDiagnostics>,
    pub concentration: SbcParameterDiagnostics,
    pub class_probability_differences: Vec<SbcParameterDiagnostics>,
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
            || !self.backend.matches(&request.backend)
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
        let dimensions_match = self.replicates.iter().all(|row| {
            row.true_baseline_logits.len() == free
                && row.true_group_log_ratio_effects.len() == free
                && row.true_class_probability_differences.len() == classes
                && row.baseline_logit_ranks.len() == free
                && row.group_log_ratio_effect_ranks.len() == free
                && row.class_probability_difference_ranks.len() == classes
                && row.baseline_logit_covered.len() == free
                && row.group_log_ratio_effect_covered.len() == free
                && row.class_probability_difference_covered.len() == classes
        });
        if !dimensions_match {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial SBC replicate is invalid".into(),
            ));
        }
        let diagnostics: Vec<&SbcParameterDiagnostics> = self
            .diagnostics
            .baseline_logits
            .iter()
            .chain(&self.diagnostics.group_log_ratio_effects)
            .chain(std::iter::once(&self.diagnostics.concentration))
            .chain(&self.diagnostics.class_probability_differences)
            .collect();
        validate_sbc_replicates(
            &self.replicates,
            &self.failures,
            &diagnostics,
            &request.calibration,
            request.source_request.sampling.chains
                * request.source_request.sampling.draws_per_chain,
            &request.source_request.diagnostic_policy,
            self.fit_state,
        )
        .map_err(|violation| violation.error("Dirichlet-multinomial"))
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
                ClaimStatus::ExperimentalSimulationCalibration
            } else {
                ClaimStatus::DiagnosticOnlyFailedCalibration
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
    pub calibration: SbcCalibrationPolicy,
    pub resources: DirichletMultinomialGroupSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<DirichletMultinomialGroupSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: DirichletMultinomialGroupSbcDiagnostics,
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}
