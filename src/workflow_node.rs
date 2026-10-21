//! Shared lifecycle for the crate's typed scientific workflow adapters.

use std::borrow::Cow;

use marklab_project::{ArtifactId, ArtifactRef, ContentDigest};
use marklab_workflow::{CacheKeyMaterial, NodeError, NodeSpec, WorkflowNode};

/// Model-specific work; codecs and input validation remain with the analysis.
pub(crate) trait NodeAnalysis {
    type Output;
    type Inputs: AsRef<[ArtifactRef]>;

    fn semantic_input_artifacts(&self) -> &[ArtifactId] {
        &[]
    }

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError>;
    fn execute(&self) -> Result<Self::Output, NodeError>;
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError>;
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError>;
    fn output_kind(&self) -> &'static str;
}

/// Own the ordered references and exact cache identity without imposing codec bounds.
pub(crate) struct TypedNode<A: NodeAnalysis> {
    pub(crate) spec: NodeSpec,
    pub(crate) analysis: A,
    pub(crate) inputs: A::Inputs,
    pub(crate) configuration_digest: ContentDigest,
    pub(crate) execution_policy: Cow<'static, [u8]>,
    pub(crate) implementation_identity: String,
}

impl<A: NodeAnalysis> TypedNode<A> {
    pub(crate) fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl<A: NodeAnalysis> WorkflowNode for TypedNode<A> {
    type Output = A::Output;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        self.inputs.as_ref()
    }

    fn semantic_input_artifacts(&self) -> &[ArtifactId] {
        self.analysis.semantic_input_artifacts()
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        self.analysis.verify_input_content(self.inputs.as_ref())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: &self.execution_policy,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        self.analysis.execute()
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.analysis.encode_output(output)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        self.analysis.decode_output(bytes)
    }

    fn output_kind(&self) -> &'static str {
        self.analysis.output_kind()
    }
}

/// Check one recomputed reference at the analysis's existing verification point.
pub(crate) fn verify_input(
    inputs: &[ArtifactRef],
    index: usize,
    current: &ArtifactRef,
) -> Result<(), NodeError> {
    inputs[index]
        .verify_identity(current.digest(), current.byte_len())
        .map_err(NodeError::input)
}

/// Preserve the adapters that recompute a complete input group before checking it.
pub(crate) fn verify_inputs(
    inputs: &[ArtifactRef],
    current: impl IntoIterator<Item = ArtifactRef>,
) -> Result<(), NodeError> {
    for (index, (_, current)) in inputs.iter().zip(current).enumerate() {
        verify_input(inputs, index, &current)?;
    }
    Ok(())
}

/// Preserve named public wrappers without exposing the private analysis types.
macro_rules! impl_workflow_node {
    ($node:ty => $output:ty) => {
        impl marklab_workflow::WorkflowNode for $node {
            type Output = $output;

            fn spec(&self) -> &marklab_workflow::NodeSpec {
                marklab_workflow::WorkflowNode::spec(&self.node)
            }

            fn input_artifacts(&self) -> &[marklab_project::ArtifactRef] {
                marklab_workflow::WorkflowNode::input_artifacts(&self.node)
            }

            fn semantic_input_artifacts(&self) -> &[marklab_project::ArtifactId] {
                marklab_workflow::WorkflowNode::semantic_input_artifacts(&self.node)
            }

            fn verify_input_content(&self) -> Result<(), marklab_workflow::NodeError> {
                marklab_workflow::WorkflowNode::verify_input_content(&self.node)
            }

            fn cache_key_material(&self) -> marklab_workflow::CacheKeyMaterial<'_> {
                marklab_workflow::WorkflowNode::cache_key_material(&self.node)
            }

            fn execute(&self) -> Result<Self::Output, marklab_workflow::NodeError> {
                marklab_workflow::WorkflowNode::execute(&self.node)
            }

            fn encode_output(
                &self,
                output: &Self::Output,
            ) -> Result<Box<[u8]>, marklab_workflow::NodeError> {
                marklab_workflow::WorkflowNode::encode_output(&self.node, output)
            }

            fn decode_output(
                &self,
                bytes: &[u8],
            ) -> Result<Self::Output, marklab_workflow::NodeError> {
                marklab_workflow::WorkflowNode::decode_output(&self.node, bytes)
            }

            fn output_kind(&self) -> &'static str {
                marklab_workflow::WorkflowNode::output_kind(&self.node)
            }
        }
    };
}

pub(crate) use impl_workflow_node;
