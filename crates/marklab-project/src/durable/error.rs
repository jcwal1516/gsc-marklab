use std::io;

use thiserror::Error;

use crate::{ArtifactRecordError, ArtifactStoreError, ContentDigest, ProjectError};

/// Durable project validation, integrity, recovery, and I/O failures.
#[derive(Debug, Error)]
pub enum DurableProjectError {
    /// At least one limit is zero or the record limit exceeds the ledger limit.
    #[error(
        "durable project limits must be positive and record bytes may not exceed ledger bytes"
    )]
    InvalidLimits,
    /// Native runtime provenance is incomplete or malformed.
    #[error("invalid native runtime provenance: {reason}")]
    InvalidRuntime {
        /// Stable rejection reason.
        reason: String,
    },
    /// Execution request violates the bounded single-node contract.
    #[error("invalid durable execution request: {reason}")]
    InvalidRequest {
        /// Stable rejection reason.
        reason: String,
    },
    /// Project-root creation, inspection, or opening failed.
    #[error("failed to {operation} durable project root: {source}")]
    Root {
        /// Failed operation.
        operation: &'static str,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// A project control path crosses a symbolic link.
    #[error("durable project path crosses a symlink boundary at {path:?}")]
    SymlinkBoundary {
        /// Root-relative path or root marker.
        path: String,
    },
    /// A path expected to be a directory or regular file has another type.
    #[error("durable project path has an unsupported file type at {path:?}")]
    UnsupportedPathType {
        /// Root-relative path or root marker.
        path: String,
    },
    /// Capability-relative project I/O failed.
    #[error("failed to {operation} at durable project path {path:?}: {source}")]
    Io {
        /// Failed operation.
        operation: &'static str,
        /// Root-relative path.
        path: String,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// Canonical state exceeds a configured bound.
    #[error("durable state {path:?} has {observed} bytes, exceeding {maximum}")]
    StateTooLarge {
        /// Affected control path.
        path: String,
        /// Observed bytes.
        observed: usize,
        /// Configured maximum bytes.
        maximum: usize,
    },
    /// Ledger contains too many records.
    #[error("execution ledger has {observed} records, exceeding {maximum}")]
    TooManyLedgerRecords {
        /// Observed records.
        observed: usize,
        /// Configured maximum records.
        maximum: usize,
    },
    /// One output exceeds the project object materialization limit.
    #[error("durable output has {observed} bytes, exceeding {maximum}")]
    ObjectTooLarge {
        /// Observed bytes.
        observed: usize,
        /// Configured maximum bytes.
        maximum: usize,
    },
    /// Strict JSON decoding or encoding failed.
    #[error("invalid JSON for durable state {path:?}: {reason}")]
    Json {
        /// Affected path or encoding context.
        path: String,
        /// Parser/encoder reason.
        reason: String,
    },
    /// State decoded but was not the required canonical fixed point.
    #[error("durable state {path:?} is not canonically encoded")]
    NonCanonicalState {
        /// Affected control path.
        path: String,
    },
    /// Format identifier is unsupported.
    #[error("unsupported durable format {observed:?} in {path:?}")]
    UnsupportedFormat {
        /// Affected control path.
        path: String,
        /// Observed format identifier.
        observed: String,
    },
    /// Format version is unsupported.
    #[error("unsupported durable version {observed} in {path:?}")]
    UnsupportedVersion {
        /// Affected control path.
        path: String,
        /// Observed numeric version.
        observed: u32,
    },
    /// Project head contains an invalid fixed policy or identifier.
    #[error("durable project head contains an invalid identity or fixed policy")]
    InvalidHead,
    /// Only one of the required initial head/ledger pair exists.
    #[error("durable project has an incomplete project-head/execution-ledger pair")]
    IncompleteProjectState,
    /// Ledger lacks its required final newline.
    #[error("execution ledger is truncated before a final newline")]
    TruncatedLedger,
    /// Ledger contains an invalid required value.
    #[error("malformed execution ledger: {reason}")]
    MalformedLedger {
        /// Stable rejection reason.
        reason: String,
    },
    /// Sequence, prior digest, or monotonic time does not form one append-only chain.
    #[error("invalid execution-ledger chain at sequence {sequence}")]
    InvalidLedgerChain {
        /// Affected sequence.
        sequence: u64,
    },
    /// Ledger or byte-length arithmetic overflowed.
    #[error("execution-ledger sequence or byte length overflowed")]
    LedgerSequenceOverflow,
    /// One cache key appears more than once in the append-only ledger.
    #[error("execution ledger repeats cache key {cache_key}")]
    DuplicateLedgerCacheKey {
        /// Repeated key.
        cache_key: ContentDigest,
    },
    /// Head summary does not exactly describe the validated ledger.
    #[error("durable project head does not match its execution ledger")]
    HeadLedgerMismatch,
    /// Pending intent cannot extend or equal the validated ledger.
    #[error("pending durable execution conflicts with the execution ledger")]
    ConflictingPendingExecution,
    /// A matching cache key names different execution identity fields.
    #[error("durable cache key {cache_key} conflicts with recorded execution identity")]
    ConflictingCacheIdentity {
        /// Conflicting key.
        cache_key: ContentDigest,
    },
    /// Reconstructed schema-bound artifact identity differs from the ledger claim.
    #[error("durable execution contains an invalid output artifact identity")]
    InvalidArtifactIdentity,
    /// Restored bytes do not produce the exact recorded output reference.
    #[error("restored durable output differs for cache key {cache_key}")]
    RestoredOutputMismatch {
        /// Affected key.
        cache_key: ContentDigest,
    },
    /// System clock predates the Unix epoch.
    #[error("system clock is before the Unix epoch")]
    ClockBeforeEpoch,
    /// System time cannot be represented in milliseconds.
    #[error("system clock milliseconds exceed u64")]
    ClockOverflow,
    /// Test-only crash point interrupted the durable commit.
    #[error("injected durable project failure {stage}")]
    InjectedFailure {
        /// Interrupted lifecycle stage.
        stage: &'static str,
    },
    /// Immutable artifact declaration is invalid.
    #[error(transparent)]
    ArtifactRecord(#[from] ArtifactRecordError),
    /// Existing local object-store operation failed.
    #[error(transparent)]
    ArtifactStore(#[from] ArtifactStoreError),
    /// In-memory project identity or cache commit failed.
    #[error(transparent)]
    Project(#[from] ProjectError),
    /// Digest text is malformed.
    #[error("invalid SHA-256 digest {value:?}")]
    InvalidDigest {
        /// Rejected text.
        value: String,
    },
}
