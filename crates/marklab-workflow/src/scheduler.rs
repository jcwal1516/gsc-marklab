use std::collections::BTreeSet;

use crate::{
    ArtifactRef, ContentDigest, LocalArtifactStore, MarklabProject, NodeId, WorkflowError,
    WorkflowGraph, WorkflowNode,
};

/// Resource bounds enforced before a successful output is committed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerLimits {
    /// Maximum encoded bytes retained for one inline cached output; must be positive.
    ///
    /// This is an artifact-size limit, not a peak-memory limit for codecs that
    /// materialize their output before returning it.
    pub max_inline_output_bytes: usize,
}

/// Stateless scheduler for one typed node at a time.
pub struct LocalScheduler {
    pub(crate) limits: SchedulerLimits,
}

impl LocalScheduler {
    /// Create a scheduler after validating nonzero resource limits.
    pub fn new(limits: SchedulerLimits) -> Result<Self, WorkflowError> {
        if limits.max_inline_output_bytes == 0 {
            return Err(WorkflowError::InvalidSchedulerLimit);
        }
        Ok(Self { limits })
    }

    /// Run or replay one node with failure-atomic project commit.
    ///
    /// Graph/spec/input errors, node errors, codec errors, and limit violations
    /// return without recording a successful run.
    pub fn run_single<N: WorkflowNode>(
        &self,
        project: &mut MarklabProject,
        graph: &WorkflowGraph,
        node: &N,
    ) -> Result<NodeRun<N::Output>, WorkflowError> {
        self.run_single_inner(project, graph, node, None)
    }

    /// Run or replay one node after verifying every schema-bound input in a local store.
    pub fn run_single_with_store<N: WorkflowNode>(
        &self,
        project: &mut MarklabProject,
        graph: &WorkflowGraph,
        node: &N,
        store: &LocalArtifactStore,
    ) -> Result<NodeRun<N::Output>, WorkflowError> {
        self.run_single_inner(project, graph, node, Some(store))
    }

    /// Compute the exact deterministic cache key without executing or mutating project state.
    ///
    /// Durable project adapters use this read-only surface to locate verified bytes before
    /// invoking the normal scheduler hit path. Callers must still use a scheduler run method to
    /// enforce graph, input, codec, and commit semantics.
    pub fn cache_key_for<N: WorkflowNode>(&self, node: &N) -> Result<ContentDigest, WorkflowError> {
        self.cache_key(node)
    }

    fn run_single_inner<N: WorkflowNode>(
        &self,
        project: &mut MarklabProject,
        graph: &WorkflowGraph,
        node: &N,
        store: Option<&LocalArtifactStore>,
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
        for dependency in spec.dependencies() {
            let dependency_spec = graph
                .node(dependency)
                .expect("validated workflow dependency must be registered");
            let matching_outputs = node
                .input_artifacts()
                .iter()
                .filter(|artifact| {
                    project.is_successful_workflow_output(
                        dependency.as_str(),
                        dependency_spec.digest(),
                        artifact,
                    )
                })
                .collect::<BTreeSet<_>>()
                .len();
            match matching_outputs {
                1 => {}
                0 => {
                    return Err(WorkflowError::MissingDependencyOutput {
                        node_id: spec.id().clone(),
                        dependency: dependency.clone(),
                    })
                }
                matches => {
                    return Err(WorkflowError::AmbiguousDependencyOutput {
                        node_id: spec.id().clone(),
                        dependency: dependency.clone(),
                        matches,
                    })
                }
            }
        }
        for artifact in node.input_artifacts() {
            if !project.contains(artifact) {
                return Err(WorkflowError::InputNotCataloged {
                    node_id: spec.id().clone(),
                    artifact: artifact.clone(),
                });
            }
        }
        let semantic_inputs = node.semantic_input_artifacts();
        if !semantic_inputs.is_empty() {
            let store = store.ok_or_else(|| WorkflowError::SemanticStoreRequired {
                node_id: spec.id().clone(),
            })?;
            for artifact in semantic_inputs {
                let record = project.artifact_record(*artifact).ok_or_else(|| {
                    WorkflowError::SemanticInputNotCataloged {
                        node_id: spec.id().clone(),
                        artifact: *artifact,
                    }
                })?;
                store
                    .verify(record)
                    .map_err(|source| WorkflowError::SemanticInputIntegrity {
                        node_id: spec.id().clone(),
                        artifact: *artifact,
                        source,
                    })?;
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
        let artifact = project.commit_workflow_success(
            spec.id().as_str(),
            spec.digest(),
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
        for artifact in node.semantic_input_artifacts() {
            fields.push(b"semantic-input".to_vec());
            fields.push(artifact.digest().as_bytes().to_vec());
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
