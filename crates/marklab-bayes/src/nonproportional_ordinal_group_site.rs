use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    model::{PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    ordinal_group::{OrdinalCutpointPosterior, OrdinalLevelPosterior, OrdinalLevelPredictive},
    ordinal_group_site_hierarchy::OrdinalGroupSitePatientData,
    sha256_hex,
    validation::{diagnostics_satisfy_policy, is_lower_hex_sha256},
    BackendContract, BayesError, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct NonproportionalOrdinalGroupSiteSpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub ordered_levels: Vec<String>,
    pub cutpoint_prior_sd: f64,
    pub site_intercept_sd_prior_sd: f64,
    pub patients: Vec<OrdinalGroupSitePatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NonproportionalOrdinalGroupSiteModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub reference_group: String,
    pub comparison_group: String,
    pub ordered_levels: Vec<String>,
    pub group_cutpoint_prior: &'static str,
    pub cutpoint_prior_sd: f64,
    pub site_intercept_prior: &'static str,
    pub site_intercept_sd_prior_sd: f64,
    pub threshold_effect_definition: &'static str,
    pub probability_constraint: &'static str,
    pub group_probability_standardization: &'static str,
    pub likelihood: &'static str,
    pub observation_unit: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct NonproportionalOrdinalGroupSiteResources {
    pub maximum_patients: u32,
    pub maximum_sites: u32,
    pub maximum_levels: u32,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NonproportionalOrdinalGroupSiteWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: NonproportionalOrdinalGroupSiteModelIr,
    pub site_ids: Vec<String>,
    pub patients: Vec<OrdinalGroupSitePatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: NonproportionalOrdinalGroupSiteResources,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl NonproportionalOrdinalGroupSiteWorkerRequest {
    pub fn new(
        spec: NonproportionalOrdinalGroupSiteSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !valid_name(&spec.reference_group)
            || !valid_name(&spec.comparison_group)
            || spec.reference_group == spec.comparison_group
            || !(2..=8).contains(&spec.ordered_levels.len())
            || !spec.cutpoint_prior_sd.is_finite()
            || spec.cutpoint_prior_sd <= 0.0
            || !spec.site_intercept_sd_prior_sd.is_finite()
            || spec.site_intercept_sd_prior_sd <= 0.0
            || !(32..=1_024).contains(&spec.patients.len())
            || !is_lower_hex_sha256(&environment_lock_sha256)
            || !is_lower_hex_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "nonproportional ordinal controls or identities are invalid".into(),
            ));
        }
        if spec.ordered_levels.iter().any(|level| !valid_name(level))
            || spec.ordered_levels.iter().collect::<BTreeSet<_>>().len()
                != spec.ordered_levels.len()
        {
            return Err(BayesError::InvalidSpec(
                "nonproportional ordinal levels are invalid".into(),
            ));
        }
        let mut patients = spec.patients;
        patients.sort_by(|a, b| a.patient_id.cmp(&b.patient_id));
        let mut patient_ids = BTreeSet::new();
        let mut site_groups = BTreeMap::<String, [usize; 2]>::new();
        let mut level_counts = vec![0_usize; spec.ordered_levels.len()];
        for patient in &patients {
            if !valid_name(&patient.patient_id)
                || !valid_name(&patient.site_id)
                || !patient_ids.insert(patient.patient_id.as_str())
                || (patient.group != spec.reference_group && patient.group != spec.comparison_group)
                || patient.outcome_code as usize >= spec.ordered_levels.len()
            {
                return Err(BayesError::InvalidSpec(
                    "nonproportional ordinal patient rows are invalid".into(),
                ));
            }
            site_groups.entry(patient.site_id.clone()).or_default()
                [usize::from(patient.group == spec.comparison_group)] += 1;
            level_counts[patient.outcome_code as usize] += 1;
        }
        if !(8..=64).contains(&site_groups.len())
            || site_groups
                .values()
                .any(|counts| counts[0] < 2 || counts[1] < 2)
            || level_counts.contains(&0)
        {
            return Err(BayesError::InvalidSpec(
                "nonproportional ordinal requires 8-64 sites, both groups per site, and every level"
                    .into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        if u64::from(sampling.chains)
            * (u64::from(sampling.tune_per_chain) + u64::from(sampling.draws_per_chain))
            > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "nonproportional ordinal iteration limit exceeded".into(),
            ));
        }
        let site_ids = site_groups.into_keys().collect();
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
            model: NonproportionalOrdinalGroupSiteModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "patient_nonproportional_ordinal_group_site",
                reference_group: spec.reference_group,
                comparison_group: spec.comparison_group,
                ordered_levels: spec.ordered_levels,
                group_cutpoint_prior: "two_independent_ordered_normal_vectors",
                cutpoint_prior_sd: spec.cutpoint_prior_sd,
                site_intercept_prior: "sum_zero_noncentered_normal",
                site_intercept_sd_prior_sd: spec.site_intercept_sd_prior_sd,
                threshold_effect_definition: "reference_cutpoint_minus_comparison_cutpoint",
                probability_constraint: "each_group_cutpoint_vector_strictly_ordered",
                group_probability_standardization: "equal_weight_sites",
                likelihood: "ordered_logistic_patient_outcome",
                observation_unit: "one_complete_ordered_outcome_per_patient",
                biological_unit: "patient",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            site_ids,
            patients,
            sampling,
            resources: NonproportionalOrdinalGroupSiteResources {
                maximum_patients: 1_024,
                maximum_sites: 64,
                maximum_levels: 8,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalThresholdGroupEffect {
    pub threshold_after_level: String,
    pub summary: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonproportionalOrdinalSitePosterior {
    pub site_id: String,
    pub intercept: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonproportionalOrdinalGroupSitePosterior {
    pub reference_cutpoints: Vec<OrdinalCutpointPosterior>,
    pub comparison_cutpoints: Vec<OrdinalCutpointPosterior>,
    pub threshold_group_log_odds_effects: Vec<OrdinalThresholdGroupEffect>,
    pub site_intercept_sd: SarScalarSummary,
    pub sites: Vec<NonproportionalOrdinalSitePosterior>,
    pub levels: Vec<OrdinalLevelPosterior>,
    pub reference_expected_code: SarScalarSummary,
    pub comparison_expected_code: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonproportionalOrdinalGroupSitePosteriorPredictive {
    pub levels: Vec<OrdinalLevelPredictive>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonproportionalOrdinalGroupSiteWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: NonproportionalOrdinalGroupSitePosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: NonproportionalOrdinalGroupSitePosteriorPredictive,
}

impl NonproportionalOrdinalGroupSiteWorkerResult {
    pub fn validate(
        &self,
        request: &NonproportionalOrdinalGroupSiteWorkerRequest,
        sha: &str,
    ) -> Result<(), BayesError> {
        let levels = &request.model.ordered_levels;
        let thresholds = levels.len() - 1;
        if self.format != "marklab.pymc_nonproportional_ordinal_group_site_worker_result"
            || self.version != 1
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != sha
            || self.posterior.reference_cutpoints.len() != thresholds
            || self.posterior.comparison_cutpoints.len() != thresholds
            || self.posterior.threshold_group_log_odds_effects.len() != thresholds
            || self.posterior.sites.len() != request.site_ids.len()
            || self.posterior.levels.len() != levels.len()
            || self.posterior_predictive.levels.len() != levels.len()
        {
            return Err(BayesError::WorkerContract(
                "nonproportional ordinal identity or dimensions mismatch".into(),
            ));
        }
        let expected =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.completed_draws != expected
            || self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
        {
            return Err(BayesError::WorkerContract(
                "nonproportional ordinal sampling mismatch".into(),
            ));
        }
        let mut previous = [f64::NEG_INFINITY; 2];
        for index in 0..thresholds {
            for (which, cutpoint) in [
                &self.posterior.reference_cutpoints[index],
                &self.posterior.comparison_cutpoints[index],
            ]
            .into_iter()
            .enumerate()
            {
                if cutpoint.lower_level != levels[index]
                    || cutpoint.upper_level != levels[index + 1]
                    || cutpoint.summary.mean <= previous[which]
                {
                    return Err(BayesError::WorkerContract(
                        "nonproportional cutpoint order mismatch".into(),
                    ));
                }
                validate_summary(&cutpoint.summary, None)?;
                previous[which] = cutpoint.summary.mean;
            }
            let effect = &self.posterior.threshold_group_log_odds_effects[index];
            if effect.threshold_after_level != levels[index] {
                return Err(BayesError::WorkerContract(
                    "threshold effect order mismatch".into(),
                ));
            }
            validate_summary(&effect.summary, None)?;
        }
        validate_summary(
            &self.posterior.site_intercept_sd,
            Some((0.0, f64::INFINITY)),
        )?;
        for (site, id) in self.posterior.sites.iter().zip(&request.site_ids) {
            if site.site_id != *id {
                return Err(BayesError::WorkerContract("site order mismatch".into()));
            }
            validate_summary(&site.intercept, None)?;
        }
        let reference_count = request
            .patients
            .iter()
            .filter(|p| p.group == request.model.reference_group)
            .count() as f64;
        let comparison_count = request.patients.len() as f64 - reference_count;
        let mut sums = [0.0; 2];
        for (index, ((posterior, predictive), level)) in self
            .posterior
            .levels
            .iter()
            .zip(&self.posterior_predictive.levels)
            .zip(levels)
            .enumerate()
        {
            if posterior.level != *level || predictive.level != *level {
                return Err(BayesError::WorkerContract("level order mismatch".into()));
            }
            validate_summary(&posterior.reference_probability, Some((0.0, 1.0)))?;
            validate_summary(&posterior.comparison_probability, Some((0.0, 1.0)))?;
            validate_summary(
                &posterior.difference_comparison_minus_reference,
                Some((-1.0, 1.0)),
            )?;
            sums[0] += posterior.reference_probability.mean;
            sums[1] += posterior.comparison_probability.mean;
            let observed = [
                request
                    .patients
                    .iter()
                    .filter(|p| {
                        p.group == request.model.reference_group && p.outcome_code as usize == index
                    })
                    .count() as f64
                    / reference_count,
                request
                    .patients
                    .iter()
                    .filter(|p| {
                        p.group == request.model.comparison_group
                            && p.outcome_code as usize == index
                    })
                    .count() as f64
                    / comparison_count,
            ];
            if (predictive.observed_reference_proportion - observed[0]).abs() > 1e-12
                || (predictive.observed_comparison_proportion - observed[1]).abs() > 1e-12
            {
                return Err(BayesError::WorkerContract("observed PPC mismatch".into()));
            }
        }
        if sums.into_iter().any(|sum| (sum - 1.0).abs() > 1e-6) {
            return Err(BayesError::WorkerContract(
                "probabilities do not sum to one".into(),
            ));
        }
        validate_summary(
            &self.posterior.reference_expected_code,
            Some((0.0, (levels.len() - 1) as f64)),
        )?;
        validate_summary(
            &self.posterior.comparison_expected_code,
            Some((0.0, (levels.len() - 1) as f64)),
        )?;
        let pass = diagnostics_satisfy_policy(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != pass {
            return Err(BayesError::WorkerContract(
                "fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }
    pub fn into_result(
        self,
        request: NonproportionalOrdinalGroupSiteWorkerRequest,
        input: NonproportionalOrdinalGroupSiteInputIdentity,
    ) -> NonproportionalOrdinalGroupSiteResult {
        NonproportionalOrdinalGroupSiteResult {
            format: "marklab.bayesian_nonproportional_ordinal_group_site",
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
                "experimental_patient_nonproportional_ordinal_group_association"
            } else {
                "diagnostic_only_nonconverged"
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_summary(
    summary: &SarScalarSummary,
    support: Option<(f64, f64)>,
) -> Result<(), BayesError> {
    if ![
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]
    .iter()
    .all(|v| v.is_finite())
        || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || support.is_some_and(|(l, u)| {
            summary.mean < l
                || summary.mean > u
                || summary.interval_lower < l
                || summary.interval_upper > u
        })
    {
        return Err(BayesError::WorkerContract(
            "nonproportional ordinal posterior summary invalid".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct NonproportionalOrdinalGroupSiteInputIdentity {
    pub path: String,
    pub patient_data_sha256: String,
    pub patient_count: usize,
    pub site_count: usize,
    pub level_count: usize,
    pub reference_patients: usize,
    pub comparison_patients: usize,
}
pub fn nonproportional_ordinal_group_site_data_sha256(
    patients: &[OrdinalGroupSitePatientData],
) -> Result<String, BayesError> {
    Ok(sha256_hex(&serde_json::to_vec(patients)?))
}

#[derive(Debug, Serialize)]
pub struct NonproportionalOrdinalGroupSiteResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: NonproportionalOrdinalGroupSiteModelIr,
    pub input: NonproportionalOrdinalGroupSiteInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: NonproportionalOrdinalGroupSitePosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: NonproportionalOrdinalGroupSitePosteriorPredictive,
    pub seed: u64,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_group_confounded_site() {
        let mut patients = Vec::new();
        for site in 0..8 {
            for patient in 0..4 {
                for group in ["MSS", if site == 7 { "MSS" } else { "MSI" }] {
                    patients.push(OrdinalGroupSitePatientData {
                        patient_id: format!("s{site}-{group}-{patient}"),
                        site_id: format!("s{site}"),
                        group: group.into(),
                        outcome_code: if group == "MSS" {
                            (patient % 2) as u32
                        } else {
                            2 + (patient % 2) as u32
                        },
                    });
                }
            }
        }
        assert!(NonproportionalOrdinalGroupSiteWorkerRequest::new(
            NonproportionalOrdinalGroupSiteSpec {
                reference_group: "MSS".into(),
                comparison_group: "MSI".into(),
                ordered_levels: vec!["I".into(), "II".into(), "III".into(), "IV".into()],
                cutpoint_prior_sd: 2.0,
                site_intercept_sd_prior_sd: 1.0,
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
        )
        .is_err());
    }
}
