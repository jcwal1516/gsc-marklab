use crate::is_worker_timeout;
use crate::validation::{
    diagnostics_satisfy_policy, is_exact_name, is_sha256, validate_mcse_summary, SummarySupport,
};

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::backend_versions::PYTHON_VERSION;
use crate::ClaimStatus;
use crate::{
    model::{PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BackendContract, BayesError, DiagnosticPolicy, FitState, McseScalarSummary, NutsSamplingSpec,
    PosteriorDiagnostics, SamplingSummary, WorkerBackend,
};
use marklab_core::backend::BackendCapability;
use marklab_core::backend::ModelMaturity;

#[derive(Clone, Debug, Serialize)]
pub struct DirichletMultinomialPatientData {
    pub patient_id: String,
    pub group: String,
    pub counts: Vec<u64>,
}

/// Dirichlet-multinomial patient class counts with a two-group log-ratio model.
///
/// Each count vector has its observed total and concentration vector
/// `phi * pi_group`. Probabilities are `pi_reference = softmax([b, 0])` and
/// `pi_comparison = softmax([b + d, 0])`, fixing the last entry of `class_ids`
/// as the zero-logit reference. Independent priors are
/// `b_k ~ Normal(0, logit_prior_sd)`,
/// `d_k ~ Normal(0, group_effect_prior_sd)`, and
/// `phi ~ HalfNormal(concentration_prior_sd)`. Normal scales are SDs and the
/// half-normal scale is its underlying normal SD. Patient composition vectors
/// are integrated out during fitting.
#[derive(Clone, Debug)]
pub struct DirichletMultinomialGroupSpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub class_ids: Vec<String>,
    pub logit_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
    pub patients: Vec<DirichletMultinomialPatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DirichletMultinomialGroupModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub reference_group: String,
    pub comparison_group: String,
    pub class_ids: Vec<String>,
    pub reference_class: String,
    pub baseline_logit_prior: &'static str,
    pub logit_prior_sd: f64,
    pub group_effect_prior: &'static str,
    pub group_effect_prior_sd: f64,
    pub concentration_prior: &'static str,
    pub concentration_prior_sd: f64,
    pub likelihood: &'static str,
    pub observation_unit: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: BackendCapability,
    pub maturity: ModelMaturity,
}

#[derive(Clone, Debug, Serialize)]
pub struct DirichletMultinomialGroupResourceLimits {
    pub maximum_patients: u32,
    pub maximum_classes: u32,
    pub maximum_total_cells: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DirichletMultinomialGroupWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: DirichletMultinomialGroupModelIr,
    pub patients: Vec<DirichletMultinomialPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: DirichletMultinomialGroupResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl DirichletMultinomialGroupWorkerRequest {
    pub fn new(
        spec: DirichletMultinomialGroupSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !is_exact_name(&spec.reference_group)
            || !is_exact_name(&spec.comparison_group)
            || spec.reference_group == spec.comparison_group
            || !(3..=16).contains(&spec.class_ids.len())
            || !spec.logit_prior_sd.is_finite()
            || spec.logit_prior_sd <= 0.0
            || !spec.group_effect_prior_sd.is_finite()
            || spec.group_effect_prior_sd <= 0.0
            || !spec.concentration_prior_sd.is_finite()
            || spec.concentration_prior_sd <= 0.0
            || !(8..=512).contains(&spec.patients.len())
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !is_worker_timeout(timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial group controls or identities are invalid".into(),
            ));
        }
        if spec.class_ids.iter().any(|class| !is_exact_name(class))
            || spec.class_ids.iter().collect::<BTreeSet<_>>().len() != spec.class_ids.len()
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial class identities are invalid".into(),
            ));
        }
        let mut patients = spec.patients;
        patients.sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut patient_ids = BTreeSet::new();
        let mut group_counts = BTreeMap::new();
        let mut total_cells = 0_u64;
        for patient in &patients {
            let patient_total = patient.counts.iter().try_fold(0_u64, |total, count| {
                total.checked_add(*count).ok_or_else(|| {
                    BayesError::InvalidSpec("Dirichlet-multinomial count overflow".into())
                })
            })?;
            if !is_exact_name(&patient.patient_id)
                || !patient_ids.insert(patient.patient_id.as_str())
                || (patient.group != spec.reference_group && patient.group != spec.comparison_group)
                || patient.counts.len() != spec.class_ids.len()
                || patient_total == 0
            {
                return Err(BayesError::InvalidSpec(
                    "Dirichlet-multinomial patient rows are invalid".into(),
                ));
            }
            *group_counts.entry(patient.group.as_str()).or_insert(0) += 1;
            total_cells = total_cells.checked_add(patient_total).ok_or_else(|| {
                BayesError::InvalidSpec("Dirichlet-multinomial total count overflow".into())
            })?;
        }
        if group_counts
            .get(spec.reference_group.as_str())
            .copied()
            .unwrap_or(0)
            < 4
            || group_counts
                .get(spec.comparison_group.as_str())
                .copied()
                .unwrap_or(0)
                < 4
            || total_cells > 10_000_000
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial requires four patients per group and at most 10000000 cells"
                    .into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        if u64::from(sampling.chains)
            * (u64::from(sampling.tune_per_chain) + u64::from(sampling.draws_per_chain))
            > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial exceeds 400000 NUTS iterations".into(),
            ));
        }
        let reference_class = spec
            .class_ids
            .last()
            .expect("at least three classes")
            .clone();
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                environment_lock_sha256,
                worker_sha256,
            },
            model: DirichletMultinomialGroupModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "patient_dirichlet_multinomial_group_regression",
                reference_group: spec.reference_group,
                comparison_group: spec.comparison_group,
                class_ids: spec.class_ids,
                reference_class,
                baseline_logit_prior: "normal_with_reference_class_zero",
                logit_prior_sd: spec.logit_prior_sd,
                group_effect_prior: "normal_log_ratio_difference_with_reference_class_zero",
                group_effect_prior_sd: spec.group_effect_prior_sd,
                concentration_prior: "half_normal",
                concentration_prior_sd: spec.concentration_prior_sd,
                likelihood: "dirichlet_multinomial_collapsed_patient_count_vectors",
                observation_unit: "patient_complete_class_count_vector",
                biological_unit: "patient",
                backend_capability: BackendCapability::Nuts,
                maturity: ModelMaturity::Experimental,
            },
            patients,
            sampling,
            resources: DirichletMultinomialGroupResourceLimits {
                maximum_patients: 512,
                maximum_classes: 16,
                maximum_total_cells: 10_000_000,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialClassPosterior {
    pub class_id: String,
    pub reference_probability: McseScalarSummary,
    pub comparison_probability: McseScalarSummary,
    pub difference_comparison_minus_reference: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialGroupPosterior {
    pub classes: Vec<DirichletMultinomialClassPosterior>,
    pub concentration: McseScalarSummary,
    pub overdispersion_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialClassPredictive {
    pub class_id: String,
    pub observed_reference_mean_proportion: f64,
    pub observed_comparison_mean_proportion: f64,
    pub replicated_reference_mean_proportion: f64,
    pub replicated_comparison_mean_proportion: f64,
    pub probability_replicated_difference_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialGroupPosteriorPredictive {
    pub classes: Vec<DirichletMultinomialClassPredictive>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirichletMultinomialGroupWorkerResult {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: DirichletMultinomialGroupPosterior,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: DirichletMultinomialGroupPosteriorPredictive,
}

impl DirichletMultinomialGroupWorkerResult {
    pub fn validate(
        &self,
        request: &DirichletMultinomialGroupWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        self.validate_for_backend(
            request,
            request_sha256,
            "marklab.pymc_dirichlet_multinomial_group_worker_result",
            &request.backend,
        )
    }

    pub(crate) fn validate_for_backend(
        &self,
        request: &DirichletMultinomialGroupWorkerRequest,
        request_sha256: &str,
        result_format: &str,
        backend: &BackendContract,
    ) -> Result<(), BayesError> {
        if self.format != result_format
            || self.version != 1
            || !self.backend.matches(backend)
            || self.request_sha256 != request_sha256
            || self.posterior.classes.len() != request.model.class_ids.len()
            || self.posterior_predictive.classes.len() != request.model.class_ids.len()
        {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial result identity or dimensions mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.completed_draws != expected_draws
            || self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
        {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial sampling counts mismatch".into(),
            ));
        }
        let mut reference_sum = 0.0;
        let mut comparison_sum = 0.0;
        let mut difference_sum = 0.0;
        for ((posterior, predictive), class_id) in self
            .posterior
            .classes
            .iter()
            .zip(&self.posterior_predictive.classes)
            .zip(&request.model.class_ids)
        {
            if posterior.class_id != *class_id || predictive.class_id != *class_id {
                return Err(BayesError::WorkerContract(
                    "Dirichlet-multinomial class order mismatch".into(),
                ));
            }
            validate_summary(&posterior.reference_probability, SummarySupport::Unit)?;
            validate_summary(&posterior.comparison_probability, SummarySupport::Unit)?;
            validate_summary(
                &posterior.difference_comparison_minus_reference,
                SummarySupport::Difference,
            )?;
            reference_sum += posterior.reference_probability.mean;
            comparison_sum += posterior.comparison_probability.mean;
            difference_sum += posterior.difference_comparison_minus_reference.mean;
            if ![
                predictive.observed_reference_mean_proportion,
                predictive.observed_comparison_mean_proportion,
                predictive.replicated_reference_mean_proportion,
                predictive.replicated_comparison_mean_proportion,
                predictive.probability_replicated_difference_at_least_observed,
            ]
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            {
                return Err(BayesError::WorkerContract(
                    "Dirichlet-multinomial predictive summary is invalid".into(),
                ));
            }
        }
        validate_summary(&self.posterior.concentration, SummarySupport::Positive)?;
        if !(0.0..1.0).contains(&self.posterior.overdispersion_mean)
            || (reference_sum - 1.0).abs() > 1e-6
            || (comparison_sum - 1.0).abs() > 1e-6
            || difference_sum.abs() > 1e-6
        {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial composition constraints are invalid".into(),
            ));
        }
        let diagnostics_pass =
            diagnostics_satisfy_policy(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: DirichletMultinomialGroupWorkerRequest,
        input: DirichletMultinomialGroupInputIdentity,
    ) -> DirichletMultinomialGroupResult {
        DirichletMultinomialGroupResult {
            format: "marklab.bayesian_dirichlet_multinomial_group",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            sampling: self.sampling,
            fit_state: self.fit_state,
            posterior: self.posterior,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            claim_status: if self.fit_state == FitState::Complete {
                ClaimStatus::ExperimentalPatientMulticlassGroupComposition
            } else {
                ClaimStatus::DiagnosticOnlyNonconverged
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_summary(
    summary: &McseScalarSummary,
    support: SummarySupport,
) -> Result<(), BayesError> {
    validate_mcse_summary(
        summary,
        support,
        "Dirichlet-multinomial posterior summary is invalid",
    )
}

#[derive(Clone, Debug, Serialize)]
pub struct DirichletMultinomialGroupInputIdentity {
    pub path: String,
    pub patient_data_sha256: String,
    pub patient_count: usize,
    pub class_count: usize,
    pub reference_patients: usize,
    pub comparison_patients: usize,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: DirichletMultinomialGroupModelIr,
    pub input: DirichletMultinomialGroupInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: DirichletMultinomialGroupPosterior,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: DirichletMultinomialGroupPosteriorPredictive,
    pub seed: u64,
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_incomplete_patient_count_vectors() {
        let patients = (0..8)
            .map(|index| DirichletMultinomialPatientData {
                patient_id: format!("p-{index}"),
                group: if index < 4 { "MSS" } else { "MSI" }.into(),
                counts: if index == 7 {
                    vec![2, 3]
                } else {
                    vec![2, 3, 4]
                },
            })
            .collect();
        assert!(matches!(
            DirichletMultinomialGroupWorkerRequest::new(
                DirichletMultinomialGroupSpec {
                    reference_group: "MSS".into(),
                    comparison_group: "MSI".into(),
                    class_ids: vec!["a".into(), "b".into(), "c".into()],
                    logit_prior_sd: 1.5,
                    group_effect_prior_sd: 1.0,
                    concentration_prior_sd: 50.0,
                    patients,
                },
                NutsSamplingSpec {
                    chains: 2,
                    tune_per_chain: 100,
                    draws_per_chain: 100,
                    target_accept: 0.9,
                    seed: 1,
                },
                "0".repeat(64),
                "1".repeat(64),
                60,
            ),
            Err(BayesError::InvalidSpec(_))
        ));
    }
}
