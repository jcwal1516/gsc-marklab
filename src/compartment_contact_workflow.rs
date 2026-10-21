use crate::workflow_node::{NodeAnalysis, TypedNode};
use std::io;

use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};

use crate::{
    compartment_interface_workflow::partition_artifact,
    compartments::{
        compartment_contact_fractions, BinaryCompartmentPartition2D, CompartmentContactFraction,
        CompartmentContactResult,
    },
};

const NODE_KIND: &str = "compartment_contact_fraction";
const RESULT_KIND: &str = "application/vnd.marklab.compartment-contact-fraction+json;version=1";
const POLICY: &[u8] =
    b"serial;exact-aligned-binary-partition;shared-over-complete-compartment-boundary";

/// Cache-addressed exact binary compartment contact-fraction node.
pub struct CompartmentContactAnalysisNode<'a> {
    node: TypedNode<CompartmentContactAnalysis<'a>>,
}

struct CompartmentContactAnalysis<'a> {
    partition: &'a BinaryCompartmentPartition2D,
}

impl<'a> CompartmentContactAnalysisNode<'a> {
    /// Bind the exact partition, role order, and fixed denominator.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        partition: &'a BinaryCompartmentPartition2D,
    ) -> Result<Self, NodeError> {
        let partition_ref = partition_artifact(partition)?;
        project
            .register_reference(partition_ref.clone())
            .map_err(NodeError::input)?;
        let node_spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let node_inputs = [partition_ref];
        let node_configuration_digest = partition.descriptor().logical_digest;
        let node_implementation_identity = format!(
            "marklab/{};adapter=compartment-contact-fraction-node-v1",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec: node_spec,
                analysis: CompartmentContactAnalysis { partition },
                inputs: node_inputs,
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

impl NodeAnalysis for CompartmentContactAnalysis<'_> {
    type Output = CompartmentContactResult;
    type Inputs = [ArtifactRef; 1];

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let current = partition_artifact(self.partition)?;
        crate::workflow_node::verify_input(inputs, 0, &current)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        Ok(compartment_contact_fractions(self.partition))
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.partition).map_err(NodeError::encoding)?;
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate(&output, self.partition).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(CompartmentContactAnalysisNode<'_> => CompartmentContactResult);

fn validate(
    result: &CompartmentContactResult,
    partition: &BinaryCompartmentPartition2D,
) -> io::Result<()> {
    let expected = compartment_contact_fractions(partition);
    if result != &expected || !valid_contact(&result.negative) || !valid_contact(&result.positive) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "compartment contact result is inconsistent or non-finite",
        ));
    }
    Ok(())
}

fn valid_contact(value: &CompartmentContactFraction) -> bool {
    value.shared_interface_length_um.is_finite()
        && value.shared_interface_length_um > 0.0
        && value.outer_tissue_boundary_length_um.is_finite()
        && value.outer_tissue_boundary_length_um >= 0.0
        && value.denominator_boundary_length_um.is_finite()
        && value.denominator_boundary_length_um > 0.0
        && value.contact_fraction.is_finite()
        && (0.0..=1.0).contains(&value.contact_fraction)
}
