use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    compartment_interface::analysis::measurement_status_name,
    soft_multiscale_neighborhood::configuration_digest, workflow::pattern_artifact,
    DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
    SoftMultiscaleNeighborhoodConfig, SoftMultiscaleNeighborhoodResult,
};

const NODE_KIND: &str = "soft_multiscale_neighborhood_composition";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str =
    "application/vnd.marklab.soft-multiscale-neighborhood-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.soft-multiscale-neighborhood+json;version=1";
const POLICY: &[u8] = b"serial;one-exact-geometry-plan;strict-increasing-physical-radii;complete-simplex-target-rows;adjacent-scale-tv";

/// Store-aware durable execution node for prespecified multiscale soft neighborhoods.
pub struct SoftMultiscaleNeighborhoodAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    mark_id: &'a ScalarMarkId,
    config: &'a SoftMultiscaleNeighborhoodConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> SoftMultiscaleNeighborhoodAnalysisNode<'a> {
    /// Bind typed input, window, simplex mark, radius schedule, and limits to one node identity.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        mark_id: &'a ScalarMarkId,
        config: &'a SoftMultiscaleNeighborhoodConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(input, window, mark_id, config)?;
        let inputs = [pattern, declared, window_ref, config_ref.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            input,
            window,
            mark_id,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=soft-multiscale-neighborhood-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Return the immutable workflow-node specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for SoftMultiscaleNeighborhoodAnalysisNode<'_> {
    type Output = SoftMultiscaleNeighborhoodResult;

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
            window_artifact(self.window)?,
            config_artifact(self.input, self.window, self.mark_id, self.config)?,
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
        crate::soft_multiscale_neighborhood_composition(
            self.input,
            self.window,
            self.mark_id,
            self.config,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.input, self.window, self.mark_id, self.config)
            .map_err(NodeError::encoding)?;
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.window, self.mark_id, self.config)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct WindowArtifact<'a> {
    logical_digest: String,
    coordinate_frame_id: &'a str,
}

fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or_else(|| NodeError::input(invalid("observation window is not frame-bound")))?;
    let bytes = serde_json::to_vec(&WindowArtifact {
        logical_digest: window.descriptor().logical_digest.to_string(),
        coordinate_frame_id: frame.as_str(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(WINDOW_KIND, &bytes).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    radii_um: &'a [f64],
    limits: crate::SoftMultiscaleNeighborhoodLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftMultiscaleNeighborhoodConfig,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, window, mark_id, config).map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        mark_id: mark_id.as_str(),
        radii_um: config.radii_um(),
        limits: config.limits(),
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &SoftMultiscaleNeighborhoodResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftMultiscaleNeighborhoodConfig,
) -> io::Result<()> {
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let levels = table
        .probability_simplex_levels(mark_id)
        .ok_or_else(|| invalid("typed probability simplex is absent"))?;
    let expected_digest =
        configuration_digest(input, window, mark_id, config).map_err(invalid_owned)?;
    if result.case_id != input.pattern().meta.case_id
        || result.timepoint != input.pattern().meta.timepoint
        || result.mark_id != mark_id.as_str()
        || result.measurement_status
            != measurement_status_name(
                table
                    .measurement_status(mark_id)
                    .ok_or_else(|| invalid("simplex status is absent"))?,
            )
        || result.coordinate_frame_id != input.coordinate_frame_id().as_str()
        || result.window_digest != window.descriptor().logical_digest.to_string()
        || result.class_ids != levels
        || result.configuration_digest != expected_digest.to_string()
        || result.geometry_build_count != 1
        || result.scales.len() != config.radii_um().len()
        || result.adjacent_scale_total_variation_distance.len()
            != result.scales.len().saturating_sub(1)
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.limits != config.limits()
    {
        return Err(invalid("soft multiscale result does not match its request"));
    }
    let mut total_visits = 0_usize;
    for (scale, radius) in result.scales.iter().zip(config.radii_um()) {
        let mut scale_visits = 0_usize;
        let mut scale_zeros = 0_usize;
        for (row, value) in scale.rows.iter().enumerate() {
            scale_visits = scale_visits
                .checked_add(value.neighbor_count)
                .ok_or_else(|| invalid("soft multiscale scale visit count overflow"))?;
            if value.neighbor_count == 0 {
                scale_zeros = scale_zeros
                    .checked_add(1)
                    .ok_or_else(|| invalid("soft multiscale zero count overflow"))?;
            }
            if value.row != row
                || value.cell_id != input.cell_ids()[row].as_str()
                || value.mean_neighbor_probabilities.is_some() != (value.neighbor_count > 0)
                || value
                    .mean_neighbor_probabilities
                    .as_ref()
                    .is_some_and(|values| !valid_vector(values, levels.len()))
            {
                return Err(invalid("soft multiscale row is inconsistent"));
            }
        }
        total_visits = total_visits
            .checked_add(scale.directed_pair_visits)
            .ok_or_else(|| invalid("soft multiscale visit count overflow"))?;
        if scale.radius_um.to_bits() != radius.to_bits()
            || scale.rows.len() != input.pattern().len()
            || scale.graph_digest.len() != 64
            || scale.directed_pair_visits != scale_visits
            || scale.zero_neighbor_cell_count != scale_zeros
            || scale.mean_neighbor_class_mass.is_some() != (scale.directed_pair_visits > 0)
            || scale
                .mean_neighbor_class_mass
                .as_ref()
                .is_some_and(|values| !valid_vector(values, levels.len()))
        {
            return Err(invalid("soft multiscale scale is inconsistent"));
        }
    }
    if total_visits != result.total_directed_pair_visits
        || total_visits > config.limits().maximum_pair_visits
        || result
            .scales
            .windows(2)
            .zip(&result.adjacent_scale_total_variation_distance)
            .any(|(scales, observed)| {
                let expected = match (
                    scales[0].mean_neighbor_class_mass.as_deref(),
                    scales[1].mean_neighbor_class_mass.as_deref(),
                ) {
                    (Some(left), Some(right)) => Some(
                        0.5 * left
                            .iter()
                            .zip(right)
                            .map(|(left, right)| (left - right).abs())
                            .sum::<f64>(),
                    ),
                    _ => None,
                };
                match (observed, expected) {
                    (Some(observed), Some(expected)) => {
                        !observed.is_finite()
                            || !(0.0..=1.0).contains(observed)
                            || observed.to_bits() != expected.to_bits()
                    }
                    (None, None) => false,
                    _ => true,
                }
            })
    {
        return Err(invalid("soft multiscale aggregate is inconsistent"));
    }
    Ok(())
}

fn valid_vector(values: &[f64], classes: usize) -> bool {
    values.len() == classes
        && values
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        && (values.iter().sum::<f64>() - 1.0).abs() <= 1e-5
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
