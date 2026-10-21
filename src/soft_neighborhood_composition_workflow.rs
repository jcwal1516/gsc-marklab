use crate::workflow_codec::invalid;
use crate::workflow_node::{NodeAnalysis, TypedNode};
use std::io;

use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::Serialize;

use crate::observation_window_artifact::frame_bound_window_artifact as window_artifact;
use crate::{
    marked_workflow::pattern_artifact,
    marks::{
        DeclaredScalarPatternInput, ScalarMarkId, SoftNeighborhoodCompositionConfig,
        SoftNeighborhoodCompositionResult,
    },
    soft_neighborhood_composition::configuration_digest,
    ObservationWindow2D,
};

const NODE_KIND: &str = "soft_neighborhood_composition";
const CONFIG_KIND: &str =
    "application/vnd.marklab.soft-neighborhood-composition-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.soft-neighborhood-composition+json;version=1";
const POLICY: &[u8] = b"serial;exact-undirected-physical-radius-graph;complete-simplex-target-rows;typed-zero-neighbor";

pub struct SoftNeighborhoodCompositionAnalysisNode<'a> {
    node: TypedNode<SoftNeighborhoodCompositionAnalysis<'a>>,
}

struct SoftNeighborhoodCompositionAnalysis<'a> {
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    mark_id: &'a ScalarMarkId,
    config: &'a SoftNeighborhoodCompositionConfig,
}

impl<'a> SoftNeighborhoodCompositionAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        mark_id: &'a ScalarMarkId,
        config: &'a SoftNeighborhoodCompositionConfig,
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
        let node_spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let node_configuration_digest = config_ref.digest();
        let node_implementation_identity = format!(
            "marklab/{};adapter=soft-neighborhood-composition-node-v1",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec: node_spec,
                analysis: SoftNeighborhoodCompositionAnalysis {
                    input,
                    window,
                    mark_id,
                    config,
                },
                inputs,
                configuration_digest: node_configuration_digest,
                execution_policy: std::borrow::Cow::Borrowed(POLICY),
                implementation_identity: node_implementation_identity,
            },
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for SoftNeighborhoodCompositionAnalysis<'_> {
    type Output = SoftNeighborhoodCompositionResult;
    type Inputs = [ArtifactRef; 4];

    fn semantic_input_artifacts(&self) -> &[marklab_project::ArtifactId] {
        self.input.semantic_artifact_ids()
    }

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let current = [
            pattern_artifact(self.input.pattern())?,
            self.input
                .declared_artifact_ref()
                .map_err(NodeError::input)?,
            window_artifact(self.window)?,
            config_artifact(self.input, self.window, self.mark_id, self.config)?,
        ];
        crate::workflow_node::verify_inputs(inputs, current)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::marks::soft_neighborhood_composition(
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

crate::workflow_node::impl_workflow_node!(SoftNeighborhoodCompositionAnalysisNode<'_> => SoftNeighborhoodCompositionResult);

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    radius_um: f64,
    limits: crate::marks::SoftNeighborhoodCompositionLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftNeighborhoodCompositionConfig,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, window, mark_id, config).map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        mark_id: mark_id.as_str(),
        radius_um: config.radius_um(),
        limits: config.limits(),
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &SoftNeighborhoodCompositionResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftNeighborhoodCompositionConfig,
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
            != table
                .measurement_status(mark_id)
                .ok_or_else(|| invalid("simplex status is absent"))?
        || result.coordinate_frame_id != input.coordinate_frame_id().as_str()
        || result.window_digest != window.descriptor().logical_digest.to_string()
        || result.radius_um.to_bits() != config.radius_um().to_bits()
        || result.class_ids != levels
        || result.configuration_digest != expected_digest.to_string()
        || result.graph_digest.len() != 64
        || result.rows.len() != input.pattern().len()
        || result.directed_pair_visits > config.limits().maximum_pair_visits
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.limits != config.limits()
    {
        return Err(invalid(
            "soft neighborhood result does not match its request",
        ));
    }
    let mut visits = 0_usize;
    let mut zeros = 0_usize;
    for (row, value) in result.rows.iter().enumerate() {
        visits = visits
            .checked_add(value.neighbor_count)
            .ok_or_else(|| invalid("soft neighborhood visit count overflow"))?;
        if value.neighbor_count == 0 {
            zeros = zeros
                .checked_add(1)
                .ok_or_else(|| invalid("soft neighborhood zero count overflow"))?;
        }
        if value.row != row
            || value.cell_id != input.cell_ids()[row].as_str()
            || value.mean_neighbor_probabilities.is_some() != (value.neighbor_count > 0)
            || value
                .mean_neighbor_probabilities
                .as_ref()
                .is_some_and(|values| !valid_probability_vector(values, levels.len()))
        {
            return Err(invalid("soft neighborhood row is inconsistent"));
        }
    }
    if visits != result.directed_pair_visits
        || zeros != result.zero_neighbor_cell_count
        || result.mean_neighbor_class_mass.is_some() != (visits > 0)
        || result
            .mean_neighbor_class_mass
            .as_ref()
            .is_some_and(|values| !valid_probability_vector(values, levels.len()))
    {
        return Err(invalid("soft neighborhood aggregate is inconsistent"));
    }
    Ok(())
}

fn valid_probability_vector(values: &[f64], classes: usize) -> bool {
    values.len() == classes
        && values
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        && (values.iter().sum::<f64>() - 1.0).abs() <= 1e-5
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
