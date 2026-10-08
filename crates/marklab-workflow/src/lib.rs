#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Minimal typed DAG and single-node local scheduler for Marklab.

pub use marklab_project::{
    ArtifactCatalog, ArtifactCatalogError, ArtifactDraft, ArtifactId, ArtifactKey, ArtifactLocator,
    ArtifactPublication, ArtifactRecord, ArtifactRecordError, ArtifactRef, ArtifactSchema,
    ArtifactStoreError, ContentDigest, ContentDigestParseError, ContentDigestWriter,
    DurableCommitDisposition, DurableExecutionRequest, DurableOpenReport, DurableProject,
    DurableProjectError, DurableProjectLimits, DurableRecoveryAction, DurableReplay,
    LocalArtifactStore, MarklabProject, NativeRuntimeProvenance, ProjectError,
    PublicationDisposition, RecoveryIssue, RecoveryIssueReason, RecoveryReport, StoreId,
    SuccessfulRun, TableColumn, TableColumnType, TableFormat, TableManifest, TableManifestError,
    TableScalarType, VerifiedReaderError,
};

mod error;
mod execution;
mod graph;
mod node;
mod scheduler;

pub use error::WorkflowError;
pub use execution::{execute_algorithm, execute_algorithm_with_store, ExecuteAlgorithmError};
pub use graph::{NodeId, NodeSpec, WorkflowGraph};
pub use node::{CacheKeyMaterial, NodeError, WorkflowNode};
pub use scheduler::{CacheStatus, LocalScheduler, NodeRun, SchedulerLimits};

#[cfg(test)]
mod tests;
