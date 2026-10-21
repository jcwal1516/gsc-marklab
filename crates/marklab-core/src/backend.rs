//! Shared scientific worker identities, result vocabulary, and boundary errors.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod backend_capability;
mod claim_status;
mod model_maturity;
pub use backend_capability::BackendCapability;
pub use claim_status::ClaimStatus;
pub use model_maturity::ModelMaturity;
pub mod versions;

#[derive(Clone, Debug, Serialize)]
/// Backend implementation and exact worker assets required by a request.
pub struct BackendContract {
    /// Backend package or combined implementation name.
    pub name: &'static str,
    /// Pinned backend package release.
    pub version: &'static str,
    /// Pinned Python language release.
    pub python_version: &'static str,
    /// SHA-256 of the worker environment lock bytes.
    pub environment_lock_sha256: String,
    /// SHA-256 of the bound worker implementation bytes.
    pub worker_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Completion state reported by a scientific worker.
pub enum FitState {
    /// The fit satisfies its declared completion and diagnostic policy.
    Complete,
    /// The fit did not satisfy its convergence policy.
    Nonconverged,
    /// Only an approximate fit is available.
    ApproximateOnly,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Backend implementation and exact assets reported by a worker.
pub struct WorkerBackend {
    /// Backend package or combined implementation name.
    pub name: String,
    /// Pinned backend package release.
    pub version: String,
    /// Pinned Python language release.
    pub python_version: String,
    /// SHA-256 of the worker environment lock bytes.
    pub environment_lock_sha256: String,
    /// SHA-256 of the bound worker implementation bytes.
    pub worker_sha256: String,
}

impl WorkerBackend {
    /// Whether a worker reported exactly the backend, Python release, environment lock and
    /// worker script that the request bound.
    pub fn matches(&self, contract: &BackendContract) -> bool {
        self.name == contract.name
            && self.version == contract.version
            && self.python_version == contract.python_version
            && self.environment_lock_sha256 == contract.environment_lock_sha256
            && self.worker_sha256 == contract.worker_sha256
    }
}

#[derive(Debug, Error)]
/// Scientific specification, numerical, or worker boundary failure.
///
/// Diagnostic prefixes retain their historical spelling for CLI compatibility.
pub enum WorkerError {
    #[error("invalid Bayesian specification: {0}")]
    /// The supplied scientific specification is invalid.
    InvalidSpec(String),
    #[error("Bayesian numerical calculation failed: {0}")]
    /// The numerical calculation failed.
    Numerical(String),
    #[error("Bayesian worker contract violation: {0}")]
    /// The worker result violates its request contract.
    WorkerContract(String),
    #[error("Bayesian JSON boundary failed: {0}")]
    /// The JSON boundary could not be encoded or decoded.
    Json(#[from] serde_json::Error),
}

/// Largest admitted wall-clock timeout for a single worker process.
pub const MAXIMUM_WORKER_TIMEOUT_SECONDS: u64 = 3_600;

/// Whether `seconds` is an allowed worker timeout: at least one second and at most
/// [`MAXIMUM_WORKER_TIMEOUT_SECONDS`].
pub fn is_worker_timeout(seconds: u64) -> bool {
    (1..=MAXIMUM_WORKER_TIMEOUT_SECONDS).contains(&seconds)
}

/// SHA-256 of the exact bytes bound into a worker or artifact identity.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// SHA-256 of a value's JSON encoding, which is what a worker request binds its input data to.
pub fn json_sha256<T: Serialize + ?Sized>(value: &T) -> Result<String, WorkerError> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

/// Whether a digest is exactly 64 lowercase ASCII hexadecimal characters.
pub fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
