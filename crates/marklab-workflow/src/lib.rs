#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Minimal typed DAG and single-node local scheduler for Marklab.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};

pub use marklab_project::{
    ArtifactRef, ContentDigest, ContentDigestWriter, MarklabProject, ProjectError, SuccessfulRun,
};
use thiserror::Error;

type BoxError = Box<dyn Error + Send + Sync + 'static>;

/// Stable, path-safe workflow node identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(String);

impl NodeId {
    /// Validate a non-empty 1-128 byte ID using `[A-Za-z0-9_.-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, WorkflowError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(WorkflowError::InvalidNodeId { value });
        }
        Ok(Self(value))
    }

    /// Borrow the validated ID text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Versioned node specification and dependency declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeSpec {
    id: NodeId,
    kind: String,
    version: u32,
    dependencies: Vec<NodeId>,
}

impl NodeSpec {
    /// Validate a versioned node specification and canonicalize dependency order.
    pub fn new(
        id: NodeId,
        kind: impl Into<String>,
        version: u32,
        mut dependencies: Vec<NodeId>,
    ) -> Result<Self, WorkflowError> {
        let kind = kind.into();
        if kind.is_empty()
            || kind.len() > 128
            || !kind
                .as_bytes()
                .iter()
                .all(|byte| (0x21..=0x7e).contains(byte))
            || version == 0
        {
            return Err(WorkflowError::InvalidNodeSpec {
                node_id: id.0,
                reason: "kind must be 1-128 visible ASCII bytes and version must be positive"
                    .into(),
            });
        }
        dependencies.sort();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WorkflowError::DuplicateDependency { node_id: id.0 });
        }
        Ok(Self {
            id,
            kind,
            version,
            dependencies,
        })
    }

    /// Node identity.
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    /// Stable node-kind identifier.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Positive node implementation version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Sorted direct dependencies.
    pub fn dependencies(&self) -> &[NodeId] {
        &self.dependencies
    }

    /// Deterministic digest of ID, kind, version, and sorted dependencies.
    pub fn digest(&self) -> ContentDigest {
        let version = self.version.to_be_bytes();
        let mut fields = vec![
            b"marklab-node-spec-v1".to_vec(),
            self.id.as_str().as_bytes().to_vec(),
            self.kind.as_bytes().to_vec(),
            version.to_vec(),
        ];
        for dependency in &self.dependencies {
            fields.push(dependency.as_str().as_bytes().to_vec());
        }
        ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
    }
}

/// A dependency-valid, acyclic workflow graph.
pub struct WorkflowGraph {
    nodes: BTreeMap<NodeId, NodeSpec>,
}

impl WorkflowGraph {
    /// Build a graph after rejecting duplicate IDs, invalid edges, and cycles.
    pub fn new(nodes: impl IntoIterator<Item = NodeSpec>) -> Result<Self, WorkflowError> {
        let mut by_id = BTreeMap::new();
        for node in nodes {
            let id = node.id.clone();
            if by_id.insert(id.clone(), node).is_some() {
                return Err(WorkflowError::DuplicateNode { node_id: id });
            }
        }

        for node in by_id.values() {
            for dependency in &node.dependencies {
                if dependency == &node.id {
                    return Err(WorkflowError::SelfDependency {
                        node_id: node.id.clone(),
                    });
                }
                if !by_id.contains_key(dependency) {
                    return Err(WorkflowError::MissingDependency {
                        node_id: node.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }

        validate_acyclic(&by_id)?;
        Ok(Self { nodes: by_id })
    }

    /// Find a validated node specification by ID.
    pub fn node(&self, id: &NodeId) -> Option<&NodeSpec> {
        self.nodes.get(id)
    }
}

fn validate_acyclic(nodes: &BTreeMap<NodeId, NodeSpec>) -> Result<(), WorkflowError> {
    let mut remaining_dependencies = nodes
        .iter()
        .map(|(id, spec)| (id.clone(), spec.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for (id, spec) in nodes {
        for dependency in &spec.dependencies {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(id.clone());
        }
    }
    let mut ready = remaining_dependencies
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<BTreeSet<_>>();
    let mut visited = 0;
    while let Some(id) = ready.pop_first() {
        visited += 1;
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let count = remaining_dependencies
                    .get_mut(child)
                    .expect("validated dependent must exist");
                *count -= 1;
                if *count == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        let cycle_nodes = remaining_dependencies
            .into_iter()
            .filter_map(|(id, count)| (count > 0).then_some(id))
            .collect();
        Err(WorkflowError::Cycle { nodes: cycle_nodes })
    }
}

/// Additional deterministic key material owned by a node implementation.
pub struct CacheKeyMaterial<'a> {
    /// Digest of the complete node configuration.
    pub configuration_digest: ContentDigest,
    /// Explicit resource, seed, and determinism policy bytes.
    pub execution_policy: &'a [u8],
    /// Versioned code/codec identity that must change with semantics.
    pub implementation_identity: &'a str,
}

/// Minimal typed node boundary used by the local scheduler.
pub trait WorkflowNode {
    /// Typed node output decoded through the node's canonical codec.
    type Output;

    /// Versioned graph specification for this node.
    fn spec(&self) -> &NodeSpec;
    /// Ordered immutable input references included in the cache key.
    fn input_artifacts(&self) -> &[ArtifactRef];
    /// Recompute and verify source content before cache lookup or execution.
    fn verify_input_content(&self) -> Result<(), NodeError>;
    /// Additional deterministic execution and implementation key material.
    fn cache_key_material(&self) -> CacheKeyMaterial<'_>;
    /// Execute the node without committing project success state.
    fn execute(&self) -> Result<Self::Output, NodeError>;
    /// Encode an output using a deterministic codec that reaches a stable
    /// representation after one decode/re-encode normalization.
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError>;
    /// Decode and validate canonical output bytes.
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError>;
    /// Artifact media/schema kind emitted by the node codec.
    fn output_kind(&self) -> &'static str;
}

/// Resource bounds enforced before a successful output is committed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerLimits {
    /// Maximum encoded bytes retained for one inline cached output; must be positive.
    ///
    /// This is an artifact-size limit, not a peak-memory limit for codecs that
    /// materialize their output before returning it.
    pub max_inline_output_bytes: usize,
}

/// Stateless scheduler for one dependency-free node.
pub struct LocalScheduler {
    limits: SchedulerLimits,
}

impl LocalScheduler {
    /// Create a scheduler after validating nonzero resource limits.
    pub fn new(limits: SchedulerLimits) -> Result<Self, WorkflowError> {
        if limits.max_inline_output_bytes == 0 {
            return Err(WorkflowError::InvalidSchedulerLimit);
        }
        Ok(Self { limits })
    }

    /// Run or replay one dependency-free node with failure-atomic project commit.
    ///
    /// Graph/spec/input errors, node errors, codec errors, and limit violations
    /// return without recording a successful run.
    pub fn run_single<N: WorkflowNode>(
        &self,
        project: &mut MarklabProject,
        graph: &WorkflowGraph,
        node: &N,
    ) -> Result<NodeRun<N::Output>, WorkflowError> {
        let spec = node.spec();
        let registered = graph
            .node(spec.id())
            .ok_or_else(|| WorkflowError::NodeNotInGraph {
                node_id: spec.id().clone(),
            })?;
        if registered != spec {
            return Err(WorkflowError::NodeSpecMismatch {
                node_id: spec.id().clone(),
            });
        }
        if !spec.dependencies().is_empty() {
            return Err(WorkflowError::DependenciesUnsupported {
                node_id: spec.id().clone(),
            });
        }
        for artifact in node.input_artifacts() {
            if !project.contains(artifact) {
                return Err(WorkflowError::InputNotCataloged {
                    node_id: spec.id().clone(),
                    artifact: artifact.clone(),
                });
            }
        }
        node.verify_input_content()
            .map_err(|source| WorkflowError::NodeFailed {
                node_id: spec.id().clone(),
                source,
            })?;

        let cache_key = self.cache_key(node)?;
        if let Some(artifact) = project
            .cached_output(spec.id().as_str(), cache_key)
            .cloned()
        {
            if artifact.kind() != node.output_kind() {
                return Err(WorkflowError::CachedOutputKind {
                    node_id: spec.id().clone(),
                    expected: node.output_kind(),
                    observed: artifact.kind().to_owned(),
                });
            }
            let bytes = project.read_inline_verified(&artifact)?;
            let output = node
                .decode_output(bytes)
                .map_err(|source| WorkflowError::NodeFailed {
                    node_id: spec.id().clone(),
                    source,
                })?;
            return Ok(NodeRun {
                output,
                artifact,
                cache_key,
                cache_status: CacheStatus::Hit,
            });
        }

        let output = node.execute().map_err(|source| WorkflowError::NodeFailed {
            node_id: spec.id().clone(),
            source,
        })?;
        let encoded = node
            .encode_output(&output)
            .map_err(|source| WorkflowError::NodeFailed {
                node_id: spec.id().clone(),
                source,
            })?;
        self.validate_inline_output_size(spec.id(), encoded.len())?;
        let canonical_output =
            node.decode_output(&encoded)
                .map_err(|source| WorkflowError::NodeFailed {
                    node_id: spec.id().clone(),
                    source,
                })?;
        drop(encoded);
        let canonical_encoded =
            node.encode_output(&canonical_output)
                .map_err(|source| WorkflowError::NodeFailed {
                    node_id: spec.id().clone(),
                    source,
                })?;
        self.validate_inline_output_size(spec.id(), canonical_encoded.len())?;
        let committed_output =
            node.decode_output(&canonical_encoded)
                .map_err(|source| WorkflowError::NodeFailed {
                    node_id: spec.id().clone(),
                    source,
                })?;
        let stable_encoded =
            node.encode_output(&committed_output)
                .map_err(|source| WorkflowError::NodeFailed {
                    node_id: spec.id().clone(),
                    source,
                })?;
        self.validate_inline_output_size(spec.id(), stable_encoded.len())?;
        if stable_encoded != canonical_encoded {
            return Err(WorkflowError::NonCanonicalCodec {
                node_id: spec.id().clone(),
            });
        }
        drop(stable_encoded);
        let artifact = project.commit_success(
            spec.id().as_str(),
            cache_key,
            node.output_kind(),
            canonical_encoded,
        )?;
        Ok(NodeRun {
            output: committed_output,
            artifact,
            cache_key,
            cache_status: CacheStatus::Miss,
        })
    }

    fn validate_inline_output_size(
        &self,
        node_id: &NodeId,
        encoded_bytes: usize,
    ) -> Result<(), WorkflowError> {
        if encoded_bytes > self.limits.max_inline_output_bytes {
            Err(WorkflowError::InlineOutputTooLarge {
                node_id: node_id.clone(),
                encoded_bytes,
                limit_bytes: self.limits.max_inline_output_bytes,
            })
        } else {
            Ok(())
        }
    }

    fn cache_key<N: WorkflowNode>(&self, node: &N) -> Result<ContentDigest, WorkflowError> {
        let material = node.cache_key_material();
        if material.execution_policy.is_empty()
            || material.implementation_identity.trim().is_empty()
        {
            return Err(WorkflowError::InvalidCacheMaterial {
                node_id: node.spec().id().clone(),
            });
        }
        let specification = node.spec().digest();
        let limit = (self.limits.max_inline_output_bytes as u128).to_be_bytes();
        let mut fields = vec![
            b"marklab-workflow-cache-v1".to_vec(),
            specification.as_bytes().to_vec(),
        ];
        for artifact in node.input_artifacts() {
            fields.push(b"input".to_vec());
            fields.push(artifact.kind().as_bytes().to_vec());
            fields.push(artifact.digest().as_bytes().to_vec());
            fields.push(artifact.byte_len().to_be_bytes().to_vec());
        }
        fields.push(b"configuration".to_vec());
        fields.push(material.configuration_digest.as_bytes().to_vec());
        fields.push(b"execution-policy".to_vec());
        fields.push(material.execution_policy.to_vec());
        fields.push(b"implementation".to_vec());
        fields.push(material.implementation_identity.as_bytes().to_vec());
        fields.push(b"max-inline-output".to_vec());
        fields.push(limit.to_vec());
        Ok(ContentDigest::from_framed(fields.iter().map(Vec::as_slice)))
    }
}

/// Whether a node result was executed or replayed from verified project bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheStatus {
    /// Verified cached bytes were decoded without execution.
    Hit,
    /// The node executed, round-tripped its codec, and committed successfully.
    Miss,
}

/// Typed output and content identity returned by the local scheduler.
pub struct NodeRun<T> {
    /// Canonically decoded node output.
    pub output: T,
    /// Reference to the canonical encoded output bytes.
    pub artifact: ArtifactRef,
    /// Deterministic key used for lookup and commit.
    pub cache_key: ContentDigest,
    /// Cache hit/miss disposition.
    pub cache_status: CacheStatus,
}

/// Errors originating inside a node boundary, separated by lifecycle stage.
#[derive(Debug, Error)]
pub enum NodeError {
    /// Input reference construction or verification failed.
    #[error("input verification failed: {source}")]
    Input {
        /// Underlying contextual error.
        #[source]
        source: BoxError,
    },
    /// Canonical scientific execution failed.
    #[error("execution failed: {source}")]
    Execution {
        /// Underlying contextual error.
        #[source]
        source: BoxError,
    },
    /// Output encoding or validation failed.
    #[error("output encoding failed: {source}")]
    Encoding {
        /// Underlying contextual error.
        #[source]
        source: BoxError,
    },
    /// Cached or newly encoded output could not be decoded.
    #[error("output decoding failed: {source}")]
    Decoding {
        /// Underlying contextual error.
        #[source]
        source: BoxError,
    },
}

impl NodeError {
    /// Wrap a typed input construction/verification error.
    pub fn input(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Input {
            source: Box::new(error),
        }
    }

    /// Wrap a typed execution error.
    pub fn execution(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Execution {
            source: Box::new(error),
        }
    }

    /// Wrap a typed output-encoding error.
    pub fn encoding(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Encoding {
            source: Box::new(error),
        }
    }

    /// Wrap a typed output-decoding error.
    pub fn decode(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Decoding {
            source: Box::new(error),
        }
    }
}

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
    /// B-04's intentionally narrow scheduler received a dependent node.
    #[error("B-04 local scheduler cannot execute dependent node {node_id:?}")]
    DependenciesUnsupported {
        /// Node requiring multi-node scheduling.
        node_id: NodeId,
    },
    /// Node input reference is not registered in the project catalog.
    #[error("node {node_id:?} input is not cataloged: {artifact:?}")]
    InputNotCataloged {
        /// Affected node ID.
        node_id: NodeId,
        /// Missing catalog reference.
        artifact: ArtifactRef,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_rejects_cycles_and_missing_dependencies() {
        let a = NodeId::new("a").expect("a");
        let b = NodeId::new("b").expect("b");
        let a_spec = NodeSpec::new(a.clone(), "test", 1, vec![b.clone()]).expect("a spec");
        let b_spec = NodeSpec::new(b.clone(), "test", 1, vec![a]).expect("b spec");
        assert!(matches!(
            WorkflowGraph::new([a_spec, b_spec]),
            Err(WorkflowError::Cycle { .. })
        ));

        let missing = NodeSpec::new(NodeId::new("root").expect("root"), "test", 1, vec![b])
            .expect("missing spec");
        assert!(matches!(
            WorkflowGraph::new([missing]),
            Err(WorkflowError::MissingDependency { .. })
        ));
    }
}
