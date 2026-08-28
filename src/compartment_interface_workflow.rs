use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    compartment_interface::analysis::{
        configuration_digest, measurement_status_name, retained_bytes,
    },
    workflow::pattern_artifact,
    BinaryCompartmentPartition2D, CompartmentInterfaceLimits, CompartmentInterfaceProfile,
    CompartmentInterfaceSummary, DeclaredScalarPatternInput, ScalarMarkId,
};

const NODE_KIND: &str = "compartment_interface_profile";
const PARTITION_KIND: &str =
    "application/vnd.marklab.binary-compartment-partition-ref+json;version=1";
const CONFIG_KIND: &str = "application/vnd.marklab.compartment-interface-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.compartment-interface-profile+json;version=1";
const POLICY: &[u8] =
    b"serial;exact-aligned-binary-partition;oriented-shared-interface-distance;typed-cell-rows";

/// Cache-addressed typed per-cell compartment-interface profile.
pub struct CompartmentInterfaceAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    partition: &'a BinaryCompartmentPartition2D,
    limits: &'a CompartmentInterfaceLimits,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> CompartmentInterfaceAnalysisNode<'a> {
    /// Bind the exact typed marks, partition, orientation, frame, and resource ceilings.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        partition: &'a BinaryCompartmentPartition2D,
        limits: &'a CompartmentInterfaceLimits,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let partition_ref = partition_artifact(partition)?;
        let config_ref = config_artifact(input, partition, limits)?;
        let inputs = [pattern, declared, partition_ref, config_ref.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            input,
            partition,
            limits,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=compartment-interface-profile-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for CompartmentInterfaceAnalysisNode<'_> {
    type Output = CompartmentInterfaceProfile;

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
            partition_artifact(self.partition)?,
            config_artifact(self.input, self.partition, self.limits)?,
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
        crate::analyze_compartment_interface_profile(self.input, self.partition, self.limits)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.input, self.partition, self.limits).map_err(NodeError::encoding)?;
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.partition, self.limits).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct PartitionArtifact<'a> {
    logical_digest: String,
    negative_compartment_id: &'a str,
    positive_compartment_id: &'a str,
    coordinate_frame_id: &'a str,
    observation_area_um2: f64,
    negative_area_um2: f64,
    positive_area_um2: f64,
    interface_segment_count: usize,
    interface_length_um: f64,
    validated_boundary_segment_count: usize,
}

fn partition_artifact(partition: &BinaryCompartmentPartition2D) -> Result<ArtifactRef, NodeError> {
    let descriptor = partition.descriptor();
    let bytes = serde_json::to_vec(&PartitionArtifact {
        logical_digest: descriptor.logical_digest.to_string(),
        negative_compartment_id: &descriptor.negative_compartment_id,
        positive_compartment_id: &descriptor.positive_compartment_id,
        coordinate_frame_id: descriptor.coordinate_frame_id.as_str(),
        observation_area_um2: descriptor.observation_area_um2,
        negative_area_um2: descriptor.negative_area_um2,
        positive_area_um2: descriptor.positive_area_um2,
        interface_segment_count: descriptor.interface_segment_count,
        interface_length_um: descriptor.interface_length_um,
        validated_boundary_segment_count: descriptor.validated_boundary_segment_count,
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(PARTITION_KIND, &bytes).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact {
    limits: CompartmentInterfaceLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    limits: &CompartmentInterfaceLimits,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, partition, limits).map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        limits: *limits,
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &CompartmentInterfaceProfile,
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    limits: &CompartmentInterfaceLimits,
) -> io::Result<()> {
    let descriptor = partition.descriptor();
    let expected_digest = configuration_digest(input, partition, limits).map_err(invalid_owned)?;
    let expected_bytes = retained_bytes(
        input,
        &descriptor.negative_compartment_id,
        &descriptor.positive_compartment_id,
    )
    .map_err(invalid_owned)?;
    let mark_id = ScalarMarkId::new("histologic_compartment").map_err(invalid_owned)?;
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let expected_status = table
        .measurement_status(&mark_id)
        .ok_or_else(|| invalid("typed compartment mark is absent"))?;
    let codes = table
        .categorical_values(&mark_id)
        .ok_or_else(|| invalid("typed compartment rows are absent"))?;
    let levels = table
        .categorical_levels(&mark_id)
        .ok_or_else(|| invalid("typed compartment levels are absent"))?;
    if result.case_id != input.pattern().meta.case_id
        || result.timepoint != input.pattern().meta.timepoint
        || result.mark_id != mark_id.as_str()
        || result.measurement_status != measurement_status_name(expected_status)
        || result.negative_compartment_id != descriptor.negative_compartment_id
        || result.positive_compartment_id != descriptor.positive_compartment_id
        || result.coordinate_frame_id != descriptor.coordinate_frame_id.as_str()
        || result.interface_length_um.to_bits() != descriptor.interface_length_um.to_bits()
        || result.partition_digest != descriptor.logical_digest.to_string()
        || result.configuration_digest != expected_digest.to_string()
        || result.query_count != input.pattern().len()
        || result.estimated_storage_bytes != expected_bytes
        || result.limits != *limits
        || result.rows.len() != input.pattern().len()
    {
        return Err(invalid("result does not match its cache-bound request"));
    }
    let mut summaries = [
        SummaryCheck::new(&descriptor.negative_compartment_id),
        SummaryCheck::new(&descriptor.positive_compartment_id),
    ];
    for (row, value) in result.rows.iter().enumerate() {
        let level = usize::try_from(codes[row])
            .ok()
            .and_then(|code| levels.get(code))
            .ok_or_else(|| invalid("typed compartment code is invalid"))?;
        let summary = if level == &descriptor.negative_compartment_id {
            if value.signed_interface_distance_um > 0.0 {
                return Err(invalid("negative compartment distance has positive sign"));
            }
            &mut summaries[0]
        } else if level == &descriptor.positive_compartment_id {
            if value.signed_interface_distance_um < 0.0 {
                return Err(invalid("positive compartment distance has negative sign"));
            }
            &mut summaries[1]
        } else {
            return Err(invalid("result contains an unsupported compartment"));
        };
        if value.row != row
            || value.cell_id != input.cell_ids()[row].as_str()
            || value.compartment_id != *level
            || !value.signed_interface_distance_um.is_finite()
        {
            return Err(invalid("result cell row is inconsistent or non-finite"));
        }
        summary.push(value.signed_interface_distance_um)?;
    }
    for (observed, expected) in result.summaries.iter().zip(summaries) {
        if !expected.matches(observed) {
            return Err(invalid("result compartment summary is inconsistent"));
        }
    }
    Ok(())
}

struct SummaryCheck<'a> {
    id: &'a str,
    count: usize,
    minimum: f64,
    maximum: f64,
    absolute_sum: f64,
    correction: f64,
}

impl<'a> SummaryCheck<'a> {
    fn new(id: &'a str) -> Self {
        Self {
            id,
            count: 0,
            minimum: f64::INFINITY,
            maximum: f64::NEG_INFINITY,
            absolute_sum: 0.0,
            correction: 0.0,
        }
    }

    fn push(&mut self, value: f64) -> io::Result<()> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| invalid("summary count overflow"))?;
        self.minimum = self.minimum.min(value);
        self.maximum = self.maximum.max(value);
        let corrected = value.abs() - self.correction;
        let next = self.absolute_sum + corrected;
        self.correction = (next - self.absolute_sum) - corrected;
        self.absolute_sum = next;
        Ok(())
    }

    fn matches(&self, value: &CompartmentInterfaceSummary) -> bool {
        self.count > 0
            && value.compartment_id == self.id
            && value.cell_count == self.count
            && value.minimum_signed_distance_um.to_bits() == self.minimum.to_bits()
            && value.maximum_signed_distance_um.to_bits() == self.maximum.to_bits()
            && value.mean_absolute_distance_um.to_bits()
                == ((self.absolute_sum + self.correction) / self.count as f64).to_bits()
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
