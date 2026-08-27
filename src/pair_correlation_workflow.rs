use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    pair_correlation::{configuration_digest, HomogeneousPairCorrelationConfig},
    workflow::pattern_artifact,
    HomogeneousPairCorrelationResult, ObservationWindow2D, PairCorrelationKernel,
    PairCorrelationPointStatus, Pattern,
};

const NODE_KIND: &str = "homogeneous_pair_correlation";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str =
    "application/vnd.marklab.homogeneous-pair-correlation-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.homogeneous-pair-correlation-result+json;version=1";
const EXECUTION_POLICY: &[u8] = b"serial;exact-rstar;epanechnikov;explicit-bandwidth;standard-border-radius-plus-bandwidth;whole-pattern-conditional-csr;erl";
const IMPLEMENTATION: &str = "homogeneous-pair-correlation-analysis-node-v1";

/// Cache-addressed homogeneous pair-correlation workflow.
pub struct HomogeneousPairCorrelationAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a HomogeneousPairCorrelationConfig,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
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
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            pattern,
            window,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter={IMPLEMENTATION}",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for HomogeneousPairCorrelationAnalysisNode<'_> {
    type Output = HomogeneousPairCorrelationResult;

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
            execution_policy: EXECUTION_POLICY,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::homogeneous_pair_correlation(self.pattern, self.window, self.config)
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
    limits: crate::ClassicalSpatialLimits,
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
        || result.edge_correction != "standard_border_radius_plus_bandwidth"
        || result.null_model != "homogeneous_csr_conditional_on_count"
        || result.randomization_unit != "whole_location_pattern"
        || result.geometry_build_count != 1
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
        let available = point.status == PairCorrelationPointStatus::Available;
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

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
