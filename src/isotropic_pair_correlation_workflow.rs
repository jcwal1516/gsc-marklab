use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    isotropic_pair_correlation::{
        analyze_isotropic_pair_correlation, isotropic_pair_correlation_configuration_digest,
        IsotropicPairCorrelationConfig, IsotropicPairCorrelationResult,
        IsotropicPairCorrelationResultDocument,
    },
    isotropic_spatial_workflow::window_artifact,
    workflow::pattern_artifact,
    ObservationWindow2D, PairCorrelationKernel, Pattern,
};

const CONFIG_KIND: &str =
    "application/vnd.marklab.isotropic-pair-correlation-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.isotropic-pair-correlation-result+json;version=1";
const POLICY: &[u8] =
    b"serial;exact-visible-arcs;epanechnikov;explicit-bandwidth;whole-pattern-conditional-csr;erl";

/// Cache-addressed exact isotropic homogeneous pair-correlation node.
pub struct IsotropicPairCorrelationAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a IsotropicPairCorrelationConfig,
    inputs: Vec<ArtifactRef>,
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> IsotropicPairCorrelationAnalysisNode<'a> {
    /// Catalog exact pattern, window, bandwidth, null, and resource identities.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a IsotropicPairCorrelationConfig,
    ) -> Result<Self, NodeError> {
        Self::new_with_implementation_identity(
            project,
            id,
            pattern,
            window,
            config,
            format!(
                "marklab/{};adapter=isotropic-pair-correlation-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
            Vec::new(),
        )
    }
    pub(crate) fn new_with_implementation_identity(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a IsotropicPairCorrelationConfig,
        implementation_identity: String,
        source_artifacts: Vec<ArtifactRef>,
    ) -> Result<Self, NodeError> {
        let pattern_ref = pattern_artifact(pattern)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(config)?;
        let mut inputs = vec![pattern_ref, window_ref, config_ref.clone()];
        inputs.extend(source_artifacts);
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(id, "isotropic_pair_correlation_analysis", 1, Vec::new())
                .map_err(NodeError::input)?,
            pattern,
            window,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity,
        })
    }
    /// Dependency-free graph specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for IsotropicPairCorrelationAnalysisNode<'_> {
    type Output = IsotropicPairCorrelationResult;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.pattern)?;
        self.inputs[0]
            .verify_identity(pattern.digest(), pattern.byte_len())
            .map_err(NodeError::input)?;
        let window = window_artifact(self.window)?;
        self.inputs[1]
            .verify_identity(window.digest(), window.byte_len())
            .map_err(NodeError::input)?;
        let config = config_artifact(self.config)?;
        self.inputs[2]
            .verify_identity(config.digest(), config.byte_len())
            .map_err(NodeError::input)
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: POLICY,
            implementation_identity: &self.implementation_identity,
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        analyze_isotropic_pair_correlation(self.pattern, self.window, self.config)
            .map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        IsotropicPairCorrelationResultDocument::new(output.clone())
            .and_then(|document| document.to_json_pretty())
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let text = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        IsotropicPairCorrelationResultDocument::from_json(text)
            .map(IsotropicPairCorrelationResultDocument::into_analysis)
            .map_err(NodeError::decode)
    }
    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    bandwidth_um: f64,
    kernel: PairCorrelationKernel,
    simulations: usize,
    seed: u64,
    alpha: f64,
    limits: crate::IsotropicSpatialLimits,
    logical_digest: String,
}

fn config_artifact(config: &IsotropicPairCorrelationConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        bandwidth_um: config.bandwidth_um(),
        kernel: PairCorrelationKernel::Epanechnikov,
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        limits: config.limits(),
        logical_digest: isotropic_pair_correlation_configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &encoded).map_err(NodeError::input)
}
