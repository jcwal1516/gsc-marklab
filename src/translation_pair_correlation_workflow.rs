use crate::workflow_node::{NodeAnalysis, TypedNode};
use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::Serialize;

use crate::{
    marked_workflow::pattern_artifact,
    spatial::PairCorrelationKernel,
    translation_pair_correlation::{
        analyze_translation_pair_correlation, translation_pair_correlation_configuration_digest,
        TranslationPairCorrelationConfig, TranslationPairCorrelationResult,
        TranslationPairCorrelationResultDocument,
    },
    translation_spatial_workflow::window_artifact,
    Pattern,
};

const NODE_KIND: &str = "translation_pair_correlation_analysis";
const CONFIG_KIND: &str =
    "application/vnd.marklab.translation-pair-correlation-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.translation-pair-correlation-result+json;version=1";
const EXECUTION_POLICY: &[u8] = b"serial;exact-polygon-translation-overlap;epanechnikov;explicit-bandwidth;whole-pattern-conditional-csr;erl";
const ADAPTER_REVISION: &str = "translation-pair-correlation-analysis-node-v5";

/// Cache-addressed exact translation-corrected homogeneous pair-correlation node.
pub struct TranslationPairCorrelationAnalysisNode<'a> {
    node: TypedNode<TranslationPairCorrelationAnalysis<'a>>,
}

struct TranslationPairCorrelationAnalysis<'a> {
    pattern: &'a Pattern,
    window: &'a crate::ObservationWindow2D,
    config: &'a TranslationPairCorrelationConfig,
}

impl<'a> TranslationPairCorrelationAnalysisNode<'a> {
    /// Catalog exact pattern, window, bandwidth, null, and resource identities.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a crate::ObservationWindow2D,
        config: &'a TranslationPairCorrelationConfig,
    ) -> Result<Self, NodeError> {
        Self::new_with_implementation_identity(
            project,
            id,
            pattern,
            window,
            config,
            format!(
                "marklab/{};adapter={ADAPTER_REVISION};{}",
                env!("CARGO_PKG_VERSION"),
                crate::common::GEO_IDENTITY
            ),
            Vec::new(),
        )
    }

    pub(crate) fn new_with_implementation_identity(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a crate::ObservationWindow2D,
        config: &'a TranslationPairCorrelationConfig,
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
                analysis: TranslationPairCorrelationAnalysis {
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

impl NodeAnalysis for TranslationPairCorrelationAnalysis<'_> {
    type Output = TranslationPairCorrelationResult;
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
        analyze_translation_pair_correlation(self.pattern, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        TranslationPairCorrelationResultDocument::new(output.clone())
            .and_then(|document| document.to_json_pretty())
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let text = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        TranslationPairCorrelationResultDocument::from_json(text)
            .map(TranslationPairCorrelationResultDocument::into_analysis)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(TranslationPairCorrelationAnalysisNode<'_> => TranslationPairCorrelationResult);

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    bandwidth_um: f64,
    kernel: PairCorrelationKernel,
    simulations: usize,
    seed: u64,
    alpha: f64,
    limits: crate::spatial::TranslationSpatialLimits,
    logical_digest: String,
}

fn config_artifact(config: &TranslationPairCorrelationConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        bandwidth_um: config.bandwidth_um(),
        kernel: PairCorrelationKernel::Epanechnikov,
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        limits: config.limits(),
        logical_digest: translation_pair_correlation_configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &encoded).map_err(NodeError::input)
}
