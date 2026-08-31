use std::error::Error;

use thiserror::Error;

use crate::{ArtifactId, ArtifactRef, ContentDigest, NodeSpec};

type BoxError = Box<dyn Error + Send + Sync + 'static>;

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
    /// Ordered schema-bound artifact inputs that require store verification.
    fn semantic_input_artifacts(&self) -> &[ArtifactId] {
        &[]
    }
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
