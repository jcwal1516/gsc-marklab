use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    workflow::pattern_artifact, InhomogeneousSpatialPointStatus, ObservationWindow2D, Pattern,
};

use super::{
    analyze_inhomogeneous_spatial_pattern,
    codec::{invalid, validate_intensity_summary},
    configuration_digest, InhomogeneousSpatialConfig, InhomogeneousSpatialLimits,
    InhomogeneousSpatialResult,
};

const NODE_KIND: &str = "inhomogeneous_spatial_analysis";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str = "application/vnd.marklab.inhomogeneous-spatial-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.inhomogeneous-spatial-result+json;version=1";
const POLICY: &[u8] = b"serial;gaussian-2d;leave-one-out-n-over-n-minus-one;cell-centred-window-quadrature;standard-border-inverse-intensity-ratio;fixed-gridded-inhomogeneous-binomial;erl";

pub struct InhomogeneousSpatialAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a InhomogeneousSpatialConfig,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> InhomogeneousSpatialAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a InhomogeneousSpatialConfig,
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
                "marklab/{};adapter=inhomogeneous-spatial-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for InhomogeneousSpatialAnalysisNode<'_> {
    type Output = InhomogeneousSpatialResult;

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
        analyze_inhomogeneous_spatial_pattern(self.pattern, self.window, self.config)
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
struct WindowArtifact {
    digest: String,
    area_um2: f64,
    bounds_um: [f64; 4],
}

pub(super) fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let descriptor = window.descriptor();
    let bytes = serde_json::to_vec(&WindowArtifact {
        digest: descriptor.logical_digest.to_string(),
        area_um2: descriptor.area_um2,
        bounds_um: descriptor.bounds_um,
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(WINDOW_KIND, &bytes).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    bandwidth_um: f64,
    integration_grid: [usize; 2],
    simulations: usize,
    seed: u64,
    alpha: f64,
    minimum_intensity_per_um2: f64,
    limits: InhomogeneousSpatialLimits,
    logical_digest: String,
}

fn config_artifact(config: &InhomogeneousSpatialConfig) -> Result<ArtifactRef, NodeError> {
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        bandwidth_um: config.bandwidth_um(),
        integration_grid: config.integration_grid(),
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        minimum_intensity_per_um2: config.minimum_intensity_per_um2(),
        limits: config.limits(),
        logical_digest: configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &InhomogeneousSpatialResult,
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
) -> io::Result<()> {
    let descriptor = window.descriptor();
    if result.case_id != pattern.meta.case_id
        || result.timepoint != pattern.meta.timepoint
        || result.window.logical_digest != descriptor.logical_digest.to_string()
        || result.window.area_um2.to_bits() != descriptor.area_um2.to_bits()
        || result.edge_correction != "standard_border_inverse_intensity_ratio"
        || result.configuration_digest != configuration_digest(config).to_string()
        || result.limits != config.limits()
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.observed_pair_visits > result.total_pair_visits
        || result.total_pair_visits > config.limits().maximum_pair_visits
        || result.intensity_evaluations > config.limits().maximum_intensity_evaluations
        || result.curve.len() != config.radii_um().len()
        || result.inference.null_model != "fixed_gridded_inhomogeneous_binomial"
        || result.inference.randomization_unit != "whole_location_pattern_conditioned_on_count"
        || result.inference.simulations_completed != config.simulations()
        || result.inference.seed != config.seed()
        || result.inference.alpha.to_bits() != config.alpha().to_bits()
        || result.inference.null_draws > config.limits().maximum_null_draws
    {
        return Err(invalid("result does not match its cache-bound request"));
    }
    validate_intensity_summary(&result.intensity, pattern, window, config)?;
    for (point, radius) in result.curve.iter().zip(config.radii_um()) {
        let available = point.status == InhomogeneousSpatialPointStatus::Available;
        if point.radius_um.to_bits() != radius.to_bits()
            || point.k.is_some() != available
            || point.l.is_some() != available
            || point.inverse_intensity_pair_sum < 0.0
            || !point.inverse_intensity_pair_sum.is_finite()
            || !point.eligible_center_inverse_intensity_sum.is_finite()
            || point
                .k
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point
                .l
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point.theoretical_k.to_bits() != (std::f64::consts::PI * radius * radius).to_bits()
            || point.theoretical_l.to_bits() != radius.to_bits()
            || point.lower_l.is_some_and(|value| !value.is_finite())
            || point.upper_l.is_some_and(|value| !value.is_finite())
            || point.inference_eligible != (point.lower_l.is_some() && point.upper_l.is_some())
        {
            return Err(invalid("K/L curve is inconsistent or non-finite"));
        }
        if let (Some(k), Some(l)) = (point.k, point.l) {
            let expected_l = (k / std::f64::consts::PI).sqrt();
            if l.to_bits().abs_diff(expected_l.to_bits()) > 1 {
                return Err(invalid("K and L disagree"));
            }
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
