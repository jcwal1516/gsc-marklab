//! Strict wire contracts for the existing causal and active-design worker.

use marklab_core::backend::is_worker_timeout;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::{ValidationStatus, ValidationSuiteStatus};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Number;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Backend {
    pub name: String,
    pub scipy_version: String,
    pub numpy_version: String,
    pub python_version: String,
    pub license: String,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

// A present optional field must have its declared type. Missing fields stay
// omitted, so forwarding does not silently turn absence into JSON null.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationalSpec {
    pub rows: Vec<ObservationalRow>,
    pub cluster_folds: Number,
    pub dose_basis_degree: Number,
    pub propensity_clip: Number,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub seed: Option<Number>,
    pub timeout_seconds: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationalRow {
    pub unit_id: String,
    pub cluster_id: String,
    pub baseline_covariates: Vec<Number>,
    pub treatment: Number,
    pub dose: Number,
    pub neighbor_exposure: Number,
    pub outcome: Number,
    pub negative_control_outcome: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerturbationSpec {
    pub rows: Vec<PerturbationRow>,
    pub assignment: String,
    pub temporal_order: Vec<String>,
    pub mediator_outcome_no_unmeasured_confounding_declared: bool,
    pub sensitivity_shifts: Vec<Number>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub seed: Option<Number>,
    pub timeout_seconds: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerturbationRow {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub unit_id: Option<String>,
    pub cluster_id: String,
    pub treatment: Number,
    pub neighbor_exposure: Number,
    pub mediator: Number,
    pub outcome: Number,
    pub negative_control_outcome: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveDesignSpec {
    pub posterior: PosteriorSpec,
    pub candidates: Vec<Candidate>,
    pub budgets: Budgets,
    pub allocation_options: Vec<AllocationOption>,
    pub power_designs: Vec<PowerDesign>,
    pub power_replicates: Number,
    pub alpha: Number,
    pub seed: Number,
    pub timeout_seconds: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PosteriorSpec {
    pub mean: Number,
    pub variance: Number,
    pub observation_variance: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub kind: String,
    pub cost: Number,
    pub information: Number,
    pub quality: Number,
    pub redundancy: Number,
    pub robustness: Number,
    pub coverage: Number,
    pub observation: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    pub sequential: Number,
    pub roi: Number,
    pub stain: Number,
    pub landmark: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationOption {
    pub id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub cost: Option<Number>,
    pub biological_replicates: Number,
    pub technical_replicates: Number,
    pub effect_standard_error: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PowerDesign {
    pub id: String,
    pub clusters: Number,
    pub effect: Number,
    pub standard_deviation: Number,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ModelSpec {
    Observational(ObservationalSpec),
    Perturbation(PerturbationSpec),
    ActiveDesign(ActiveDesignSpec),
}

impl ModelSpec {
    pub fn timeout_seconds(&self) -> Result<u64, &'static str> {
        let value = match self {
            Self::Observational(spec) => &spec.timeout_seconds,
            Self::Perturbation(spec) => &spec.timeout_seconds,
            Self::ActiveDesign(spec) => &spec.timeout_seconds,
        };
        let timeout = value
            .as_u64()
            .ok_or("causal model input requires integer timeout_seconds")?;
        if !is_worker_timeout(timeout) {
            return Err("causal model timeout must be between 1 and 3600 seconds");
        }
        Ok(timeout)
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Self::Observational(_) => "observational",
            Self::Perturbation(_) => "perturbation",
            Self::ActiveDesign(_) => "active_design",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRequest {
    pub format: &'static str,
    pub version: u32,
    pub mode: &'static str,
    pub backend: Backend,
    pub spec: ModelSpec,
    pub input_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRequest {
    pub format: &'static str,
    pub version: u32,
    pub mode: &'static str,
    pub backend: Backend,
    pub seed: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationalResult {
    pub format: String,
    pub version: u32,
    pub backend: Backend,
    pub request_sha256: String,
    pub propensity: Propensity,
    pub dose_response: DoseResponse,
    pub cross_fitted_aipw: EffectEstimate,
    pub adjusted_exposure_regression: ExposureRegression,
    pub spatial_dml: SpatialDml,
    pub negative_control: ObservationalNegativeControl,
    pub cross_fitting: CrossFitting,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Propensity {
    pub coefficients: Vec<f64>,
    pub minimum: f64,
    pub maximum: f64,
    pub positivity_passed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DoseResponse {
    pub basis_degree: usize,
    pub linear_effect: f64,
    pub standard_error: f64,
    pub polynomial_terms: Vec<PolynomialTerm>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolynomialTerm {
    pub power: usize,
    pub coefficient: f64,
    pub standard_error: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectEstimate {
    pub effect: f64,
    pub standard_error: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExposureRegression {
    pub mapping: String,
    pub effect: f64,
    pub standard_error: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialDml {
    pub dose_effect: f64,
    pub dose_standard_error: f64,
    pub spillover_effect: f64,
    pub spillover_standard_error: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationalNegativeControl {
    pub adjusted_treatment_effect: f64,
    pub standard_error: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CrossFitting {
    pub folds: Vec<CrossFittingFold>,
    pub cluster_leakage: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CrossFittingFold {
    pub fold: usize,
    pub test_clusters: Vec<String>,
    pub train_cluster_count: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerturbationResult {
    pub format: String,
    pub version: u32,
    pub backend: Backend,
    pub request_sha256: String,
    pub design: PerturbationDesign,
    pub primary: PrimaryEffects,
    pub mediation: Mediation,
    pub negative_control: PerturbationNegativeControl,
    pub replication: Replication,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerturbationDesign {
    pub assignment: String,
    pub temporal_order_validated: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrimaryEffects {
    pub direct_effect: f64,
    pub direct_standard_error: f64,
    pub spillover_effect: f64,
    pub spillover_standard_error: f64,
    pub multiplicity_policy: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mediation {
    pub treatment_to_mediator: f64,
    pub treatment_to_mediator_standard_error: f64,
    pub mediator_to_outcome: f64,
    pub indirect_effect: f64,
    pub sensitivity_indirect_effects: Vec<f64>,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerturbationNegativeControl {
    pub treatment_effect: f64,
    pub standard_error: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Replication {
    pub unit: String,
    pub independent_cluster_count: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveDesignResult {
    pub format: String,
    pub version: u32,
    pub backend: Backend,
    pub request_sha256: String,
    pub sequential: SequentialDesign,
    pub roi_selection: Selection,
    pub stain_selection: Selection,
    pub landmark_selection: Selection,
    pub replicate_allocation: ReplicateAllocation,
    pub power: Vec<PowerEstimate>,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SequentialDesign {
    pub acquisition_sequence: Vec<String>,
    pub posterior_history: Vec<PosteriorHistory>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PosteriorHistory {
    Initial(InitialPosterior),
    Acquisition(AcquisitionPosterior),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InitialPosterior {
    pub step: usize,
    pub mean: f64,
    pub variance: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionPosterior {
    pub step: usize,
    pub candidate_id: String,
    pub mean: f64,
    pub variance: f64,
    pub remaining_budget: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub selected_ids: Vec<String>,
    pub spent: f64,
    pub expected_net_utility: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicateAllocation {
    pub recommended_id: String,
    pub sensitivity_frontier: Vec<AllocationFrontier>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationFrontier {
    pub id: String,
    pub biological_replicates: Number,
    pub technical_replicates: Number,
    pub effect_standard_error: Number,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PowerEstimate {
    pub design_id: String,
    pub estimated_power: f64,
    pub failure_rate: f64,
    pub wilson_interval: [f64; 2],
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationResult {
    pub format: String,
    pub version: u32,
    pub backend: Backend,
    pub request_sha256: String,
    pub entries: Vec<ValidationEntry>,
    pub overall_status: ValidationSuiteStatus,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationEntry {
    pub validation_id: String,
    pub status: ValidationStatus,
    #[serde(deserialize_with = "Option::deserialize")]
    pub value: Option<Number>,
    pub threshold: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ModelResult {
    Observational(Box<ObservationalResult>),
    Perturbation(Box<PerturbationResult>),
    ActiveDesign(Box<ActiveDesignResult>),
    Validation(Box<ValidationResult>),
}

impl ModelResult {
    pub fn decode(mode: &str, response: &[u8]) -> Result<Self, serde_json::Error> {
        match mode {
            "observational" => serde_json::from_slice(response).map(Self::Observational),
            "perturbation" => serde_json::from_slice(response).map(Self::Perturbation),
            "active_design" => serde_json::from_slice(response).map(Self::ActiveDesign),
            _ => serde_json::from_slice(response).map(Self::Validation),
        }
    }

    pub fn validate_identity(
        &self,
        backend: &Backend,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        let (format, version, reported_backend, reported_sha256, expected, expected_version) =
            match self {
                Self::Observational(result) => (
                    &result.format,
                    result.version,
                    &result.backend,
                    &result.request_sha256,
                    "marklab.synthetic_observational_causal_estimators",
                    2,
                ),
                Self::Perturbation(result) => (
                    &result.format,
                    result.version,
                    &result.backend,
                    &result.request_sha256,
                    "marklab.synthetic_spatial_perturbation_analysis",
                    1,
                ),
                Self::ActiveDesign(result) => (
                    &result.format,
                    result.version,
                    &result.backend,
                    &result.request_sha256,
                    "marklab.synthetic_active_design",
                    1,
                ),
                Self::Validation(result) => (
                    &result.format,
                    result.version,
                    &result.backend,
                    &result.request_sha256,
                    "marklab.causal_active_validation_suite",
                    1,
                ),
            };
        if format != expected
            || version != expected_version
            || reported_backend != backend
            || reported_sha256 != request_sha256
        {
            return Err("causal/active result identity mismatch");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn causal_specs_preserve_forwarded_numbers_and_reject_unknown_fields() {
        let input = br#"{
            "rows":[{"unit_id":"unit","cluster_id":"cluster","baseline_covariates":[1,1.0],
                "treatment":1,"dose":1.0,"neighbor_exposure":0,"outcome":2.0,
                "negative_control_outcome":0.0}],
            "cluster_folds":4,"dose_basis_degree":2,"propensity_clip":0.02,
            "timeout_seconds":60
        }"#;
        let mut legacy: serde_json::Value = serde_json::from_slice(input).unwrap();
        for seed in [None, Some(serde_json::json!(211))] {
            if let Some(seed) = seed {
                legacy["seed"] = seed;
            }
            let original = serde_json::to_vec(&legacy).unwrap();
            let typed: ObservationalSpec = serde_json::from_slice(&original).unwrap();
            assert_eq!(
                serde_json::to_vec(&serde_json::to_value(&typed).unwrap()).unwrap(),
                original
            );
            assert_eq!(
                ModelSpec::Observational(typed).timeout_seconds().unwrap(),
                60
            );
        }
        legacy["rows"][0]["unrecognized_measurement"] = serde_json::json!(1);
        assert!(serde_json::from_value::<ObservationalSpec>(legacy).is_err());

        let perturbation = serde_json::json!({
            "rows":[{"unit_id":"unit","cluster_id":"cluster","treatment":1.0,
                "neighbor_exposure":0,"mediator":2.0,"outcome":3,"negative_control_outcome":0}],
            "assignment":"cluster_randomized","temporal_order":["treatment","mediator","outcome"],
            "mediator_outcome_no_unmeasured_confounding_declared":true,
            "sensitivity_shifts":[0,0.0],"seed":223,"timeout_seconds":60
        });
        let typed: PerturbationSpec = serde_json::from_value(perturbation.clone()).unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), perturbation);
    }

    #[test]
    fn causal_results_require_the_complete_typed_worker_shape() {
        let mut response = serde_json::json!({
            "format":"marklab.causal_active_validation_suite","version":1,
            "backend":{"name":"worker","scipy_version":"scipy","numpy_version":"numpy",
                "python_version":"python","license":"BSD-3-Clause",
                "environment_lock_sha256":"lock","worker_sha256":"worker"},
            "request_sha256":"request",
            "entries":[{"validation_id":"unexecuted","status":"not_verified_unexecuted_control",
                "value":null,"threshold":"sum=1"}],
            "overall_status":"partial_synthetic_and_prospective_evidence_required",
            "claim_status":"arithmetic_smoke_only_missing_production_calibration_and_prospective_evidence"
        });
        let bytes = serde_json::to_vec(&response).unwrap();
        let result = ModelResult::decode("validation", &bytes).unwrap();
        assert_eq!(serde_json::to_value(result).unwrap(), response);

        response["entries"][0]["value"] = serde_json::json!([1.0, 2.0]);
        assert!(
            ModelResult::decode("validation", &serde_json::to_vec(&response).unwrap()).is_err()
        );
        response["entries"][0]["value"] = serde_json::Value::Null;
        response["entries"][0]["unrecognized_measurement"] = serde_json::json!(1);
        assert!(
            ModelResult::decode("validation", &serde_json::to_vec(&response).unwrap()).is_err()
        );
        response["entries"][0]
            .as_object_mut()
            .unwrap()
            .remove("unrecognized_measurement");
        response["entries"][0]
            .as_object_mut()
            .unwrap()
            .remove("value");
        assert!(
            ModelResult::decode("validation", &serde_json::to_vec(&response).unwrap()).is_err()
        );
    }
}
