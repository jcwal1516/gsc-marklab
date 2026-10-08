use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    compartment_interface::analysis::measurement_status_name,
    soft_class_composition::configuration_digest, workflow::pattern_artifact,
    DeclaredScalarPatternInput, ScalarMarkId, SoftClassCompositionLimits,
    SoftClassCompositionResult,
};

const NODE_KIND: &str = "soft_class_composition";
const CONFIG_KIND: &str = "application/vnd.marklab.soft-class-composition-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.soft-class-composition+json;version=1";
const POLICY: &[u8] = b"serial;complete-probability-simplex-rows;no-threshold;shannon-nats";

/// Cache-addressed soft class-composition node over one typed simplex column.
pub struct SoftClassCompositionAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    mark_id: &'a ScalarMarkId,
    limits: &'a SoftClassCompositionLimits,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> SoftClassCompositionAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        mark_id: &'a ScalarMarkId,
        limits: &'a SoftClassCompositionLimits,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let config = config_artifact(input, mark_id, limits)?;
        let inputs = [pattern, declared, config.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            input,
            mark_id,
            limits,
            inputs,
            configuration_digest: config.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=soft-class-composition-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for SoftClassCompositionAnalysisNode<'_> {
    type Output = SoftClassCompositionResult;

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
            config_artifact(self.input, self.mark_id, self.limits)?,
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
        crate::soft_class_composition(self.input, self.mark_id, self.limits)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.input, self.mark_id, self.limits).map_err(NodeError::encoding)?;
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.mark_id, self.limits).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    limits: SoftClassCompositionLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    limits: &SoftClassCompositionLimits,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, mark_id, limits).map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        mark_id: mark_id.as_str(),
        limits: *limits,
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &SoftClassCompositionResult,
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    limits: &SoftClassCompositionLimits,
) -> io::Result<()> {
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let levels = table
        .probability_simplex_levels(mark_id)
        .ok_or_else(|| invalid("typed probability simplex is absent"))?;
    let values = table
        .probability_simplex_values(mark_id)
        .ok_or_else(|| invalid("typed probability simplex values are absent"))?;
    let expected_digest = configuration_digest(input, mark_id, limits).map_err(invalid_owned)?;
    let expected_value_count = result
        .row_count
        .checked_mul(result.class_count)
        .ok_or_else(|| invalid("soft composition value count overflow"))?;
    let mean_sum = result
        .classes
        .iter()
        .map(|class| class.mean_probability)
        .sum::<f64>();
    if result.case_id != input.pattern().meta.case_id
        || result.timepoint != input.pattern().meta.timepoint
        || result.mark_id != mark_id.as_str()
        || result.measurement_status
            != measurement_status_name(
                table
                    .measurement_status(mark_id)
                    .ok_or_else(|| invalid("typed simplex status is absent"))?,
            )
        || result.row_count != input.pattern().len()
        || result.class_count != levels.len()
        || result.row_count == 0
        || result.class_count < 2
        || values.len() != expected_value_count
        || result.classes.len() != levels.len()
        || result
            .classes
            .iter()
            .zip(levels)
            .any(|(class, level)| class.class_id != *level || !finite_unit(class.mean_probability))
        || result.configuration_digest != expected_digest.to_string()
        || result.estimated_storage_bytes > limits.maximum_retained_bytes
        || result.limits != *limits
        || !result.mean_row_entropy_nats.is_finite()
        || result.mean_row_entropy_nats < 0.0
        || !result.aggregate_composition_entropy_nats.is_finite()
        || result.aggregate_composition_entropy_nats < 0.0
        || result.effective_class_count.to_bits()
            != result.aggregate_composition_entropy_nats.exp().to_bits()
        || !result.effective_class_count.is_finite()
        || result.effective_class_count < 1.0
        || result.effective_class_count > result.class_count as f64
        || !mean_sum.is_finite()
        || (mean_sum - 1.0).abs() > 1e-5
        || !result.maximum_row_sum_absolute_error.is_finite()
        || result.maximum_row_sum_absolute_error > 1e-5
    {
        return Err(invalid(
            "soft class-composition result is inconsistent or non-finite",
        ));
    }
    Ok(())
}

fn finite_unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
