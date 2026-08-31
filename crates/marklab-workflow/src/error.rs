use thiserror::Error;

use crate::{ArtifactId, ArtifactRef, ArtifactStoreError, NodeError, NodeId, ProjectError};

/// Graph, scheduling, cache, resource, and project-commit failures.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Node ID violates the shared restricted syntax.
    #[error("invalid workflow node ID {value:?}")]
    InvalidNodeId {
        /// Rejected ID text.
        value: String,
    },
    /// Node kind/version or another specification field is invalid.
    #[error("invalid specification for node {node_id}: {reason}")]
    InvalidNodeSpec {
        /// Affected node ID.
        node_id: String,
        /// Validation reason.
        reason: String,
    },
    /// One direct dependency is declared more than once.
    #[error("node {node_id} declares the same dependency more than once")]
    DuplicateDependency {
        /// Affected node ID.
        node_id: String,
    },
    /// Graph contains two specifications with the same ID.
    #[error("workflow contains duplicate node {node_id:?}")]
    DuplicateNode {
        /// Duplicated ID.
        node_id: NodeId,
    },
    /// A dependency edge names a node absent from the graph.
    #[error("node {node_id:?} depends on missing node {dependency:?}")]
    MissingDependency {
        /// Dependent node.
        node_id: NodeId,
        /// Missing dependency.
        dependency: NodeId,
    },
    /// Node declares itself as a direct dependency.
    #[error("node {node_id:?} depends on itself")]
    SelfDependency {
        /// Self-dependent node.
        node_id: NodeId,
    },
    /// Graph contains at least one dependency cycle.
    #[error("workflow contains a cycle involving {nodes:?}")]
    Cycle {
        /// Deterministically ordered nodes remaining after topological sorting.
        nodes: Vec<NodeId>,
    },
    /// Scheduler was asked to run a node absent from the validated graph.
    #[error("node {node_id:?} is not registered in the workflow graph")]
    NodeNotInGraph {
        /// Missing node ID.
        node_id: NodeId,
    },
    /// Runtime node specification differs from the registered specification.
    #[error("node {node_id:?} does not match its registered specification")]
    NodeSpecMismatch {
        /// Mismatched node ID.
        node_id: NodeId,
    },
    /// Reserved compatibility error from the former dependency-free scheduler.
    #[error("local scheduler cannot execute dependent node {node_id:?}")]
    DependenciesUnsupported {
        /// Node requiring multi-node scheduling.
        node_id: NodeId,
    },
    /// A declared dependency has not produced any exact input artifact consumed by this node.
    #[error("node {node_id:?} is missing the successful output of dependency {dependency:?}")]
    MissingDependencyOutput {
        /// Dependent node.
        node_id: NodeId,
        /// Upstream node whose exact output is absent.
        dependency: NodeId,
    },
    /// More than one declared input claims to be the selected output of one dependency edge.
    #[error(
        "node {node_id:?} ambiguously consumes {matches} outputs from dependency {dependency:?}"
    )]
    AmbiguousDependencyOutput {
        /// Dependent node.
        node_id: NodeId,
        /// Upstream node with ambiguous selected outputs.
        dependency: NodeId,
        /// Number of matching declared input artifacts.
        matches: usize,
    },
    /// Node input reference is not registered in the project catalog.
    #[error("node {node_id:?} input is not cataloged: {artifact:?}")]
    InputNotCataloged {
        /// Affected node ID.
        node_id: NodeId,
        /// Missing catalog reference.
        artifact: ArtifactRef,
    },
    /// Schema-bound inputs were declared without binding a verifying store.
    #[error("node {node_id:?} declares semantic inputs but no artifact store was bound")]
    SemanticStoreRequired {
        /// Affected node.
        node_id: NodeId,
    },
    /// Schema-bound input ID is absent from the project catalog.
    #[error("node {node_id:?} semantic input {artifact} is not cataloged")]
    SemanticInputNotCataloged {
        /// Affected node.
        node_id: NodeId,
        /// Missing schema-bound input.
        artifact: ArtifactId,
    },
    /// Store-backed semantic input is missing, mutated, or otherwise unverifiable.
    #[error("node {node_id:?} semantic input {artifact} failed verification: {source}")]
    SemanticInputIntegrity {
        /// Affected node.
        node_id: NodeId,
        /// Unavailable or invalid semantic input.
        artifact: ArtifactId,
        /// Store verification failure.
        #[source]
        source: ArtifactStoreError,
    },
    /// A typed node lifecycle operation failed.
    #[error("node {node_id:?} failed: {source}")]
    NodeFailed {
        /// Failed node ID.
        node_id: NodeId,
        /// Stage-specific node error.
        #[source]
        source: NodeError,
    },
    /// Canonical encoded output exceeds the configured per-output limit.
    #[error(
        "node {node_id:?} encoded {encoded_bytes} inline bytes, exceeding limit {limit_bytes}"
    )]
    InlineOutputTooLarge {
        /// Affected node ID.
        node_id: NodeId,
        /// Actual encoded output size.
        encoded_bytes: usize,
        /// Configured maximum encoded size.
        limit_bytes: usize,
    },
    /// A node codec did not reach a stable representation before commit.
    #[error("node {node_id:?} codec is not canonical after decode/re-encode normalization")]
    NonCanonicalCodec {
        /// Affected node ID.
        node_id: NodeId,
    },
    /// Cached artifact kind does not match the node's current codec contract.
    #[error("cached output kind mismatch for node {node_id:?}: expected {expected}, observed {observed}")]
    CachedOutputKind {
        /// Affected node ID.
        node_id: NodeId,
        /// Current codec artifact kind.
        expected: &'static str,
        /// Cached artifact kind.
        observed: String,
    },
    /// Node omitted required execution or implementation key material.
    #[error("invalid cache material for node {node_id:?}")]
    InvalidCacheMaterial {
        /// Affected node ID.
        node_id: NodeId,
    },
    /// Inline-output limit is zero.
    #[error("scheduler inline-output limit must be positive")]
    InvalidSchedulerLimit,
    /// Project catalog, cache-integrity, or commit operation failed.
    #[error(transparent)]
    Project(#[from] ProjectError),
}
