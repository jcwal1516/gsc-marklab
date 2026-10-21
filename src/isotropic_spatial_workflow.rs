use crate::workflow_node::{NodeAnalysis, TypedNode};
use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::Serialize;

pub(crate) use crate::observation_window_artifact::full_window_artifact as window_artifact;
use crate::{
    geom::window::ObservationWindow2D,
    isotropic_spatial::{
        analyze_isotropic_spatial_pattern, IsotropicSpatialConfig, IsotropicSpatialLimits,
        IsotropicSpatialResult, IsotropicSpatialResultDocument,
    },
    marked_workflow::pattern_artifact,
    Pattern,
};

const NODE_KIND: &str = "isotropic_spatial_analysis";
const CONFIG_KIND: &str = "application/vnd.marklab.isotropic-spatial-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.isotropic-spatial-result+json;version=1";
const EXECUTION_POLICY: &[u8] =
    b"serial;exact-segment-circle-visible-arcs;whole-pattern-conditional-csr;erl";
const ADAPTER_REVISION: &str = "isotropic-spatial-analysis-node-v3";

/// Cache-addressed exact isotropic homogeneous K/L node.
pub struct IsotropicSpatialAnalysisNode<'a> {
    node: TypedNode<IsotropicSpatialAnalysis<'a>>,
}

struct IsotropicSpatialAnalysis<'a> {
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a IsotropicSpatialConfig,
}

impl<'a> IsotropicSpatialAnalysisNode<'a> {
    /// Catalog exact pattern, window, and configuration identities.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a IsotropicSpatialConfig,
    ) -> Result<Self, NodeError> {
        Self::new_with_implementation_identity(
            project,
            id,
            pattern,
            window,
            config,
            format!(
                "marklab/{};adapter={ADAPTER_REVISION}",
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
        config: &'a IsotropicSpatialConfig,
        implementation_identity: String,
        source_artifacts: Vec<ArtifactRef>,
    ) -> Result<Self, NodeError> {
        let pattern_ref = pattern_artifact(pattern)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(config)?;
        let spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let mut inputs = vec![pattern_ref, window_ref, config_ref.clone()];
        inputs.extend(source_artifacts);
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        let node_configuration_digest = config_ref.digest();
        Ok(Self {
            node: TypedNode {
                spec,
                analysis: IsotropicSpatialAnalysis {
                    pattern,
                    window,
                    config,
                },
                inputs,
                configuration_digest: node_configuration_digest,
                execution_policy: std::borrow::Cow::Borrowed(EXECUTION_POLICY),
                implementation_identity,
            },
        })
    }

    /// Dependency-free graph specification.
    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for IsotropicSpatialAnalysis<'_> {
    type Output = IsotropicSpatialResult;
    type Inputs = Vec<ArtifactRef>;

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.pattern)?;
        crate::workflow_node::verify_input(inputs, 0, &pattern)?;
        let window = window_artifact(self.window)?;
        crate::workflow_node::verify_input(inputs, 1, &window)?;
        let config = config_artifact(self.config)?;
        crate::workflow_node::verify_input(inputs, 2, &config)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        analyze_isotropic_spatial_pattern(self.pattern, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        IsotropicSpatialResultDocument::new(output.clone())
            .and_then(|document| document.to_json_pretty())
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let text = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        IsotropicSpatialResultDocument::from_json(text)
            .map(IsotropicSpatialResultDocument::into_analysis)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(IsotropicSpatialAnalysisNode<'_> => IsotropicSpatialResult);

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    simulations: usize,
    seed: u64,
    alpha: f64,
    limits: IsotropicSpatialLimits,
}

fn config_artifact(config: &IsotropicSpatialConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        limits: config.limits(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &encoded).map_err(NodeError::input)
}
