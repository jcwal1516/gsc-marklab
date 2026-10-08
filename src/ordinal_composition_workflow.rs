use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    ordinal_composition::configuration_digest, workflow::pattern_artifact,
    DeclaredScalarPatternInput, OrdinalClassCompositionConfig, OrdinalClassCompositionResult,
    ScalarMarkId,
};

const NODE_KIND: &str = "ordinal_class_composition";
const CONFIG_KIND: &str = "application/vnd.marklab.ordinal-class-composition-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.ordinal-class-composition+json;version=1";
const POLICY: &[u8] = b"serial;ordered-level-counts-cdf-median-interval;no-code-arithmetic";

/// Store-aware durable execution node for one typed ordinal composition.
pub struct OrdinalClassCompositionAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    mark_id: &'a ScalarMarkId,
    config: &'a OrdinalClassCompositionConfig,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> OrdinalClassCompositionAnalysisNode<'a> {
    /// Bind the typed table, mark identity, resource policy, and workflow node.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        mark_id: &'a ScalarMarkId,
        config: &'a OrdinalClassCompositionConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let config_ref = config_artifact(input, mark_id, config)?;
        let inputs = [pattern, declared, config_ref.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            input,
            mark_id,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=ordinal-class-composition-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Return the immutable workflow-node specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for OrdinalClassCompositionAnalysisNode<'_> {
    type Output = OrdinalClassCompositionResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn semantic_input_artifacts(&self) -> &[marklab_workflow::ArtifactId] {
        self.input.semantic_artifact_ids()
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = [
            pattern_artifact(self.input.pattern())?,
            self.input
                .declared_artifact_ref()
                .map_err(NodeError::input)?,
            config_artifact(self.input, self.mark_id, self.config)?,
        ];
        for (bound, current) in self.inputs.iter().zip(current) {
            bound
                .verify_identity(current.digest(), current.byte_len())
                .map_err(NodeError::input)?;
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: POLICY,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::ordinal_class_composition(self.input, self.mark_id, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.input, self.mark_id, self.config).map_err(NodeError::encoding)?;
        crate::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = crate::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.mark_id, self.config).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    config: OrdinalClassCompositionConfig,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    config: &OrdinalClassCompositionConfig,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, mark_id, config).map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        mark_id: mark_id.as_str(),
        config: *config,
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &OrdinalClassCompositionResult,
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    config: &OrdinalClassCompositionConfig,
) -> io::Result<()> {
    let expected =
        crate::ordinal_class_composition(input, mark_id, config).map_err(invalid_owned)?;
    if result != &expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordinal composition result does not match its typed request",
        ));
    }
    Ok(())
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
