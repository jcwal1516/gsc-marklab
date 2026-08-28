use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    workflow::pattern_artifact, ObservationWindow2D, PairCorrelationKernel,
    PairCorrelationPointStatus, Pattern,
};

use super::{
    analyze_inhomogeneous_pair_correlation,
    codec::{invalid, validate_intensity_summary},
    g::g_configuration_digest,
    workflow::window_artifact,
    InhomogeneousPairCorrelationConfig, InhomogeneousPairCorrelationResult,
    InhomogeneousSpatialLimits,
};

const NODE_KIND: &str = "inhomogeneous_pair_correlation";
const CONFIG_KIND: &str =
    "application/vnd.marklab.inhomogeneous-pair-correlation-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.inhomogeneous-pair-correlation-result+json;version=1";
const POLICY: &[u8] = b"serial;gaussian-2d;leave-one-out-n-over-n-minus-one;cell-centred-window-quadrature;epanechnikov;standard-border-radius-plus-pair-bandwidth-inverse-intensity-ratio;fixed-gridded-inhomogeneous-binomial;erl";

/// Cache-addressed inhomogeneous pair-correlation workflow.
pub struct InhomogeneousPairCorrelationAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a InhomogeneousPairCorrelationConfig,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> InhomogeneousPairCorrelationAnalysisNode<'a> {
    /// Bind the exact pattern, observation window, intensity pilot, pair bandwidth, null, and limits.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a InhomogeneousPairCorrelationConfig,
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
                "marklab/{};adapter=inhomogeneous-pair-correlation-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for InhomogeneousPairCorrelationAnalysisNode<'_> {
    type Output = InhomogeneousPairCorrelationResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = [
            pattern_artifact(self.pattern)?,
            window_artifact(self.window)?,
            config_artifact(self.config)?,
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
        analyze_inhomogeneous_pair_correlation(self.pattern, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.pattern, self.window, self.config).map_err(NodeError::encoding)?;
        crate::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = crate::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        validate(&output, self.pattern, self.window, self.config).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    intensity_bandwidth_um: f64,
    pair_bandwidth_um: f64,
    integration_grid: [usize; 2],
    simulations: usize,
    seed: u64,
    alpha: f64,
    minimum_intensity_per_um2: f64,
    limits: InhomogeneousSpatialLimits,
    logical_digest: String,
}

fn config_artifact(config: &InhomogeneousPairCorrelationConfig) -> Result<ArtifactRef, NodeError> {
    let intensity = config.intensity_config();
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: intensity.radii_um(),
        intensity_bandwidth_um: intensity.bandwidth_um(),
        pair_bandwidth_um: config.pair_bandwidth_um(),
        integration_grid: intensity.integration_grid(),
        simulations: intensity.simulations(),
        seed: intensity.seed(),
        alpha: intensity.alpha(),
        minimum_intensity_per_um2: intensity.minimum_intensity_per_um2(),
        limits: intensity.limits(),
        logical_digest: g_configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &InhomogeneousPairCorrelationResult,
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousPairCorrelationConfig,
) -> io::Result<()> {
    let intensity = config.intensity_config();
    let descriptor = window.descriptor();
    if result.case_id != pattern.meta.case_id
        || result.timepoint != pattern.meta.timepoint
        || result.window.logical_digest != descriptor.logical_digest.to_string()
        || result.window.area_um2.to_bits() != descriptor.area_um2.to_bits()
        || result.kernel != PairCorrelationKernel::Epanechnikov
        || result.pair_bandwidth_um.to_bits() != config.pair_bandwidth_um().to_bits()
        || result.intensity_bandwidth_um.to_bits() != intensity.bandwidth_um().to_bits()
        || result.edge_correction
            != "standard_border_radius_plus_pair_bandwidth_inverse_intensity_ratio"
        || result.configuration_digest != g_configuration_digest(config).to_string()
        || result.limits != intensity.limits()
        || result.estimated_storage_bytes > intensity.limits().maximum_retained_bytes
        || result.observed_pair_visits > result.total_pair_visits
        || result.total_pair_visits > intensity.limits().maximum_pair_visits
        || result.intensity_evaluations > intensity.limits().maximum_intensity_evaluations
        || result.curve.len() != intensity.radii_um().len()
        || result.inference.null_model != "fixed_gridded_inhomogeneous_binomial"
        || result.inference.randomization_unit != "whole_location_pattern_conditioned_on_count"
        || result.inference.simulations_completed != intensity.simulations()
        || result.inference.seed != intensity.seed()
        || result.inference.alpha.to_bits() != intensity.alpha().to_bits()
        || result.inference.null_draws > intensity.limits().maximum_null_draws
    {
        return Err(invalid("result does not match its cache-bound request"));
    }
    validate_intensity_summary(&result.intensity, pattern, window, intensity)?;
    for (point, radius) in result.curve.iter().zip(intensity.radii_um()) {
        let status_consistent = match point.status {
            PairCorrelationPointStatus::Available => {
                point.eligible_centers > 0
                    && point.directed_pairs_in_support > 0
                    && point.inverse_intensity_kernel_sum > 0.0
                    && point.eligible_center_inverse_intensity_sum > 0.0
                    && point.g.is_some()
            }
            PairCorrelationPointStatus::NoEligibleCenters => {
                point.eligible_centers == 0
                    && point.directed_pairs_in_support == 0
                    && point.inverse_intensity_kernel_sum.to_bits() == 0.0_f64.to_bits()
                    && point.eligible_center_inverse_intensity_sum.to_bits() == 0.0_f64.to_bits()
                    && point.g.is_none()
            }
            PairCorrelationPointStatus::NoPairsInKernelSupport => {
                point.eligible_centers > 0
                    && point.directed_pairs_in_support == 0
                    && point.inverse_intensity_kernel_sum.to_bits() == 0.0_f64.to_bits()
                    && point.eligible_center_inverse_intensity_sum > 0.0
                    && point.g.is_none()
            }
        };
        if !status_consistent
            || point.radius_um.to_bits() != radius.to_bits()
            || !point.inverse_intensity_kernel_sum.is_finite()
            || !point.eligible_center_inverse_intensity_sum.is_finite()
            || point
                .g
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point.theoretical_g.to_bits() != 1.0_f64.to_bits()
            || point.lower_g.is_some_and(|value| !value.is_finite())
            || point.upper_g.is_some_and(|value| !value.is_finite())
            || point.lower_g.is_some() != point.upper_g.is_some()
            || point.inference_eligible != (point.lower_g.is_some() && point.upper_g.is_some())
        {
            return Err(invalid(
                "inhomogeneous g curve is inconsistent or non-finite",
            ));
        }
    }
    let inference = [
        result.inference.p_global,
        result.inference.erl_depth,
        result.inference.critical_depth,
    ];
    if inference.iter().flatten().any(|value| !value.is_finite())
        || inference.iter().any(Option::is_some) != inference.iter().all(Option::is_some)
        || result.inference.eligible_radius_count
            != result
                .curve
                .iter()
                .filter(|point| point.inference_eligible)
                .count()
    {
        return Err(invalid("inference is inconsistent or non-finite"));
    }
    Ok(())
}
