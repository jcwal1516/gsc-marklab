use crate::result_vocabulary::EdgeCorrection;
use crate::workflow_codec::invalid;
use crate::workflow_node::{NodeAnalysis, TypedNode};
use std::io;

use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::Serialize;

use crate::{
    marked_workflow::pattern_artifact,
    pair_correlation::{configuration_digest, HomogeneousPairCorrelationConfig},
    spatial::{
        HomogeneousPairCorrelationResult, PairCorrelationKernel, PairCorrelationPointStatus,
    },
    ObservationWindow2D, Pattern,
};

const NODE_KIND: &str = "homogeneous_pair_correlation";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str =
    "application/vnd.marklab.homogeneous-pair-correlation-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.homogeneous-pair-correlation-result+json;version=2";
const EXECUTION_POLICY: &[u8] = b"serial;exact-rstar;epanechnikov;explicit-bandwidth;standard-border-radius-plus-bandwidth;whole-pattern-conditional-csr;erl";
const IMPLEMENTATION: &str = "homogeneous-pair-correlation-analysis-node-v4";

/// Cache-addressed homogeneous pair-correlation workflow.
pub struct HomogeneousPairCorrelationAnalysisNode<'a> {
    node: TypedNode<HomogeneousPairCorrelationAnalysis<'a>>,
}

struct HomogeneousPairCorrelationAnalysis<'a> {
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a HomogeneousPairCorrelationConfig,
}

impl<'a> HomogeneousPairCorrelationAnalysisNode<'a> {
    /// Bind the exact pattern, window, kernel controls, null design, and limits.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a HomogeneousPairCorrelationConfig,
    ) -> Result<Self, NodeError> {
        let pattern_ref = pattern_artifact(pattern)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(config)?;
        let inputs = [pattern_ref, window_ref, config_ref.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        let node_spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let node_configuration_digest = config_ref.digest();
        let node_implementation_identity = format!(
            "marklab/{};adapter={IMPLEMENTATION}",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec: node_spec,
                analysis: HomogeneousPairCorrelationAnalysis {
                    pattern,
                    window,
                    config,
                },
                inputs,
                configuration_digest: node_configuration_digest,
                execution_policy: std::borrow::Cow::Borrowed(EXECUTION_POLICY),
                implementation_identity: node_implementation_identity,
            },
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for HomogeneousPairCorrelationAnalysis<'_> {
    type Output = HomogeneousPairCorrelationResult;
    type Inputs = [ArtifactRef; 3];

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.pattern)?;
        crate::workflow_node::verify_input(inputs, 0, &pattern)?;
        let window = window_artifact(self.window)?;
        crate::workflow_node::verify_input(inputs, 1, &window)?;
        let config = config_artifact(self.config)?;
        crate::workflow_node::verify_input(inputs, 2, &config)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::spatial::homogeneous_pair_correlation(self.pattern, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate_result(output, self.pattern, self.window, self.config)
            .map_err(NodeError::encoding)?;
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let result: HomogeneousPairCorrelationResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate_result(&result, self.pattern, self.window, self.config)
            .map_err(NodeError::decode)?;
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(HomogeneousPairCorrelationAnalysisNode<'_> => HomogeneousPairCorrelationResult);

#[derive(Serialize)]
struct WindowArtifact<'a> {
    logical_digest: String,
    coordinate_frame: &'a str,
}

fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&WindowArtifact {
        logical_digest: window.descriptor().logical_digest.to_string(),
        coordinate_frame: "existing_pattern_physical_xy",
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(WINDOW_KIND, &encoded).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    bandwidth_um: f64,
    kernel: PairCorrelationKernel,
    simulations: usize,
    seed: u64,
    alpha: f64,
    limits: crate::spatial::ClassicalSpatialLimits,
    logical_digest: String,
}

fn config_artifact(config: &HomogeneousPairCorrelationConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        bandwidth_um: config.bandwidth_um(),
        kernel: PairCorrelationKernel::Epanechnikov,
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        limits: config.limits(),
        logical_digest: configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &encoded).map_err(NodeError::input)
}

fn validate_result(
    result: &HomogeneousPairCorrelationResult,
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &HomogeneousPairCorrelationConfig,
) -> io::Result<()> {
    let descriptor = window.descriptor();
    if result.case_id != pattern.meta.case_id
        || result.timepoint != pattern.meta.timepoint
        || result.kernel != PairCorrelationKernel::Epanechnikov
        || result.bandwidth_um.to_bits() != config.bandwidth_um().to_bits()
        || result.edge_correction != EdgeCorrection::StandardBorderRadiusPlusBandwidth
        || result.null_model != crate::spatial::ClassicalNullModel::HomogeneousCsrConditionalOnCount
        || result.randomization_unit
            != crate::spatial::ClassicalRandomizationUnit::WholeLocationPattern
        || result.observed_pair_visits > result.total_pair_visits
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.configuration_digest != configuration_digest(config).to_string()
        || result.limits != config.limits()
        || result.simulations != config.simulations()
        || result.seed != config.seed()
        || result.alpha.to_bits() != config.alpha().to_bits()
        || result.window.logical_digest != descriptor.logical_digest.to_string()
        || result.window.area_um2.to_bits() != descriptor.area_um2.to_bits()
        || result.window.perimeter_um.to_bits() != descriptor.perimeter_um.to_bits()
        || result.curve.len() != config.radii_um().len()
    {
        return Err(invalid("result does not match its cache-bound request"));
    }
    if !result.bandwidth_um.is_finite()
        || !result.alpha.is_finite()
        || !result.window.area_um2.is_finite()
        || !result.window.perimeter_um.is_finite()
        || result
            .window
            .bounds_um
            .iter()
            .any(|value| !value.is_finite())
    {
        return Err(invalid("result contains a non-finite summary"));
    }
    for (point, radius) in result.curve.iter().zip(config.radii_um()) {
        let available = point.status == PairCorrelationPointStatus::Available
            || (point.status == PairCorrelationPointStatus::NoPairsInKernelSupport
                && pattern.len() >= 2
                && point.eligible_centers > 0
                && point.directed_pairs_in_support == 0
                && point.kernel_weight_sum == 0.0
                && point.g == Some(0.0));
        if point.radius_um.to_bits() != radius.to_bits()
            || !point.radius_um.is_finite()
            || !point.kernel_weight_sum.is_finite()
            || point.kernel_weight_sum < 0.0
            || point.theoretical_g.to_bits() != 1.0_f64.to_bits()
            || point.g.is_some() != available
            || point.g.is_some_and(|value| !value.is_finite())
            || point.lower_g.is_some_and(|value| !value.is_finite())
            || point.upper_g.is_some_and(|value| !value.is_finite())
            || point.inference_eligible != (point.lower_g.is_some() && point.upper_g.is_some())
        {
            return Err(invalid("result curve is inconsistent or non-finite"));
        }
    }
    if let Some(inference) = &result.inference {
        if !inference.p_global.is_finite()
            || !inference.erl_depth.is_finite()
            || !inference.critical_depth.is_finite()
            || inference.eligible_radius_count
                != result
                    .curve
                    .iter()
                    .filter(|point| point.inference_eligible)
                    .count()
        {
            return Err(invalid("result inference is inconsistent or non-finite"));
        }
    } else if result.curve.iter().any(|point| point.inference_eligible) {
        return Err(invalid("result has envelopes without global inference"));
    }
    Ok(())
}
