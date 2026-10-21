//! Typed messages and scientific checks for arbitrary window lgcp sbc.

use super::arbitrary_window_lgcp_fit;
use super::arbitrary_window_lgcp_fit::InputIdentity;
use crate::backend_versions::JAX_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::ClaimStatus;
use crate::FitState;
use crate::GriddedLgcpSbcCalibrationPolicy;
use crate::GriddedLgcpSbcDiagnostics;
use crate::GriddedLgcpSbcFailure;
use crate::GriddedLgcpSbcReplicate;
use crate::GriddedLgcpSbcResourceLimits;
use crate::NumpyroGriddedLgcpSbcWorkerRequest;
use crate::NumpyroGriddedLgcpSbcWorkerResult;
use crate::NutsSamplingSpec;
use crate::WorkerBackend;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize)]
pub struct WorkerRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source_backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: &'a arbitrary_window_lgcp_fit::WorkerRequest,
    pub source_sbc_request_sha256: &'a str,
    pub source_sbc_request: &'a NumpyroGriddedLgcpSbcWorkerRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input: InputIdentity,
    pub covariance_sha256: String,
    pub physical_latent_node_id: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub source_sbc_request_sha256: String,
    pub source_result: NumpyroGriddedLgcpSbcWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct SbcResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    pub jax_version: String,
    pub input: InputIdentity,
    pub covariance_sha256: String,
    pub physical_latent_node_id: String,
    pub sampling: NutsSamplingSpec,
    pub calibration: GriddedLgcpSbcCalibrationPolicy,
    pub resources: GriddedLgcpSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<GriddedLgcpSbcReplicate>,
    pub failures: Vec<GriddedLgcpSbcFailure>,
    pub diagnostics: GriddedLgcpSbcDiagnostics,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub source_sbc_request_sha256: String,
    pub statistical_unit: StatisticalUnit,
    pub claim_status: ClaimStatus,
}

pub fn validate_outer(
    result: &WorkerResult,
    request: &WorkerRequest<'_>,
    request_sha256: &str,
    source_sbc_request_sha256: &str,
) -> Result<(), BayesError> {
    let latent_index = request.source_sbc_request.calibration.latent_cell_index as usize;
    if result.format != "marklab.numpyro_arbitrary_window_lgcp_sbc_result"
        || result.version != 1
        || !result.backend.matches(&request.backend)
        || result.jax_version != JAX_VERSION
        || result.input != request.source_request.input
        || result.covariance_sha256 != request.source_request.covariance_sha256
        || result.physical_latent_node_id != request.source_request.nodes[latent_index].node_id
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != request.source_request_sha256
        || result.source_sbc_request_sha256 != source_sbc_request_sha256
    {
        return Err(BayesError::WorkerContract(
            "NumPyro arbitrary-window LGCP SBC outer identity differs".into(),
        ));
    }
    Ok(())
}
