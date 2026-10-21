//! Typed messages and scientific checks for replicated arbitrary window multitype lgcp prior calibration.

use crate::backend_versions::NUMPY_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Calibration {
    pub replicates: u32,
    pub maximum_mean_error_sd: f64,
    pub maximum_relative_sd_error: f64,
    pub maximum_field_covariance_rmse_scale: f64,
    pub maximum_poisson_residual_mean: f64,
    pub maximum_poisson_residual_second_moment_error: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resources {
    pub node_type_rows: usize,
    pub simulation_node_type_work: u64,
    pub maximum_simulation_node_type_work: u64,
    pub estimated_working_bytes: u64,
    pub maximum_working_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
pub struct WorkerRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub numpy_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: serde_json::Value,
    pub calibration: Calibration,
    pub resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MomentCheck {
    pub component: String,
    pub expected_mean: f64,
    pub observed_mean: f64,
    pub mean_error_sd: f64,
    pub expected_sd: f64,
    pub observed_sd: f64,
    pub relative_sd_error: f64,
    pub passes: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldOracle {
    pub maximum_centering_error: f64,
    pub covariance_rmse_scale: f64,
    pub passes: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PoissonOracle {
    pub standardized_residual_count: u64,
    pub standardized_residual_mean: f64,
    pub standardized_residual_second_moment: f64,
    pub passes: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub numpy_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub moment_checks: Vec<MomentCheck>,
    pub field_oracle: FieldOracle,
    pub poisson_oracle: PoissonOracle,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub numpy_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub calibration: Calibration,
    pub resources: Resources,
    pub moment_checks: Vec<MomentCheck>,
    pub field_oracle: FieldOracle,
    pub poisson_oracle: PoissonOracle,
    pub statistical_unit: StatisticalUnit,
    pub generative_model: &'static str,
    pub analytic_oracles: [&'static str; 3],
    pub assumptions: [&'static str; 4],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

pub fn validate_result(
    result: &WorkerResult,
    backend: &BackendContract,
    request_sha: &str,
    source_sha: &str,
    input_sha: &str,
    calibration: &Calibration,
    simulation_work: u64,
) -> Result<(), BayesError> {
    let names = [
        "intercept",
        "group_effect",
        "covariate_effect",
        "patient_sd",
        "pattern_sd",
    ];
    let moments_valid = result.moment_checks.len() == names.len()
        && result.moment_checks.iter().zip(names).all(|(row, name)| {
            row.component == name
                && [
                    row.expected_mean,
                    row.observed_mean,
                    row.mean_error_sd,
                    row.expected_sd,
                    row.observed_sd,
                    row.relative_sd_error,
                ]
                .into_iter()
                .all(f64::is_finite)
                && row.expected_sd > 0.0
                && row.observed_sd >= 0.0
                && row.passes
                    == (row.mean_error_sd <= calibration.maximum_mean_error_sd
                        && row.relative_sd_error <= calibration.maximum_relative_sd_error)
        });
    let field_valid = [
        result.field_oracle.maximum_centering_error,
        result.field_oracle.covariance_rmse_scale,
    ]
    .into_iter()
    .all(f64::is_finite)
        && result.field_oracle.passes
            == (result.field_oracle.maximum_centering_error <= 1e-12
                && result.field_oracle.covariance_rmse_scale
                    <= calibration.maximum_field_covariance_rmse_scale);
    let poisson_valid = [
        result.poisson_oracle.standardized_residual_mean,
        result.poisson_oracle.standardized_residual_second_moment,
    ]
    .into_iter()
    .all(f64::is_finite)
        && result.poisson_oracle.standardized_residual_count == simulation_work
        && result.poisson_oracle.passes
            == (result.poisson_oracle.standardized_residual_mean.abs()
                <= calibration.maximum_poisson_residual_mean
                && (result.poisson_oracle.standardized_residual_second_moment - 1.0).abs()
                    <= calibration.maximum_poisson_residual_second_moment_error);
    let passes = moments_valid
        && result.moment_checks.iter().all(|row| row.passes)
        && field_valid
        && result.field_oracle.passes
        && poisson_valid
        && result.poisson_oracle.passes;
    if result.format
        != "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.numpy_version != NUMPY_VERSION
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha
        || result.input_sha256 != input_sha
        || !moments_valid
        || !field_valid
        || !poisson_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesError::WorkerContract(
            "replicated multitype LGCP prior-calibration result differs".into(),
        ));
    }
    Ok(())
}
