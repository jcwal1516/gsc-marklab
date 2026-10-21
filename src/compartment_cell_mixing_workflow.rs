use crate::workflow_codec::invalid;
use crate::workflow_node::{NodeAnalysis, TypedNode};
use std::io;

use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::Serialize;

use crate::{
    compartment_cell_mixing::configuration_digest,
    compartment_interface_workflow::partition_artifact,
    compartments::{
        BinaryCompartmentPartition2D, CompartmentCellMixingConfig, CompartmentCellMixingResult,
    },
    marked_workflow::pattern_artifact,
    marks::{DeclaredScalarPatternInput, ScalarMarkId},
    scalar_mark::COMPARTMENT_MARK_ID,
};

const NODE_KIND: &str = "compartment_cell_mixing";
const CONFIG_KIND: &str = "application/vnd.marklab.compartment-cell-mixing-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.compartment-cell-mixing+json;version=1";
const POLICY: &[u8] = b"serial;exact-undirected-physical-radius-graph;binary-compartment-cross-edge-fraction;neighbor-label-entropy";

/// Cache-addressed typed compartment cell-mixing node.
pub struct CompartmentCellMixingAnalysisNode<'a> {
    node: TypedNode<CompartmentCellMixingAnalysis<'a>>,
}

struct CompartmentCellMixingAnalysis<'a> {
    input: &'a DeclaredScalarPatternInput<'a>,
    partition: &'a BinaryCompartmentPartition2D,
    config: &'a CompartmentCellMixingConfig,
}

impl<'a> CompartmentCellMixingAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        partition: &'a BinaryCompartmentPartition2D,
        config: &'a CompartmentCellMixingConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let partition_ref = partition_artifact(partition)?;
        let config_ref = config_artifact(input, partition, config)?;
        let inputs = [pattern, declared, partition_ref, config_ref.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        let node_spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let node_configuration_digest = config_ref.digest();
        let node_implementation_identity = format!(
            "marklab/{};adapter=compartment-cell-mixing-node-v1",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec: node_spec,
                analysis: CompartmentCellMixingAnalysis {
                    input,
                    partition,
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

impl NodeAnalysis for CompartmentCellMixingAnalysis<'_> {
    type Output = CompartmentCellMixingResult;
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
            partition_artifact(self.partition)?,
            config_artifact(self.input, self.partition, self.config)?,
        ];
        crate::workflow_node::verify_inputs(inputs, current)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::compartments::analyze_compartment_cell_mixing(
            self.input,
            self.partition,
            self.config,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.input, self.partition, self.config).map_err(NodeError::encoding)?;
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.partition, self.config).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(CompartmentCellMixingAnalysisNode<'_> => CompartmentCellMixingResult);

#[derive(Serialize)]
struct ConfigArtifact {
    radius_um: f64,
    limits: crate::compartments::CompartmentCellMixingLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    config: &CompartmentCellMixingConfig,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, partition, config).map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radius_um: config.radius_um(),
        limits: config.limits(),
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &CompartmentCellMixingResult,
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    config: &CompartmentCellMixingConfig,
) -> io::Result<()> {
    let descriptor = partition.descriptor();
    let expected_digest = configuration_digest(input, partition, config).map_err(invalid_owned)?;
    let mark_id = ScalarMarkId::new(COMPARTMENT_MARK_ID).map_err(invalid_owned)?;
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let levels = table
        .categorical_levels(&mark_id)
        .ok_or_else(|| invalid("typed compartment levels are absent"))?;
    let codes = table
        .categorical_values(&mark_id)
        .ok_or_else(|| invalid("typed compartment rows are absent"))?;
    if codes.len() < 2 {
        return Err(invalid("cell mixing requires both compartment rows"));
    }
    let negative_code = level_code(levels, &descriptor.negative_compartment_id)?;
    let positive_code = level_code(levels, &descriptor.positive_compartment_id)?;
    let negative_count = codes.iter().filter(|code| **code == negative_code).count();
    let positive_count = codes.iter().filter(|code| **code == positive_code).count();
    let expected_random = 2.0 * negative_count as f64 * positive_count as f64
        / (codes.len() as f64 * (codes.len() - 1) as f64);
    let doubled_edges = result
        .undirected_edge_count
        .checked_mul(2)
        .ok_or_else(|| invalid("mixing edge count overflow"))?;
    let expected_edge_entropy = binary_entropy(result.cross_compartment_edge_fraction);
    if result.case_id != input.pattern().meta.case_id
        || result.timepoint != input.pattern().meta.timepoint
        || result.mark_id != mark_id.as_str()
        || result.measurement_status
            != table
                .measurement_status(&mark_id)
                .ok_or_else(|| invalid("typed compartment status is absent"))?
        || result.coordinate_frame_id != descriptor.coordinate_frame_id.as_str()
        || result.partition_digest != descriptor.logical_digest.to_string()
        || result.radius_um.to_bits() != config.radius_um().to_bits()
        || result.configuration_digest != expected_digest.to_string()
        || result.point_count != input.pattern().len()
        || result.undirected_edge_count == 0
        || result.directed_pair_visits != doubled_edges
        || result.directed_pair_visits > config.limits().maximum_pair_visits
        || result.cross_compartment_edge_count > result.undirected_edge_count
        || result.cross_compartment_edge_fraction.to_bits()
            != (result.cross_compartment_edge_count as f64 / result.undirected_edge_count as f64)
                .to_bits()
        || result.random_label_cross_edge_expectation.to_bits() != expected_random.to_bits()
        || result.cross_edge_fraction_minus_expectation.to_bits()
            != (result.cross_compartment_edge_fraction - expected_random).to_bits()
        || result.negative.compartment_id != descriptor.negative_compartment_id
        || result.positive.compartment_id != descriptor.positive_compartment_id
        || result.negative.cell_count != negative_count
        || result.positive.cell_count != positive_count
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.limits != config.limits()
        || result.graph_digest.len() != 64
        || !finite_unit(result.cross_compartment_edge_fraction)
        || result.edge_type_entropy_nats.to_bits() != expected_edge_entropy.to_bits()
        || result.normalized_edge_type_entropy.to_bits()
            != (expected_edge_entropy / 2.0_f64.ln()).to_bits()
        || !finite_entropy(
            result.edge_type_entropy_nats,
            result.normalized_edge_type_entropy,
        )
        || !valid_summary(&result.negative)
        || !valid_summary(&result.positive)
    {
        return Err(invalid(
            "compartment cell-mixing result is inconsistent or non-finite",
        ));
    }
    let doubled_cross = result
        .negative
        .cross_compartment_neighbor_incidences
        .checked_mul(2)
        .ok_or_else(|| invalid("mixing incidence count overflow"))?;
    let incidence_total = result
        .negative
        .same_compartment_neighbor_incidences
        .checked_add(result.positive.same_compartment_neighbor_incidences)
        .and_then(|value| value.checked_add(doubled_cross))
        .ok_or_else(|| invalid("mixing incidence count overflow"))?;
    if result.negative.cross_compartment_neighbor_incidences
        != result.positive.cross_compartment_neighbor_incidences
        || result.negative.cross_compartment_neighbor_incidences
            != result.cross_compartment_edge_count
        || incidence_total != doubled_edges
    {
        return Err(invalid("mixing incidences disagree with graph edges"));
    }
    Ok(())
}

fn level_code(levels: &[String], expected: &str) -> io::Result<u32> {
    levels
        .iter()
        .position(|level| level == expected)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| invalid("partition level is absent"))
}

fn finite_unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn finite_entropy(value: f64, normalized: f64) -> bool {
    value.is_finite() && value >= 0.0 && normalized.is_finite() && (0.0..=1.0).contains(&normalized)
}

fn valid_summary(value: &crate::compartments::CompartmentCellMixingSummary) -> bool {
    let total = value
        .same_compartment_neighbor_incidences
        .checked_add(value.cross_compartment_neighbor_incidences);
    let Some(total) = total else {
        return false;
    };
    let cross_fraction = if total == 0 {
        0.0
    } else {
        value.cross_compartment_neighbor_incidences as f64 / total as f64
    };
    let expected_entropy = binary_entropy(cross_fraction);
    value.cell_count > 0
        && value.neighbor_label_entropy_nats.is_finite()
        && value.neighbor_label_entropy_nats >= 0.0
        && value.neighbor_label_entropy_nats.to_bits() == expected_entropy.to_bits()
        && value.normalized_neighbor_label_entropy.is_finite()
        && (0.0..=1.0).contains(&value.normalized_neighbor_label_entropy)
        && value.normalized_neighbor_label_entropy.to_bits()
            == (expected_entropy / 2.0_f64.ln()).to_bits()
}

fn binary_entropy(probability: f64) -> f64 {
    let mut value = 0.0;
    if probability > 0.0 {
        value -= probability * probability.ln();
    }
    if probability < 1.0 {
        let complement = 1.0 - probability;
        value -= complement * complement.ln();
    }
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
