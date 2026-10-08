use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};

use crate::{
    compartment_fragmentation, compartment_interface_workflow::partition_artifact,
    BinaryCompartmentPartition2D, CompartmentFragmentationResult, CompartmentFragmentationSummary,
};

const NODE_KIND: &str = "compartment_fragmentation";
const RESULT_KIND: &str = "application/vnd.marklab.compartment-fragmentation+json;version=1";
const POLICY: &[u8] =
    b"serial;exact-vector-polygon-components;component-area-entropy;perimeter-area";

/// Cache-addressed exact binary compartment fragmentation node.
pub struct CompartmentFragmentationAnalysisNode<'a> {
    spec: NodeSpec,
    partition: &'a BinaryCompartmentPartition2D,
    inputs: [ArtifactRef; 1],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> CompartmentFragmentationAnalysisNode<'a> {
    /// Bind the exact partition, role order, component geometry, and fixed formulas.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        partition: &'a BinaryCompartmentPartition2D,
    ) -> Result<Self, NodeError> {
        let partition_ref = partition_artifact(partition)?;
        project
            .register_reference(partition_ref.clone())
            .map_err(NodeError::input)?;
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            partition,
            inputs: [partition_ref],
            configuration_digest: partition.descriptor().logical_digest,
            implementation_identity: format!(
                "marklab/{};adapter=compartment-fragmentation-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for CompartmentFragmentationAnalysisNode<'_> {
    type Output = CompartmentFragmentationResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = partition_artifact(self.partition)?;
        self.inputs[0]
            .verify_identity(current.digest(), current.byte_len())
            .map_err(NodeError::input)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: POLICY,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        Ok(compartment_fragmentation(self.partition))
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

fn validate(
    result: &CompartmentFragmentationResult,
    partition: &BinaryCompartmentPartition2D,
) -> io::Result<()> {
    if result != &compartment_fragmentation(partition)
        || !valid_summary(&result.negative)
        || !valid_summary(&result.positive)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "compartment fragmentation result is inconsistent or non-finite",
        ));
    }
    Ok(())
}

fn valid_summary(value: &CompartmentFragmentationSummary) -> bool {
    value.component_count > 0
        && value.component_count == value.component_areas_um2.len()
        && value
            .component_areas_um2
            .iter()
            .all(|area| area.is_finite() && *area > 0.0)
        && value
            .component_areas_um2
            .windows(2)
            .all(|pair| pair[0] <= pair[1])
        && value.total_area_um2.is_finite()
        && value.total_area_um2 > 0.0
        && value.total_perimeter_um.is_finite()
        && value.total_perimeter_um > 0.0
        && value.largest_component_area_fraction.is_finite()
        && (0.0..=1.0).contains(&value.largest_component_area_fraction)
        && value.component_area_entropy_nats.is_finite()
        && value.component_area_entropy_nats >= 0.0
        && value.normalized_component_area_entropy.is_finite()
        && (0.0..=1.0).contains(&value.normalized_component_area_entropy)
        && value.perimeter_area_ratio_per_um.is_finite()
        && value.perimeter_area_ratio_per_um > 0.0
}
