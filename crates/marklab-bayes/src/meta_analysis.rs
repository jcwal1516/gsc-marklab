use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct SiteEstimate {
    pub site_id: String,
    pub effect: f64,
    pub standard_error: f64,
    pub covariate: f64,
}

#[derive(Clone, Debug)]
pub struct MetaAnalysisSpec {
    pub covariate_name: String,
    pub new_site_covariate: f64,
    pub global_prior_mean: f64,
    pub global_prior_sd: f64,
    pub covariate_prior_sd: f64,
    pub heterogeneity_prior_sd: f64,
    pub sites: Vec<SiteEstimate>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MetaAnalysisModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub covariate_name: String,
    pub global_prior_mean: f64,
    pub global_prior_sd: f64,
    pub covariate_prior_sd: f64,
    pub heterogeneity_prior_sd: f64,
    pub observation_unit: &'static str,
    pub biological_unit: &'static str,
    pub hierarchy: [&'static str; 1],
    pub generated_quantities: [&'static str; 2],
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MetaAnalysisWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: MetaAnalysisModelIr,
    pub sites: Vec<SiteEstimate>,
    pub new_site_covariate: f64,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl MetaAnalysisWorkerRequest {
    pub fn new(
        mut spec: MetaAnalysisSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if spec.covariate_name.is_empty() || spec.covariate_name.trim() != spec.covariate_name {
            return Err(BayesError::InvalidSpec(
                "covariate name must be non-empty without surrounding whitespace".into(),
            ));
        }
        if !spec.new_site_covariate.is_finite() || !spec.global_prior_mean.is_finite() {
            return Err(BayesError::InvalidSpec(
                "new-site covariate and global prior mean must be finite".into(),
            ));
        }
        for (value, name) in [
            (spec.global_prior_sd, "global prior SD"),
            (spec.covariate_prior_sd, "covariate prior SD"),
            (spec.heterogeneity_prior_sd, "heterogeneity prior SD"),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(BayesError::InvalidSpec(format!(
                    "{name} must be finite and positive"
                )));
            }
        }
        if spec.sites.len() < 5 || spec.sites.len() > 10_000 {
            return Err(BayesError::InvalidSpec(
                "meta-analysis requires between 5 and 10000 sites".into(),
            ));
        }
        spec.sites
            .sort_by(|left, right| left.site_id.cmp(&right.site_id));
        for (index, site) in spec.sites.iter().enumerate() {
            if site.site_id.is_empty()
                || site.site_id.trim() != site.site_id
                || !site.effect.is_finite()
                || !site.standard_error.is_finite()
                || site.standard_error <= 0.0
                || !site.covariate.is_finite()
                || (index > 0 && spec.sites[index - 1].site_id == site.site_id)
            {
                return Err(BayesError::InvalidSpec(
                    "site IDs/values must be unique, exact, finite, with positive SE".into(),
                ));
            }
        }
        let first_covariate = spec.sites[0].covariate;
        if spec
            .sites
            .iter()
            .all(|site| site.covariate.to_bits() == first_covariate.to_bits())
        {
            return Err(BayesError::InvalidSpec(
                "site covariate must vary for identifiable meta-regression".into(),
            ));
        }
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let total_iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        let maximum_total_iterations = 800_000;
        if total_iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "requested NUTS iterations exceed 800000".into(),
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
            model: MetaAnalysisModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "bayesian_random_effects_meta_regression",
                covariate_name: spec.covariate_name,
                global_prior_mean: spec.global_prior_mean,
                global_prior_sd: spec.global_prior_sd,
                covariate_prior_sd: spec.covariate_prior_sd,
                heterogeneity_prior_sd: spec.heterogeneity_prior_sd,
                observation_unit: "site_effect_estimate",
                biological_unit: "site_or_cohort",
                hierarchy: ["site"],
                generated_quantities: ["site_effect", "new_site_effect"],
                backend_capability: "nuts",
                maturity: "experimental",
            },
            sites: spec.sites,
            new_site_covariate: spec.new_site_covariate,
            sampling,
            resources: WorkerResourceLimits {
                maximum_observations: 10_000,
                maximum_total_iterations,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetaScalarSummary {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetaPosterior {
    pub global_effect: MetaScalarSummary,
    pub covariate_effect: MetaScalarSummary,
    pub heterogeneity: MetaScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SiteEffectSummary {
    pub site_id: String,
    pub observed_effect: f64,
    pub standard_error: f64,
    pub covariate: f64,
    pub posterior_mean: f64,
    pub posterior_sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewSitePrediction {
    pub covariate: f64,
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetaPosteriorPredictive {
    pub observed_effect_mean: f64,
    pub replicated_effect_mean: f64,
    pub replicated_effect_mean_sd: f64,
    pub observed_effect_sd: f64,
    pub replicated_effect_sd_mean: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetaAnalysisWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: MetaPosterior,
    pub site_effects: Vec<SiteEffectSummary>,
    pub new_site_prediction: NewSitePrediction,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: MetaPosteriorPredictive,
}

impl MetaAnalysisWorkerResult {
    pub fn validate(
        &self,
        request: &MetaAnalysisWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly {
            return Err(BayesError::WorkerContract(
                "meta-analysis NUTS cannot return approximate-only state".into(),
            ));
        }
        if self.format != "marklab.pymc_meta_analysis_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "meta-analysis result identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.site_effects.len() != request.sites.len()
        {
            return Err(BayesError::WorkerContract(
                "meta-analysis sample/site counts mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.global_effect,
            &self.posterior.covariate_effect,
            &self.posterior.heterogeneity,
        ] {
            validate_scalar(summary)?;
        }
        if self.posterior.heterogeneity.mean <= 0.0
            || self.posterior.heterogeneity.interval_lower < 0.0
        {
            return Err(BayesError::WorkerContract(
                "invalid heterogeneity summary".into(),
            ));
        }
        for (actual, expected) in self.site_effects.iter().zip(&request.sites) {
            if actual.site_id != expected.site_id
                || actual.observed_effect.to_bits() != expected.effect.to_bits()
                || actual.standard_error.to_bits() != expected.standard_error.to_bits()
                || actual.covariate.to_bits() != expected.covariate.to_bits()
                || !finite(&[
                    actual.posterior_mean,
                    actual.posterior_sd,
                    actual.interval_lower,
                    actual.interval_upper,
                ])
                || actual.posterior_sd <= 0.0
                || actual.interval_lower > actual.interval_upper
            {
                return Err(BayesError::WorkerContract(
                    "invalid site-effect summary".into(),
                ));
            }
        }
        if self.new_site_prediction.covariate.to_bits() != request.new_site_covariate.to_bits()
            || !finite(&[
                self.new_site_prediction.mean,
                self.new_site_prediction.sd,
                self.new_site_prediction.interval_lower,
                self.new_site_prediction.interval_upper,
                self.diagnostics.r_hat,
                self.diagnostics.ess_bulk,
                self.diagnostics.ess_tail,
                self.diagnostics.mcse_mean,
                self.diagnostics.mcse_sd,
                self.diagnostics.minimum_ebfmi,
                self.posterior_predictive.observed_effect_mean,
                self.posterior_predictive.replicated_effect_mean,
                self.posterior_predictive.replicated_effect_mean_sd,
                self.posterior_predictive.observed_effect_sd,
                self.posterior_predictive.replicated_effect_sd_mean,
            ])
            || self.new_site_prediction.sd <= 0.0
            || self.new_site_prediction.interval_lower > self.new_site_prediction.interval_upper
            || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
            || self.posterior_predictive.replicated_effect_mean_sd < 0.0
            || self.posterior_predictive.observed_effect_sd < 0.0
            || self.posterior_predictive.replicated_effect_sd_mean < 0.0
        {
            return Err(BayesError::WorkerContract(
                "invalid prediction or diagnostics".into(),
            ));
        }
        let observed_mean =
            request.sites.iter().map(|site| site.effect).sum::<f64>() / request.sites.len() as f64;
        let observed_sd = (request
            .sites
            .iter()
            .map(|site| (site.effect - observed_mean).powi(2))
            .sum::<f64>()
            / (request.sites.len() - 1) as f64)
            .sqrt();
        if (self.posterior_predictive.observed_effect_mean - observed_mean).abs()
            > 1e-12 * observed_mean.abs().max(1.0)
            || (self.posterior_predictive.observed_effect_sd - observed_sd).abs()
                > 1e-12 * observed_sd.abs().max(1.0)
        {
            return Err(BayesError::WorkerContract(
                "meta-analysis posterior predictive changed observed summaries".into(),
            ));
        }
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
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
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "meta-analysis fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: MetaAnalysisWorkerRequest,
        input: MetaAnalysisInputIdentity,
    ) -> MetaAnalysisFit {
        MetaAnalysisFit {
            format: "marklab.bayesian_meta_analysis_fit",
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
            site_effects: self.site_effects,
            new_site_prediction: self.new_site_prediction,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(summary: &MetaScalarSummary) -> Result<(), BayesError> {
    if !finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
    {
        return Err(BayesError::WorkerContract(
            "invalid meta-analysis scalar summary".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[derive(Debug, Serialize)]
pub struct MetaAnalysisInputIdentity {
    pub path: String,
    pub sites: usize,
    pub site_data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct MetaAnalysisFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: MetaAnalysisModelIr,
    pub input: MetaAnalysisInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: MetaPosterior,
    pub site_effects: Vec<SiteEffectSummary>,
    pub new_site_prediction: NewSitePrediction,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: MetaPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_request_sorts_sites_and_rejects_constant_covariate() {
        let mut sites = vec![
            site("s-5", 5.0, 4.0),
            site("s-1", 1.0, 0.0),
            site("s-4", 4.0, 3.0),
            site("s-2", 2.0, 1.0),
            site("s-3", 3.0, 2.0),
        ];
        let request = MetaAnalysisWorkerRequest::new(
            spec(sites.clone()),
            sampling(),
            "lock".into(),
            "worker".into(),
            180,
        )
        .expect("request");
        assert_eq!(request.sites[0].site_id, "s-1");
        assert_eq!(request.sites[4].site_id, "s-5");

        for site in &mut sites {
            site.covariate = 1.0;
        }
        assert!(matches!(
            MetaAnalysisWorkerRequest::new(
                spec(sites),
                sampling(),
                "lock".into(),
                "worker".into(),
                180,
            ),
            Err(BayesError::InvalidSpec(_))
        ));
    }

    fn site(site_id: &str, effect: f64, covariate: f64) -> SiteEstimate {
        SiteEstimate {
            site_id: site_id.into(),
            effect,
            standard_error: 0.2,
            covariate,
        }
    }

    fn spec(sites: Vec<SiteEstimate>) -> MetaAnalysisSpec {
        MetaAnalysisSpec {
            covariate_name: "score".into(),
            new_site_covariate: 0.0,
            global_prior_mean: 0.0,
            global_prior_sd: 5.0,
            covariate_prior_sd: 2.0,
            heterogeneity_prior_sd: 1.0,
            sites,
        }
    }

    fn sampling() -> NutsSamplingSpec {
        NutsSamplingSpec {
            chains: 2,
            tune_per_chain: 500,
            draws_per_chain: 1_000,
            target_accept: 0.9,
            seed: 7,
        }
    }
}
