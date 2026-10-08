use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    validation::{
        diagnostics_satisfy_policy, is_lower_hex_sha256, validate_scalar_summary, SummarySupport,
    },
    BackendContract, BayesError, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_negative_binomial_hierarchy_request";
const RESULT_FORMAT: &str = "marklab.negative_binomial_hierarchy";
const CLAIM_COMPLETE: &str = "experimental_conditional_within_patient_association";
const CLAIM_NONCONVERGED: &str = "diagnostic_only_nonconverged";
const MAXIMUM_PATIENTS: usize = 64;
const MAXIMUM_SLIDES: usize = 128;
const MAXIMUM_OBSERVATIONS: usize = 512;
const MAXIMUM_TOTAL_ITERATIONS: u64 = 100_000;
const MAXIMUM_DRAW_OBSERVATION_PRODUCTS: u64 = 16_000_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyObservation {
    pub patient_id: String,
    pub slide_id: String,
    pub roi_id: String,
    pub count: u32,
    pub area: f64,
    pub predictor: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyPriors {
    pub intercept_mean: f64,
    pub intercept_sd: f64,
    pub slope_sd: f64,
    pub log_dispersion_mean: f64,
    pub log_dispersion_sd: f64,
    pub patient_intercept_sd_scale: f64,
    pub patient_slope_sd_scale: f64,
    pub slide_sd_scale: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchySpec {
    pub area_unit: String,
    pub predictor_name: String,
    pub predictor_unit: String,
    pub observations: Vec<NegativeBinomialHierarchyObservation>,
    pub priors: NegativeBinomialHierarchyPriors,
    pub sampling: NutsSamplingSpec,
    pub timeout_seconds: u64,
    pub maximum_draw_observation_products: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyDesign {
    pub patient_ids: Vec<String>,
    pub slide_ids: Vec<String>,
    pub patient_index: Vec<u32>,
    pub slide_index: Vec<u32>,
    pub patient_predictor_means: Vec<f64>,
    pub within_predictor: Vec<f64>,
    pub within_predictor_sd: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NegativeBinomialHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub spec: NegativeBinomialHierarchySpec,
    pub design: NegativeBinomialHierarchyDesign,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl NegativeBinomialHierarchyWorkerRequest {
    pub fn new(
        mut spec: NegativeBinomialHierarchySpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_lower_hex_sha256(&environment_lock_sha256) || !is_lower_hex_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "negative-binomial hierarchy backend identities must be lowercase SHA-256 digests"
                    .into(),
            ));
        }
        validate_controls(&spec)?;
        spec.observations.sort_by(|left, right| {
            left.patient_id
                .cmp(&right.patient_id)
                .then_with(|| left.slide_id.cmp(&right.slide_id))
                .then_with(|| left.roi_id.cmp(&right.roi_id))
        });
        let design = compile_design(&spec.observations)?;
        let policy = DiagnosticPolicy::default();
        let posterior_draws = u64::from(spec.sampling.chains)
            .checked_mul(u64::from(spec.sampling.draws_per_chain))
            .ok_or_else(|| BayesError::InvalidSpec("posterior draw count overflow".into()))?;
        let draws_with_prior = posterior_draws
            .checked_add(u64::from(policy.prior_predictive_draws))
            .ok_or_else(|| BayesError::InvalidSpec("predictive draw count overflow".into()))?;
        let products = (spec.observations.len() as u64)
            .checked_mul(draws_with_prior)
            .ok_or_else(|| BayesError::InvalidSpec("draw-observation products overflow".into()))?;
        if spec.maximum_draw_observation_products > MAXIMUM_DRAW_OBSERVATION_PRODUCTS
            || products > spec.maximum_draw_observation_products
        {
            return Err(BayesError::InvalidSpec(format!(
                "negative-binomial hierarchy requires {products} draw-observation products within a caller ceiling no greater than {MAXIMUM_DRAW_OBSERVATION_PRODUCTS}"
            )));
        }
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
            spec,
            design,
            diagnostic_policy: policy,
        })
    }
}

fn validate_controls(spec: &NegativeBinomialHierarchySpec) -> Result<(), BayesError> {
    for (value, name) in [
        (&spec.area_unit, "area unit"),
        (&spec.predictor_name, "predictor name"),
        (&spec.predictor_unit, "predictor unit"),
    ] {
        if value.is_empty() || value.len() > 128 || value.trim() != value {
            return Err(BayesError::InvalidSpec(format!(
                "negative-binomial hierarchy {name} must be an exact nonempty string"
            )));
        }
    }
    let p = &spec.priors;
    if !p.intercept_mean.is_finite()
        || !p.log_dispersion_mean.is_finite()
        || [
            p.intercept_sd,
            p.slope_sd,
            p.log_dispersion_sd,
            p.patient_intercept_sd_scale,
            p.patient_slope_sd_scale,
            p.slide_sd_scale,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(BayesError::InvalidSpec(
            "negative-binomial hierarchy priors must be finite with positive scales".into(),
        ));
    }
    spec.sampling.validate()?;
    let iterations_per_chain = u64::from(spec.sampling.tune_per_chain)
        .checked_add(u64::from(spec.sampling.draws_per_chain))
        .ok_or_else(|| BayesError::InvalidSpec("NUTS iteration count overflow".into()))?;
    let total_iterations = u64::from(spec.sampling.chains)
        .checked_mul(iterations_per_chain)
        .ok_or_else(|| BayesError::InvalidSpec("total NUTS iterations overflow".into()))?;
    if total_iterations > MAXIMUM_TOTAL_ITERATIONS {
        return Err(BayesError::InvalidSpec(format!(
            "negative-binomial hierarchy requested {total_iterations} NUTS iterations above the {MAXIMUM_TOTAL_ITERATIONS} limit"
        )));
    }
    if !(1..=3_600).contains(&spec.timeout_seconds) {
        return Err(BayesError::InvalidSpec(
            "negative-binomial hierarchy timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    Ok(())
}

fn compile_design(
    observations: &[NegativeBinomialHierarchyObservation],
) -> Result<NegativeBinomialHierarchyDesign, BayesError> {
    if observations.len() > MAXIMUM_OBSERVATIONS {
        return Err(BayesError::InvalidSpec(
            "negative-binomial hierarchy supports at most 512 ROI observations".into(),
        ));
    }
    let mut roi_ids = BTreeSet::new();
    let mut patient_rows = BTreeMap::<String, Vec<usize>>::new();
    let mut slide_rows = BTreeMap::<String, (String, Vec<usize>)>::new();
    for (index, row) in observations.iter().enumerate() {
        if !valid_id(&row.patient_id)
            || !valid_id(&row.slide_id)
            || !valid_id(&row.roi_id)
            || !roi_ids.insert(row.roi_id.as_str())
            || !row.area.is_finite()
            || row.area <= 0.0
            || !row.predictor.is_finite()
        {
            return Err(BayesError::InvalidSpec(
                "negative-binomial hierarchy ROI rows require exact unique IDs, positive finite area, and finite predictors".into(),
            ));
        }
        patient_rows
            .entry(row.patient_id.clone())
            .or_default()
            .push(index);
        let slide = slide_rows
            .entry(row.slide_id.clone())
            .or_insert_with(|| (row.patient_id.clone(), Vec::new()));
        if slide.0 != row.patient_id {
            return Err(BayesError::InvalidSpec(
                "negative-binomial hierarchy slide IDs must have exactly one patient parent".into(),
            ));
        }
        slide.1.push(index);
    }
    if !(8..=MAXIMUM_PATIENTS).contains(&patient_rows.len())
        || !(16..=MAXIMUM_SLIDES).contains(&slide_rows.len())
    {
        return Err(BayesError::InvalidSpec(
            "negative-binomial hierarchy requires 8-64 patients and 16-128 slides".into(),
        ));
    }
    for (patient, indices) in &patient_rows {
        let slide_count = indices
            .iter()
            .map(|index| observations[*index].slide_id.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if slide_count < 2 {
            return Err(BayesError::InvalidSpec(format!(
                "negative-binomial hierarchy patient {patient} requires at least two slides"
            )));
        }
    }
    for (slide, (_, indices)) in &slide_rows {
        if indices.len() < 4 {
            return Err(BayesError::InvalidSpec(format!(
                "negative-binomial hierarchy slide {slide} requires at least four ROIs"
            )));
        }
        let (minimum, maximum) = indices.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), index| {
                let value = observations[*index].predictor;
                (minimum.min(value), maximum.max(value))
            },
        );
        if maximum <= minimum {
            return Err(BayesError::InvalidSpec(format!(
                "negative-binomial hierarchy predictor does not vary within slide {slide}"
            )));
        }
    }

    let patient_ids = patient_rows.keys().cloned().collect::<Vec<_>>();
    let slide_ids = slide_rows.keys().cloned().collect::<Vec<_>>();
    let patient_lookup = patient_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let slide_lookup = slide_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let mut means = Vec::with_capacity(patient_ids.len());
    let mut centered = vec![0.0; observations.len()];
    for patient in &patient_ids {
        let indices = &patient_rows[patient];
        let origin = observations[indices[0]].predictor;
        let mean_offset = compensated_sum(
            indices
                .iter()
                .map(|index| observations[*index].predictor - origin),
        ) / indices.len() as f64;
        let mean = origin + mean_offset;
        if !mean.is_finite() {
            return Err(BayesError::InvalidSpec(
                "negative-binomial hierarchy patient predictor mean is nonfinite".into(),
            ));
        }
        means.push(mean);
        for index in indices {
            centered[*index] = (observations[*index].predictor - origin) - mean_offset;
        }
    }
    let maximum_centered = centered.iter().map(|value| value.abs()).fold(0.0, f64::max);
    let scale = maximum_centered
        * (compensated_sum(
            centered
                .iter()
                .map(|value| (value / maximum_centered).powi(2)),
        ) / observations.len() as f64)
            .sqrt();
    if !scale.is_finite() || scale <= 0.0 {
        return Err(BayesError::InvalidSpec(
            "negative-binomial hierarchy within-patient predictor scale is invalid".into(),
        ));
    }
    let within_predictor = centered
        .into_iter()
        .map(|value| value / scale)
        .collect::<Vec<_>>();
    if within_predictor.iter().any(|value| !value.is_finite()) {
        return Err(BayesError::InvalidSpec(
            "negative-binomial hierarchy standardized predictor is nonfinite".into(),
        ));
    }
    Ok(NegativeBinomialHierarchyDesign {
        patient_ids,
        slide_ids,
        patient_index: observations
            .iter()
            .map(|row| patient_lookup[&row.patient_id])
            .collect(),
        slide_index: observations
            .iter()
            .map(|row| slide_lookup[&row.slide_id])
            .collect(),
        patient_predictor_means: means,
        within_predictor,
        within_predictor_sd: scale,
    })
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.trim() == value
}

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let next = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
    }
    sum + correction
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyPosterior {
    pub alpha: SarScalarSummary,
    pub beta: SarScalarSummary,
    pub rate_ratio: SarScalarSummary,
    pub dispersion: SarScalarSummary,
    pub patient_intercept_sd: SarScalarSummary,
    pub patient_slope_sd: SarScalarSummary,
    pub slide_sd: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyPatientPosterior {
    pub patient_id: String,
    pub observation_count: u32,
    pub random_intercept: SarScalarSummary,
    pub slope: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchySlidePosterior {
    pub slide_id: String,
    pub patient_id: String,
    pub observation_count: u32,
    pub random_intercept: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyRoiPosterior {
    pub roi_id: String,
    pub patient_id: String,
    pub slide_id: String,
    pub count: u32,
    pub area: f64,
    pub predictor: f64,
    pub within_predictor: f64,
    pub expected_count: SarScalarSummary,
    pub expected_rate: SarScalarSummary,
    pub zero_probability: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyPriorPatientRate {
    pub patient_id: String,
    pub mean: f64,
    pub sd: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyPriorPredictive {
    pub draws: u32,
    pub patient_rates: Vec<NegativeBinomialHierarchyPriorPatientRate>,
    pub zero_fraction_mean: f64,
    pub zero_fraction_sd: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyPatientPredictive {
    pub patient_id: String,
    pub observed_mean_rate: f64,
    pub replicated_mean_rate: f64,
    pub replicated_mean_rate_sd: f64,
    pub observed_pearson_discrepancy: f64,
    pub replicated_pearson_discrepancy_mean: f64,
    pub probability_replicated_pearson_at_least_observed: f64,
    pub pearson_tail_probability_mcse: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyPosteriorPredictive {
    pub observed_zero_fraction: f64,
    pub replicated_zero_fraction_mean: f64,
    pub replicated_zero_fraction_sd: f64,
    pub probability_replicated_zero_fraction_at_least_observed: f64,
    pub zero_fraction_tail_probability_mcse: Option<f64>,
    pub patients: Vec<NegativeBinomialHierarchyPatientPredictive>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyModel {
    pub likelihood: String,
    pub linear_predictor: String,
    pub predictor_centering: String,
    pub area_unit: String,
    pub predictor_name: String,
    pub predictor_unit: String,
    pub priors: NegativeBinomialHierarchyPriors,
}

impl NegativeBinomialHierarchyModel {
    fn for_spec(spec: &NegativeBinomialHierarchySpec) -> Self {
        Self {
            likelihood: "negative_binomial_2".into(),
            linear_predictor: "log_area_offset_plus_within_patient_slope_and_patient_intercept_slope_and_nested_slide_intercept".into(),
            predictor_centering: "within_patient_then_pooled_within_sd".into(),
            area_unit: spec.area_unit.clone(),
            predictor_name: spec.predictor_name.clone(),
            predictor_unit: spec.predictor_unit.clone(),
            priors: spec.priors.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeBinomialHierarchyResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub claim_status: String,
    pub model: NegativeBinomialHierarchyModel,
    pub sampling: SamplingSummary,
    pub diagnostics: NormalMeanDiagnostics,
    pub design: NegativeBinomialHierarchyDesign,
    pub posterior: NegativeBinomialHierarchyPosterior,
    pub patients: Vec<NegativeBinomialHierarchyPatientPosterior>,
    pub slides: Vec<NegativeBinomialHierarchySlidePosterior>,
    pub rois: Vec<NegativeBinomialHierarchyRoiPosterior>,
    pub prior_predictive: NegativeBinomialHierarchyPriorPredictive,
    pub posterior_predictive: NegativeBinomialHierarchyPosteriorPredictive,
}

impl NegativeBinomialHierarchyResult {
    pub fn validate(
        &self,
        request: &NegativeBinomialHierarchyWorkerRequest,
        expected_request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.fit_state == FitState::ApproximateOnly
            || self.backend.name != "numpyro"
            || self.backend.version != NUMPYRO_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.jax_version != JAX_VERSION
            || self.request_sha256 != expected_request_sha256
            || self.model != NegativeBinomialHierarchyModel::for_spec(&request.spec)
            || self.design != request.design
        {
            return Err(BayesError::WorkerContract(
                "negative-binomial hierarchy result identity mismatch".into(),
            ));
        }
        let expected_draws = u64::from(request.spec.sampling.chains)
            * u64::from(request.spec.sampling.draws_per_chain);
        if self.sampling.chains != request.spec.sampling.chains
            || self.sampling.tune_per_chain != request.spec.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.spec.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.patients.len() != request.design.patient_ids.len()
            || self.slides.len() != request.design.slide_ids.len()
            || self.rois.len() != request.spec.observations.len()
            || self.prior_predictive.draws != request.diagnostic_policy.prior_predictive_draws
            || self.prior_predictive.patient_rates.len() != request.design.patient_ids.len()
            || self.posterior_predictive.patients.len() != request.design.patient_ids.len()
        {
            return Err(BayesError::WorkerContract(
                "negative-binomial hierarchy sampling or result dimensions mismatch".into(),
            ));
        }
        if ![
            self.diagnostics.r_hat,
            self.diagnostics.ess_bulk,
            self.diagnostics.ess_tail,
            self.diagnostics.mcse_mean,
            self.diagnostics.mcse_sd,
            self.diagnostics.minimum_ebfmi,
        ]
        .iter()
        .all(|value| value.is_finite())
            || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
        {
            return Err(BayesError::WorkerContract(
                "negative-binomial hierarchy diagnostics are invalid or nonfinite".into(),
            ));
        }
        for summary in [&self.posterior.alpha, &self.posterior.beta] {
            real_summary(summary)?;
        }
        for summary in [
            &self.posterior.rate_ratio,
            &self.posterior.dispersion,
            &self.posterior.patient_intercept_sd,
            &self.posterior.patient_slope_sd,
            &self.posterior.slide_sd,
        ] {
            positive_summary(summary)?;
        }
        let tolerance = 1e-10;
        if (self.posterior.rate_ratio.interval_lower - self.posterior.beta.interval_lower.exp())
            .abs()
            > tolerance * self.posterior.rate_ratio.interval_lower.max(1.0)
            || (self.posterior.rate_ratio.interval_upper - self.posterior.beta.interval_upper.exp())
                .abs()
                > tolerance * self.posterior.rate_ratio.interval_upper.max(1.0)
        {
            return Err(BayesError::WorkerContract(
                "negative-binomial hierarchy rate-ratio interval disagrees with exp(beta)".into(),
            ));
        }
        self.validate_rows(request)?;
        self.validate_predictive(request)?;
        let diagnostics_pass =
            diagnostics_satisfy_policy(&self.diagnostics, &request.diagnostic_policy);
        let expected_claim = if self.fit_state == FitState::Complete {
            CLAIM_COMPLETE
        } else {
            CLAIM_NONCONVERGED
        };
        if diagnostics_pass != (self.fit_state == FitState::Complete)
            || self.claim_status != expected_claim
        {
            return Err(BayesError::WorkerContract(
                "negative-binomial hierarchy fit state or claim disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    fn validate_rows(
        &self,
        request: &NegativeBinomialHierarchyWorkerRequest,
    ) -> Result<(), BayesError> {
        let patient_counts = counts_by_patient(&request.spec.observations);
        for ((actual, patient_id), prior) in self
            .patients
            .iter()
            .zip(&request.design.patient_ids)
            .zip(&self.prior_predictive.patient_rates)
        {
            if actual.patient_id != *patient_id
                || prior.patient_id != *patient_id
                || actual.observation_count != patient_counts[patient_id] as u32
            {
                return Err(BayesError::WorkerContract(
                    "negative-binomial hierarchy patient identity mismatch".into(),
                ));
            }
            real_summary(&actual.random_intercept)?;
            real_summary(&actual.slope)?;
            finite_nonnegative(prior.mean, "prior patient rate mean")?;
            finite_nonnegative(prior.sd, "prior patient rate SD")?;
        }
        let slide_counts = counts_by_slide(&request.spec.observations);
        let slide_parents = request
            .spec
            .observations
            .iter()
            .map(|row| (row.slide_id.as_str(), row.patient_id.as_str()))
            .collect::<BTreeMap<_, _>>();
        for (actual, slide_id) in self.slides.iter().zip(&request.design.slide_ids) {
            if actual.slide_id != *slide_id
                || actual.patient_id != slide_parents[slide_id.as_str()]
                || actual.observation_count != slide_counts[slide_id] as u32
            {
                return Err(BayesError::WorkerContract(
                    "negative-binomial hierarchy slide identity mismatch".into(),
                ));
            }
            real_summary(&actual.random_intercept)?;
        }
        for ((actual, source), within) in self
            .rois
            .iter()
            .zip(&request.spec.observations)
            .zip(&request.design.within_predictor)
        {
            if actual.roi_id != source.roi_id
                || actual.patient_id != source.patient_id
                || actual.slide_id != source.slide_id
                || actual.count != source.count
                || actual.area != source.area
                || actual.predictor != source.predictor
                || actual.within_predictor != *within
            {
                return Err(BayesError::WorkerContract(
                    "negative-binomial hierarchy ROI identity mismatch".into(),
                ));
            }
            positive_summary(&actual.expected_count)?;
            positive_summary(&actual.expected_rate)?;
            unit_summary(&actual.zero_probability)?;
            if !scaled_summary_equal(&actual.expected_count, &actual.expected_rate, source.area) {
                return Err(BayesError::WorkerContract(
                    "negative-binomial hierarchy expected count and rate disagree with area".into(),
                ));
            }
        }
        Ok(())
    }

    fn validate_predictive(
        &self,
        request: &NegativeBinomialHierarchyWorkerRequest,
    ) -> Result<(), BayesError> {
        finite_unit(
            self.prior_predictive.zero_fraction_mean,
            "prior zero fraction",
        )?;
        finite_nonnegative(
            self.prior_predictive.zero_fraction_sd,
            "prior zero-fraction SD",
        )?;
        let ppc = &self.posterior_predictive;
        finite_unit(ppc.observed_zero_fraction, "observed zero fraction")?;
        finite_unit(
            ppc.replicated_zero_fraction_mean,
            "replicated zero fraction",
        )?;
        finite_nonnegative(
            ppc.replicated_zero_fraction_sd,
            "replicated zero-fraction SD",
        )?;
        finite_unit(
            ppc.probability_replicated_zero_fraction_at_least_observed,
            "zero-fraction tail probability",
        )?;
        if let Some(mcse) = ppc.zero_fraction_tail_probability_mcse {
            finite_nonnegative(mcse, "zero-fraction tail-probability MCSE")?;
        }
        let observed_zero_fraction = request
            .spec
            .observations
            .iter()
            .filter(|row| row.count == 0)
            .count() as f64
            / request.spec.observations.len() as f64;
        if !close(ppc.observed_zero_fraction, observed_zero_fraction) {
            return Err(BayesError::WorkerContract(
                "negative-binomial hierarchy posterior predictive changed the observed zero fraction"
                    .into(),
            ));
        }
        for (actual, patient_id) in ppc.patients.iter().zip(&request.design.patient_ids) {
            let rows = request
                .spec
                .observations
                .iter()
                .filter(|row| row.patient_id == *patient_id);
            let (count, area) = rows.fold((0_u64, 0.0), |(count, area), row| {
                (count + u64::from(row.count), area + row.area)
            });
            let observed_rate = count as f64 / area;
            if actual.patient_id != *patient_id || !close(actual.observed_mean_rate, observed_rate)
            {
                return Err(BayesError::WorkerContract(
                    "negative-binomial hierarchy patient predictive identity changed".into(),
                ));
            }
            for (value, name) in [
                (actual.replicated_mean_rate, "replicated patient mean rate"),
                (
                    actual.replicated_mean_rate_sd,
                    "replicated patient mean-rate SD",
                ),
                (
                    actual.observed_pearson_discrepancy,
                    "observed Pearson discrepancy",
                ),
                (
                    actual.replicated_pearson_discrepancy_mean,
                    "replicated Pearson discrepancy",
                ),
            ] {
                finite_nonnegative(value, name)?;
            }
            finite_unit(
                actual.probability_replicated_pearson_at_least_observed,
                "Pearson tail probability",
            )?;
            if let Some(mcse) = actual.pearson_tail_probability_mcse {
                finite_nonnegative(mcse, "Pearson tail-probability MCSE")?;
            }
        }
        Ok(())
    }
}

fn real_summary(summary: &SarScalarSummary) -> Result<(), BayesError> {
    validate_scalar_summary(
        summary,
        SummarySupport::Real,
        "negative-binomial hierarchy real posterior summary is invalid",
    )
}

fn positive_summary(summary: &SarScalarSummary) -> Result<(), BayesError> {
    validate_scalar_summary(
        summary,
        SummarySupport::Positive,
        "negative-binomial hierarchy positive posterior summary is invalid",
    )
}

fn unit_summary(summary: &SarScalarSummary) -> Result<(), BayesError> {
    validate_scalar_summary(
        summary,
        SummarySupport::Unit,
        "negative-binomial hierarchy probability posterior summary is invalid",
    )
}

fn scaled_summary_equal(count: &SarScalarSummary, rate: &SarScalarSummary, area: f64) -> bool {
    [
        (count.mean, rate.mean),
        (count.sd, rate.sd),
        (count.interval_lower, rate.interval_lower),
        (count.interval_upper, rate.interval_upper),
    ]
    .iter()
    .all(|(count, rate)| close(*count, *rate * area))
}

fn finite_nonnegative(value: f64, name: &str) -> Result<(), BayesError> {
    if !value.is_finite() || value < 0.0 {
        return Err(BayesError::WorkerContract(format!(
            "negative-binomial hierarchy {name} is invalid"
        )));
    }
    Ok(())
}

fn finite_unit(value: f64, name: &str) -> Result<(), BayesError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(BayesError::WorkerContract(format!(
            "negative-binomial hierarchy {name} is invalid"
        )));
    }
    Ok(())
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

fn counts_by_patient(
    observations: &[NegativeBinomialHierarchyObservation],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in observations {
        *counts.entry(row.patient_id.clone()).or_default() += 1;
    }
    counts
}

fn counts_by_slide(
    observations: &[NegativeBinomialHierarchyObservation],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in observations {
        *counts.entry(row.slide_id.clone()).or_default() += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> NegativeBinomialHierarchySpec {
        let mut observations = Vec::new();
        for patient in (0..8).rev() {
            for slide in (0..2).rev() {
                for roi in (0..4).rev() {
                    observations.push(NegativeBinomialHierarchyObservation {
                        patient_id: format!("patient-{patient:02}"),
                        slide_id: format!("slide-{patient:02}-{slide}"),
                        roi_id: format!("roi-{patient:02}-{slide}-{roi}"),
                        count: roi % 3,
                        area: 1.0,
                        predictor: 1_000_000_000_000.0 + patient as f64 * 100.0 + roi as f64,
                    });
                }
            }
        }
        NegativeBinomialHierarchySpec {
            area_unit: "square_millimetre".into(),
            predictor_name: "declared_score".into(),
            predictor_unit: "arbitrary_unit".into(),
            observations,
            priors: NegativeBinomialHierarchyPriors {
                intercept_mean: 0.0,
                intercept_sd: 2.0,
                slope_sd: 1.0,
                log_dispersion_mean: 0.0,
                log_dispersion_sd: 1.0,
                patient_intercept_sd_scale: 1.0,
                patient_slope_sd_scale: 0.5,
                slide_sd_scale: 0.5,
            },
            sampling: NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 100,
                draws_per_chain: 100,
                target_accept: 0.9,
                seed: 42,
            },
            timeout_seconds: 180,
            maximum_draw_observation_products: 100_000,
        }
    }

    fn request() -> NegativeBinomialHierarchyWorkerRequest {
        NegativeBinomialHierarchyWorkerRequest::new(spec(), "a".repeat(64), "b".repeat(64))
            .expect("valid request")
    }

    fn summary(mean: f64, sd: f64, lower: f64, upper: f64) -> serde_json::Value {
        serde_json::json!({
            "mean": mean,
            "sd": sd,
            "interval_lower": lower,
            "interval_upper": upper,
        })
    }

    fn result_value(request: &NegativeBinomialHierarchyWorkerRequest) -> serde_json::Value {
        let beta_lower: f64 = 0.1;
        let beta_upper: f64 = 0.3;
        let patients = request
            .design
            .patient_ids
            .iter()
            .map(|patient_id| {
                serde_json::json!({
                    "patient_id": patient_id,
                    "observation_count": 8,
                    "random_intercept": summary(0.0, 0.1, -0.2, 0.2),
                    "slope": summary(0.2, 0.1, 0.0, 0.4),
                })
            })
            .collect::<Vec<_>>();
        let slides = request
            .design
            .slide_ids
            .iter()
            .map(|slide_id| {
                let patient_id = format!("patient-{}", &slide_id[6..8]);
                serde_json::json!({
                    "slide_id": slide_id,
                    "patient_id": patient_id,
                    "observation_count": 4,
                    "random_intercept": summary(0.0, 0.1, -0.2, 0.2),
                })
            })
            .collect::<Vec<_>>();
        let rois = request
            .spec
            .observations
            .iter()
            .zip(&request.design.within_predictor)
            .map(|(row, within)| {
                serde_json::json!({
                    "roi_id": row.roi_id,
                    "patient_id": row.patient_id,
                    "slide_id": row.slide_id,
                    "count": row.count,
                    "area": row.area,
                    "predictor": row.predictor,
                    "within_predictor": within,
                    "expected_count": summary(2.0, 0.2, 1.0, 3.0),
                    "expected_rate": summary(2.0, 0.2, 1.0, 3.0),
                    "zero_probability": summary(0.25, 0.05, 0.15, 0.35),
                })
            })
            .collect::<Vec<_>>();
        let prior_rates = request
            .design
            .patient_ids
            .iter()
            .map(|patient_id| serde_json::json!({"patient_id": patient_id, "mean": 2.0, "sd": 1.0}))
            .collect::<Vec<_>>();
        let predictive_patients = request
            .design
            .patient_ids
            .iter()
            .map(|patient_id| {
                serde_json::json!({
                    "patient_id": patient_id,
                    "observed_mean_rate": 0.75,
                    "replicated_mean_rate": 0.8,
                    "replicated_mean_rate_sd": 0.2,
                    "observed_pearson_discrepancy": 7.0,
                    "replicated_pearson_discrepancy_mean": 8.0,
                    "probability_replicated_pearson_at_least_observed": 0.55,
                    "pearson_tail_probability_mcse": null,
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "format": RESULT_FORMAT,
            "version": 1,
            "backend": {
                "name": "numpyro",
                "version": NUMPYRO_VERSION,
                "python_version": "3.12",
                "environment_lock_sha256": "a".repeat(64),
                "worker_sha256": "b".repeat(64),
            },
            "jax_version": JAX_VERSION,
            "request_sha256": "request",
            "fit_state": "complete",
            "claim_status": CLAIM_COMPLETE,
            "model": {
                "likelihood": "negative_binomial_2",
                "linear_predictor": "log_area_offset_plus_within_patient_slope_and_patient_intercept_slope_and_nested_slide_intercept",
                "predictor_centering": "within_patient_then_pooled_within_sd",
                "area_unit": request.spec.area_unit,
                "predictor_name": request.spec.predictor_name,
                "predictor_unit": request.spec.predictor_unit,
                "priors": request.spec.priors,
            },
            "sampling": {
                "chains": 2,
                "tune_per_chain": 100,
                "draws_per_chain": 100,
                "completed_draws": 200,
            },
            "diagnostics": {
                "prior_predictive_finite": true,
                "posterior_finite": true,
                "r_hat": 1.0,
                "ess_bulk": 500.0,
                "ess_tail": 500.0,
                "mcse_mean": 0.01,
                "mcse_sd": 0.01,
                "minimum_ebfmi": 0.8,
                "divergences": 0,
                "max_tree_depth_hits": 0,
                "constraints_valid": true,
                "identifiability_checks_passed": true,
            },
            "design": request.design,
            "posterior": {
                "alpha": summary(0.0, 0.1, -0.2, 0.2),
                "beta": summary(0.2, 0.05, beta_lower, beta_upper),
                "rate_ratio": summary(0.2_f64.exp(), 0.06, beta_lower.exp(), beta_upper.exp()),
                "dispersion": summary(3.0, 0.4, 2.2, 3.8),
                "patient_intercept_sd": summary(0.4, 0.1, 0.2, 0.6),
                "patient_slope_sd": summary(0.2, 0.05, 0.1, 0.3),
                "slide_sd": summary(0.3, 0.08, 0.15, 0.45),
            },
            "patients": patients,
            "slides": slides,
            "rois": rois,
            "prior_predictive": {
                "draws": 500,
                "patient_rates": prior_rates,
                "zero_fraction_mean": 0.3,
                "zero_fraction_sd": 0.1,
            },
            "posterior_predictive": {
                "observed_zero_fraction": 0.5,
                "replicated_zero_fraction_mean": 0.45,
                "replicated_zero_fraction_sd": 0.08,
                "probability_replicated_zero_fraction_at_least_observed": 0.3,
                "zero_fraction_tail_probability_mcse": null,
                "patients": predictive_patients,
            },
        })
    }

    #[test]
    fn request_sorts_and_compiles_stable_within_patient_design() {
        let request = request();
        assert_eq!(request.spec.observations[0].roi_id, "roi-00-0-0");
        assert_eq!(request.design.patient_ids[0], "patient-00");
        assert_eq!(request.design.slide_ids[0], "slide-00-0");
        assert_eq!(
            request.design.patient_predictor_means[0],
            1_000_000_000_001.5
        );
        assert!((request.design.within_predictor_sd - 1.25_f64.sqrt()).abs() < 1e-15);
        for patient in 0..8 {
            let sum = request.design.within_predictor[patient * 8..patient * 8 + 8]
                .iter()
                .sum::<f64>();
            assert!(sum.abs() < 1e-14);
        }
        assert_eq!(request.diagnostic_policy.maximum_tree_depth, 10);

        let mut untranslated = spec();
        for row in &mut untranslated.observations {
            row.predictor = if row.roi_id.ends_with("-3") { 1.0 } else { 0.0 };
        }
        let mut translated = untranslated.clone();
        for row in &mut translated.observations {
            row.predictor += 2_f64.powi(52);
        }
        let untranslated = NegativeBinomialHierarchyWorkerRequest::new(
            untranslated,
            "a".repeat(64),
            "b".repeat(64),
        )
        .expect("untranslated design");
        let translated =
            NegativeBinomialHierarchyWorkerRequest::new(translated, "a".repeat(64), "b".repeat(64))
                .expect("translated design");
        assert_eq!(
            translated.design.within_predictor_sd,
            untranslated.design.within_predictor_sd
        );
        assert_eq!(
            translated.design.within_predictor,
            untranslated.design.within_predictor
        );
    }

    #[test]
    fn request_rejects_parentage_support_and_resource_failures() {
        let mut cases = Vec::new();
        let mut parentage = spec();
        parentage.observations[0].slide_id = "slide-00-0".into();
        cases.push(parentage);
        let mut no_variation = spec();
        let slide = no_variation.observations[0].slide_id.clone();
        let value = no_variation.observations[0].predictor;
        for row in &mut no_variation.observations {
            if row.slide_id == slide {
                row.predictor = value;
            }
        }
        cases.push(no_variation);
        let mut products = spec();
        products.maximum_draw_observation_products = 44_799;
        cases.push(products);
        let mut iterations = spec();
        iterations.sampling.tune_per_chain = 50_000;
        iterations.sampling.draws_per_chain = 100;
        cases.push(iterations);

        for invalid in cases {
            assert!(NegativeBinomialHierarchyWorkerRequest::new(
                invalid,
                "a".repeat(64),
                "b".repeat(64),
            )
            .is_err());
        }
    }

    #[test]
    fn result_requires_complete_identities_and_unchanged_diagnostics() {
        let request = request();
        let valid: NegativeBinomialHierarchyResult =
            serde_json::from_value(result_value(&request)).expect("typed result");
        valid.validate(&request, "request").expect("valid result");

        let mut dropped = result_value(&request);
        dropped["rois"].as_array_mut().expect("ROI array").pop();
        let dropped: NegativeBinomialHierarchyResult =
            serde_json::from_value(dropped).expect("typed dropped result");
        assert!(dropped.validate(&request, "request").is_err());

        let mut mismatch = result_value(&request);
        mismatch["diagnostics"]["ess_tail"] = serde_json::json!(399.0);
        let mismatch: NegativeBinomialHierarchyResult =
            serde_json::from_value(mismatch).expect("typed mismatch result");
        assert!(mismatch.validate(&request, "request").is_err());

        let mut invalid_mcse = result_value(&request);
        invalid_mcse["posterior_predictive"]["zero_fraction_tail_probability_mcse"] =
            serde_json::json!(-0.01);
        let invalid_mcse: NegativeBinomialHierarchyResult =
            serde_json::from_value(invalid_mcse).expect("typed invalid MCSE result");
        assert!(invalid_mcse.validate(&request, "request").is_err());
    }
}
