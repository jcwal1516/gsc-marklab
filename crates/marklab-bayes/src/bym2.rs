use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    sar_fit::design_full_rank,
    BayesError, BymPosteriorPredictive, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, SarCoefficientSummary, SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct Bym2FitSpec {
    pub region_ids: Vec<String>,
    pub weights_digest_sha256: String,
    pub scaled_icar_transform: Vec<f64>,
    pub rank_deficiency: usize,
    pub components: Vec<Vec<usize>>,
    pub original_typical_marginal_variance: f64,
    pub scaled_typical_marginal_variance: f64,
    pub counts: Vec<u64>,
    pub expected: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub total_sd_prior: f64,
    pub phi_alpha: f64,
    pub phi_beta: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Bym2ModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub likelihood: &'static str,
    pub offset: &'static str,
    pub reparameterization: &'static str,
    pub icar_scaling: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub total_sd_prior: f64,
    pub phi_alpha: f64,
    pub phi_beta: f64,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct Bym2FitWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: Bym2ModelIr,
    pub region_ids: Vec<String>,
    pub weights_digest_sha256: String,
    pub scaled_icar_transform: Vec<f64>,
    pub rank_deficiency: usize,
    pub components: Vec<Vec<usize>>,
    pub original_typical_marginal_variance: f64,
    pub scaled_typical_marginal_variance: f64,
    pub counts: Vec<u64>,
    pub expected: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl Bym2FitWorkerRequest {
    pub fn new(
        spec: Bym2FitSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        validate_spec(&spec)?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "BYM2 worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        let iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "requested BYM2 NUTS iterations exceed 400000".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: Bym2ModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "poisson_bym2_disease_mapping",
                likelihood: "poisson_log_link",
                offset: "log_expected",
                reparameterization: "sigma_times_sqrt_phi_icar_plus_sqrt_one_minus_phi_iid",
                icar_scaling: "geometric_mean_generalized_marginal_variance_one",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                coefficient_prior_sd: spec.coefficient_prior_sd,
                total_sd_prior: spec.total_sd_prior,
                phi_alpha: spec.phi_alpha,
                phi_beta: spec.phi_beta,
                backend_capability: "nuts",
                maturity: "experimental",
            },
            region_ids: spec.region_ids,
            weights_digest_sha256: spec.weights_digest_sha256,
            scaled_icar_transform: spec.scaled_icar_transform,
            rank_deficiency: spec.rank_deficiency,
            components: spec.components,
            original_typical_marginal_variance: spec.original_typical_marginal_variance,
            scaled_typical_marginal_variance: spec.scaled_typical_marginal_variance,
            counts: spec.counts,
            expected: spec.expected,
            design: spec.design,
            predictor_names: spec.predictor_names,
            sampling,
            resources: WorkerResourceLimits {
                maximum_observations: 64,
                maximum_total_iterations,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

fn validate_spec(spec: &Bym2FitSpec) -> Result<(), BayesError> {
    let dimension = spec.region_ids.len();
    let predictors = spec.predictor_names.len();
    let constrained_dimension = dimension.saturating_sub(spec.rank_deficiency);
    if !(6..=64).contains(&dimension)
        || !(1..=16).contains(&predictors)
        || spec.counts.len() != dimension
        || spec.expected.len() != dimension
        || spec.design.len() != dimension * predictors
        || spec.scaled_icar_transform.len() != dimension * constrained_dimension
        || spec.rank_deficiency == 0
        || spec.rank_deficiency >= dimension
        || spec.components.len() != spec.rank_deficiency
    {
        return Err(BayesError::InvalidSpec(
            "BYM2 input dimensions violate the region, predictor, or ICAR contract".into(),
        ));
    }
    for (index, region_id) in spec.region_ids.iter().enumerate() {
        if region_id.is_empty()
            || region_id.trim() != region_id
            || (index > 0 && spec.region_ids[index - 1] >= *region_id)
        {
            return Err(BayesError::InvalidSpec(
                "BYM2 region IDs must be exact and strictly increasing".into(),
            ));
        }
    }
    for (index, predictor) in spec.predictor_names.iter().enumerate() {
        if predictor.is_empty()
            || predictor.trim() != predictor
            || predictor == "intercept"
            || spec.predictor_names[..index].contains(predictor)
        {
            return Err(BayesError::InvalidSpec(
                "BYM2 predictor names must be exact, unique, and not intercept".into(),
            ));
        }
    }
    if spec
        .expected
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || spec.design.iter().any(|value| !value.is_finite())
        || spec
            .scaled_icar_transform
            .iter()
            .any(|value| !value.is_finite())
        || !design_full_rank(&spec.design, dimension, predictors)
        || !spec.original_typical_marginal_variance.is_finite()
        || spec.original_typical_marginal_variance <= 0.0
        || !spec.scaled_typical_marginal_variance.is_finite()
        || (spec.scaled_typical_marginal_variance - 1.0).abs() > 1e-12
    {
        return Err(BayesError::InvalidSpec(
            "BYM2 expected/design/transform/rank or ICAR scale is invalid".into(),
        ));
    }
    let mut covered = vec![false; dimension];
    for component in &spec.components {
        if component.len() < 2 {
            return Err(BayesError::InvalidSpec(
                "BYM2 ICAR components must contain at least two regions".into(),
            ));
        }
        for &index in component {
            if index >= dimension || std::mem::replace(&mut covered[index], true) {
                return Err(BayesError::InvalidSpec(
                    "BYM2 components must partition all regions exactly once".into(),
                ));
            }
        }
        for column in 0..constrained_dimension {
            let sum = component
                .iter()
                .map(|&row| spec.scaled_icar_transform[row * constrained_dimension + column])
                .sum::<f64>();
            if sum.abs() > 1e-10 {
                return Err(BayesError::InvalidSpec(
                    "BYM2 scaled ICAR transform violates sum-to-zero".into(),
                ));
            }
        }
    }
    if covered.iter().any(|covered| !covered)
        || spec.weights_digest_sha256.len() != 64
        || !spec
            .weights_digest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(BayesError::InvalidSpec(
            "BYM2 components or weights identity are invalid".into(),
        ));
    }
    for (value, name) in [
        (spec.intercept_prior_sd, "intercept prior SD"),
        (spec.coefficient_prior_sd, "coefficient prior SD"),
        (spec.total_sd_prior, "total SD prior"),
        (spec.phi_alpha, "phi alpha"),
        (spec.phi_beta, "phi beta"),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesError::InvalidSpec(format!(
                "BYM2 {name} must be finite and positive"
            )));
        }
    }
    if !spec.intercept_prior_mean.is_finite() {
        return Err(BayesError::InvalidSpec(
            "BYM2 intercept prior mean must be finite".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bym2Posterior {
    pub intercept: SarScalarSummary,
    pub coefficients: Vec<SarCoefficientSummary>,
    pub sigma: SarScalarSummary,
    pub phi: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bym2RegionSummary {
    pub region_id: String,
    pub observed_count: u64,
    pub expected: f64,
    pub structured_component: SarScalarSummary,
    pub unstructured_component: SarScalarSummary,
    pub combined_effect: SarScalarSummary,
    pub relative_risk: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bym2FitWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: Bym2Posterior,
    pub regions: Vec<Bym2RegionSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BymPosteriorPredictive,
}

impl Bym2FitWorkerResult {
    pub fn validate(
        &self,
        request: &Bym2FitWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_bym2_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "BYM2 result identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.posterior.coefficients.len() != request.predictor_names.len()
            || self.regions.len() != request.region_ids.len()
        {
            return Err(BayesError::WorkerContract(
                "BYM2 sampling, coefficient, or region counts mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.intercept,
            &self.posterior.sigma,
            &self.posterior.phi,
        ] {
            validate_scalar(summary, false)?;
        }
        if self.posterior.sigma.mean <= 0.0
            || self.posterior.sigma.interval_lower < 0.0
            || self.posterior.phi.interval_lower < 0.0
            || self.posterior.phi.interval_upper > 1.0
        {
            return Err(BayesError::WorkerContract(
                "BYM2 sigma or phi posterior violates support".into(),
            ));
        }
        for (actual, expected) in self
            .posterior
            .coefficients
            .iter()
            .zip(&request.predictor_names)
        {
            if actual.predictor != *expected {
                return Err(BayesError::WorkerContract(
                    "BYM2 coefficient identity mismatch".into(),
                ));
            }
            validate_scalar(&actual.summary, false)?;
        }
        for (index, region) in self.regions.iter().enumerate() {
            if region.region_id != request.region_ids[index]
                || region.observed_count != request.counts[index]
                || region.expected.to_bits() != request.expected[index].to_bits()
            {
                return Err(BayesError::WorkerContract(
                    "BYM2 region result changed exact input identity".into(),
                ));
            }
            validate_scalar(&region.structured_component, false)?;
            validate_scalar(&region.unstructured_component, false)?;
            validate_scalar(&region.combined_effect, false)?;
            validate_scalar(&region.relative_risk, true)?;
        }
        let observed_total = request.counts.iter().sum::<u64>();
        let observed_zeros = request.counts.iter().filter(|&&count| count == 0).count() as u64;
        if self.posterior_predictive.observed_total_count != observed_total
            || self.posterior_predictive.observed_zero_regions != observed_zeros
            || !finite(&[
                self.posterior_predictive.replicated_total_mean,
                self.posterior_predictive.replicated_total_sd,
                self.posterior_predictive.replicated_zero_regions_mean,
                self.diagnostics.r_hat,
                self.diagnostics.ess_bulk,
                self.diagnostics.ess_tail,
                self.diagnostics.mcse_mean,
                self.diagnostics.mcse_sd,
                self.diagnostics.minimum_ebfmi,
            ])
            || self.posterior_predictive.replicated_total_sd < 0.0
            || self.posterior_predictive.replicated_zero_regions_mean < 0.0
            || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
        {
            return Err(BayesError::WorkerContract(
                "BYM2 predictive or diagnostics are invalid".into(),
            ));
        }
        let pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= request.diagnostic_policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= request.diagnostic_policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
            && self.diagnostics.divergences <= request.diagnostic_policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits
                <= request.diagnostic_policy.maximum_tree_depth_hits;
        if (self.fit_state == FitState::Complete) != pass {
            return Err(BayesError::WorkerContract(
                "BYM2 fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: Bym2FitWorkerRequest,
        input: Bym2InputIdentity,
    ) -> Bym2FitResult {
        Bym2FitResult {
            format: "marklab.bayesian_bym2_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            fit_state: self.fit_state,
            claim_status: match self.fit_state {
                FitState::Complete => "experimental",
                FitState::Nonconverged => "diagnostic_only_nonconverged",
                FitState::ApproximateOnly => "experimental_approximate_only",
            },
            sampling: self.sampling,
            posterior: self.posterior,
            icar_scaling: Bym2IcarScaling {
                convention: "geometric_mean_generalized_marginal_variance",
                original_typical_marginal_variance: request.original_typical_marginal_variance,
                scaled_typical_marginal_variance: request.scaled_typical_marginal_variance,
            },
            rank_deficiency: request.rank_deficiency,
            constraints: request
                .components
                .iter()
                .map(|component| {
                    format!(
                        "sum_to_zero:{}",
                        component
                            .iter()
                            .map(|&index| request.region_ids[index].as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect(),
            regions: self.regions,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(summary: &SarScalarSummary, positive: bool) -> Result<(), BayesError> {
    if !finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || (positive && (summary.mean <= 0.0 || summary.interval_lower <= 0.0))
    {
        return Err(BayesError::WorkerContract(
            "invalid BYM2 scalar summary".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[derive(Debug, Serialize)]
pub struct Bym2InputIdentity {
    pub regions_path: String,
    pub edges_path: String,
    pub data_path: String,
    pub weights_digest_sha256: String,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Bym2IcarScaling {
    pub convention: &'static str,
    pub original_typical_marginal_variance: f64,
    pub scaled_typical_marginal_variance: f64,
}

#[derive(Debug, Serialize)]
pub struct Bym2FitResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: Bym2ModelIr,
    pub input: Bym2InputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: Bym2Posterior,
    pub icar_scaling: Bym2IcarScaling,
    pub rank_deficiency: usize,
    pub constraints: Vec<String>,
    pub regions: Vec<Bym2RegionSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BymPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}
