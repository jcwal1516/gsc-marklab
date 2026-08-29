use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{workflow::pattern_artifact, ObservationWindow2D, Pattern};

use super::{
    bandwidth::{
        bandwidth_selection_configuration_digest, candidate_config, selected_retained_bytes,
        selection_digest,
    },
    bandwidth_types::{
        GaussianBandwidthSelectionConfig, GaussianBandwidthSelectionLimits,
        SelectedInhomogeneousSpatialResult,
    },
    workflow::{validate as validate_analysis, window_artifact},
};

const NODE_KIND: &str = "gaussian_bandwidth_selected_spatial_analysis";
const CONFIG_KIND: &str =
    "application/vnd.marklab.gaussian-bandwidth-selection-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.gaussian-bandwidth-selected-spatial-result+json;version=1";
const POLICY: &[u8] = b"serial;prespecified-increasing-bandwidth-candidates;mean-leave-one-out-log-event-intensity;smallest-exact-tie;selection-independent-of-spatial-curve;selected-gaussian-kl-v1";

/// Durable typed node for prespecified Gaussian bandwidth selection followed by K/L.
pub struct GaussianBandwidthSelectedSpatialAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a GaussianBandwidthSelectionConfig,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> GaussianBandwidthSelectedSpatialAnalysisNode<'a> {
    /// Bind the exact pattern, window, candidate score contract, downstream controls, and limits.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a GaussianBandwidthSelectionConfig,
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
                "marklab/{};adapter=gaussian-bandwidth-selected-spatial-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Bound workflow specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for GaussianBandwidthSelectedSpatialAnalysisNode<'_> {
    type Output = SelectedInhomogeneousSpatialResult;

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
        super::analyze_selected_inhomogeneous_spatial_pattern(
            self.pattern,
            self.window,
            self.config,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_selected_spatial_result(output, self.pattern, self.window, self.config)
            .map_err(NodeError::encoding)
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

pub(crate) fn encode_selected_spatial_result(
    output: &SelectedInhomogeneousSpatialResult,
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &GaussianBandwidthSelectionConfig,
) -> io::Result<Box<[u8]>> {
    validate(output, pattern, window, config)?;
    crate::exact_float_json::encode(output)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    candidate_bandwidths_um: &'a [f64],
    integration_grid: [usize; 2],
    simulations: usize,
    seed: u64,
    alpha: f64,
    minimum_intensity_per_um2: f64,
    analysis_limits: super::InhomogeneousSpatialLimits,
    selection_limits: GaussianBandwidthSelectionLimits,
    logical_digest: String,
}

fn config_artifact(config: &GaussianBandwidthSelectionConfig) -> Result<ArtifactRef, NodeError> {
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: &config.radii_um,
        candidate_bandwidths_um: &config.candidate_bandwidths_um,
        integration_grid: config.integration_grid,
        simulations: config.simulations,
        seed: config.seed,
        alpha: config.alpha,
        minimum_intensity_per_um2: config.minimum_intensity_per_um2,
        analysis_limits: config.analysis_limits,
        selection_limits: config.selection_limits,
        logical_digest: bandwidth_selection_configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &SelectedInhomogeneousSpatialResult,
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &GaussianBandwidthSelectionConfig,
) -> io::Result<()> {
    let selection = &result.selection;
    if result.configuration_digest != bandwidth_selection_configuration_digest(config).to_string()
        || result.selection_limits != config.selection_limits
        || selection.method != "leave_one_out_log_likelihood"
        || selection.objective != "maximize_mean_log_leave_one_out_event_intensity"
        || selection.tie_break != "smallest_bandwidth_um"
        || selection.selection_uses_spatial_curve
        || selection.candidates.len() != config.candidate_bandwidths_um.len()
        || selection.selected_index >= selection.candidates.len()
        || selection.intensity_evaluations > config.selection_limits.maximum_intensity_evaluations
        || result.estimated_storage_bytes > config.analysis_limits.maximum_retained_bytes
    {
        return Err(invalid(
            "selected result does not match its cache-bound request",
        ));
    }
    let mut evaluation_sum = 0_usize;
    let mut best_index = 0_usize;
    let mut best_score = f64::NEG_INFINITY;
    for (index, (candidate, bandwidth)) in selection
        .candidates
        .iter()
        .zip(&config.candidate_bandwidths_um)
        .enumerate()
    {
        if candidate.bandwidth_um.to_bits() != bandwidth.to_bits()
            || !candidate.mean_log_leave_one_out_intensity.is_finite()
            || !candidate.minimum_intensity_per_um2.is_finite()
            || candidate.minimum_intensity_per_um2 <= 0.0
            || !candidate.maximum_intensity_per_um2.is_finite()
            || candidate.maximum_intensity_per_um2 < candidate.minimum_intensity_per_um2
            || candidate.intensity_evaluations == 0
        {
            return Err(invalid("bandwidth candidate score is inconsistent"));
        }
        evaluation_sum = evaluation_sum
            .checked_add(candidate.intensity_evaluations)
            .ok_or_else(|| invalid("selection evaluation count overflow"))?;
        if candidate.mean_log_leave_one_out_intensity > best_score {
            best_index = index;
            best_score = candidate.mean_log_leave_one_out_intensity;
        }
    }
    if selection.selected_index != best_index
        || selection.selected_bandwidth_um.to_bits()
            != config.candidate_bandwidths_um[best_index].to_bits()
        || selection.selected_bandwidth_um.to_bits()
            != result.analysis.intensity.bandwidth_um.to_bits()
        || selection.intensity_evaluations != evaluation_sum
        || selection.artifact_digest
            != selection_digest(
                pattern,
                window,
                config,
                &selection.candidates,
                selection.selected_index,
            )
            .to_string()
        || result.estimated_storage_bytes
            != selected_retained_bytes(&result.analysis, selection.candidates.len())
                .map_err(|_| invalid("selected retained-byte estimate overflow"))?
    {
        return Err(invalid("bandwidth selection identity is inconsistent"));
    }
    let selected_config = candidate_config(
        config,
        selection.selected_bandwidth_um,
        config.analysis_limits,
    )
    .map_err(|_| invalid("selected analysis config is invalid"))?;
    validate_analysis(&result.analysis, pattern, window, &selected_config)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
