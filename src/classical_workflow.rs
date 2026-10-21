use crate::workflow_node::{NodeAnalysis, TypedNode};
use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::Serialize;

use crate::observation_window_artifact::full_window_artifact as window_artifact;
use crate::{
    classical::{
        analyze_classical_spatial_pattern, ClassicalSpatialConfig, ClassicalSpatialLimits,
        ClassicalSpatialResult, ClassicalSpatialResultDocument,
    },
    geom::window::ObservationWindow2D,
    marked_workflow::pattern_artifact,
    Pattern,
};

const NODE_KIND: &str = "classical_spatial_analysis";
const CONFIG_KIND: &str = "application/vnd.marklab.classical-spatial-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.classical-spatial-result+json;version=1";
const EXECUTION_POLICY: &[u8] =
    b"serial;exact-rstar;standard-border;whole-pattern-conditional-csr;erl";
const ADAPTER_REVISION: &str = "classical-spatial-analysis-node-v2";

/// Complete classical spatial workflow exposed as one cache-addressed project node.
pub struct ClassicalSpatialAnalysisNode<'a> {
    node: TypedNode<ClassicalSpatialAnalysis<'a>>,
}

struct ClassicalSpatialAnalysis<'a> {
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a ClassicalSpatialConfig,
}

impl<'a> ClassicalSpatialAnalysisNode<'a> {
    /// Catalog exact pattern, window, and configuration identities and construct the node.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a ClassicalSpatialConfig,
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
        config: &'a ClassicalSpatialConfig,
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
                analysis: ClassicalSpatialAnalysis {
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

    /// Return the dependency-free graph specification.
    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for ClassicalSpatialAnalysis<'_> {
    type Output = ClassicalSpatialResult;
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
        analyze_classical_spatial_pattern(self.pattern, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        ClassicalSpatialResultDocument::new(output.clone())
            .and_then(|document| document.to_json_pretty())
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let text = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        ClassicalSpatialResultDocument::from_json(text)
            .map(ClassicalSpatialResultDocument::into_analysis)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(ClassicalSpatialAnalysisNode<'_> => ClassicalSpatialResult);

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    simulations: usize,
    seed: u64,
    alpha: f64,
    maximum_points: usize,
    maximum_radii: usize,
    maximum_pair_visits: usize,
    maximum_csr_draws: usize,
    maximum_retained_bytes: usize,
}

fn config_artifact(config: &ClassicalSpatialConfig) -> Result<ArtifactRef, NodeError> {
    let ClassicalSpatialLimits {
        maximum_points,
        maximum_radii,
        maximum_pair_visits,
        maximum_csr_draws,
        maximum_retained_bytes,
    } = config.limits();
    let encoded = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        maximum_points,
        maximum_radii,
        maximum_pair_visits,
        maximum_csr_draws,
        maximum_retained_bytes,
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &encoded).map_err(NodeError::input)
}
