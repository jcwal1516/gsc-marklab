use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicatedLgcpPattern {
    pub pattern_id: String,
    pub patient_id: String,
    pub window_sha256: String,
    pub grid_sha256: String,
    pub covariate_sha256: String,
    pub event_count: u64,
    pub cell_count: u32,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub enum ReplicatedFieldPolicy {
    IndependentReplicateFields,
    SharedPlusReplicate,
}
impl ReplicatedFieldPolicy {
    pub fn parse(v: &str) -> Result<Self, ReplicatedError> {
        match v {
            "independent-replicate-fields" => Ok(Self::IndependentReplicateFields),
            "shared-plus-replicate" => Ok(Self::SharedPlusReplicate),
            _ => Err(ReplicatedError::Invalid("field policy is invalid".into())),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::IndependentReplicateFields => "independent_replicate_fields",
            Self::SharedPlusReplicate => "shared_plus_replicate",
        }
    }
}
#[derive(Clone, Debug)]
pub struct ReplicatedLgcpSpec {
    pub patterns: Vec<ReplicatedLgcpPattern>,
    pub policy: ReplicatedFieldPolicy,
    pub global_prior_sd: f64,
    pub patient_intercept_prior_sd: f64,
    pub population_field_amplitude: f64,
    pub population_field_length_scale_um: f64,
    pub replicate_field_amplitude: f64,
    pub jitter: f64,
}
#[derive(Debug, Error)]
pub enum ReplicatedError {
    #[error("invalid replicated point-process model: {0}")]
    Invalid(String),
}
#[derive(Debug, Serialize)]
pub struct ReplicatedLgcpModelIr {
    pub family: &'static str,
    pub hierarchy: &'static str,
    pub biological_unit: &'static str,
    pub pattern_unit: &'static str,
    pub global_fixed_effects: &'static str,
    pub patient_effects: &'static str,
    pub field_policy: &'static str,
    pub population_kernel: &'static str,
    pub population_field_amplitude: f64,
    pub population_field_length_scale_um: f64,
    pub replicate_field_amplitude: f64,
    pub jitter: f64,
    pub global_prior_sd: f64,
    pub patient_intercept_prior_sd: f64,
    pub likelihood: &'static str,
    pub pattern_combination: &'static str,
    pub generated_quantities: [&'static str; 3],
    pub maturity: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ReplicatedLgcpModel {
    pub model: ReplicatedLgcpModelIr,
    pub patient_count: u32,
    pub pattern_count: u32,
    pub total_event_count: u64,
    pub patterns_per_patient: BTreeMap<String, u32>,
    pub patterns: Vec<ReplicatedLgcpPattern>,
}
pub fn replicated_hierarchical_lgcp(
    mut s: ReplicatedLgcpSpec,
) -> Result<ReplicatedLgcpModel, ReplicatedError> {
    if !(6..=1000).contains(&s.patterns.len())
        || [
            s.global_prior_sd,
            s.patient_intercept_prior_sd,
            s.population_field_amplitude,
            s.population_field_length_scale_um,
            s.replicate_field_amplitude,
            s.jitter,
        ]
        .into_iter()
        .any(|v| !v.is_finite() || v <= 0.0)
    {
        return Err(ReplicatedError::Invalid(
            "pattern dimensions or prior/field scales are invalid".into(),
        ));
    }
    s.patterns.sort_by(|a, b| a.pattern_id.cmp(&b.pattern_id));
    let mut ids = HashSet::new();
    let mut per = BTreeMap::<String, u32>::new();
    let mut total = 0_u64;
    for p in &s.patterns {
        if p.pattern_id.is_empty()
            || p.patient_id.is_empty()
            || !ids.insert(p.pattern_id.as_str())
            || [&p.window_sha256, &p.grid_sha256, &p.covariate_sha256]
                .into_iter()
                .any(|v| !is_sha(v))
            || p.event_count == 0
            || !(4..=36).contains(&p.cell_count)
        {
            return Err(ReplicatedError::Invalid(
                "pattern identities, digests, counts, or cells are invalid".into(),
            ));
        }
        *per.entry(p.patient_id.clone()).or_default() += 1;
        total = total
            .checked_add(p.event_count)
            .ok_or_else(|| ReplicatedError::Invalid("event count overflows".into()))?;
    }
    if per.len() < 3 || per.values().any(|n| *n < 2) {
        return Err(ReplicatedError::Invalid(
            "at least three patients and two patterns each are required".into(),
        ));
    }
    Ok(ReplicatedLgcpModel {
        model: ReplicatedLgcpModelIr {
            family: "replicated_hierarchical_log_gaussian_cox_process",
            hierarchy: "pattern_nested_in_patient",
            biological_unit: "patient",
            pattern_unit: "replicate_pattern",
            global_fixed_effects: "shared_population_coefficients",
            patient_effects: "gaussian_random_intercepts",
            field_policy: s.policy.name(),
            population_kernel: "matern_3_2_euclidean_2d_shared_hyperparameters",
            population_field_amplitude: s.population_field_amplitude,
            population_field_length_scale_um: s.population_field_length_scale_um,
            replicate_field_amplitude: s.replicate_field_amplitude,
            jitter: s.jitter,
            global_prior_sd: s.global_prior_sd,
            patient_intercept_prior_sd: s.patient_intercept_prior_sd,
            likelihood: "separate_exact_grid_cox_likelihood_per_pattern",
            pattern_combination: "never_concatenate_preserve_each_window",
            generated_quantities: [
                "population_intensity",
                "patient_intensity",
                "replicate_intensity",
            ],
            maturity: "experimental_model_construction",
        },
        patient_count: per.len() as u32,
        pattern_count: s.patterns.len() as u32,
        total_event_count: total,
        patterns_per_patient: per,
        patterns: s.patterns,
    })
}
fn is_sha(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
