use crate::workflow_codec::invalid;
use crate::workflow_node::{NodeAnalysis, TypedNode};
use std::io;

use marklab_project::{ArtifactRef, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::Serialize;

use crate::{
    marked_workflow::pattern_artifact,
    marks::{
        DeclaredScalarPatternInput, ScalarMarkId, SoftClassCompositionLimits,
        SoftClassCompositionResult,
    },
    soft_class_composition::configuration_digest,
};

const NODE_KIND: &str = "soft_class_composition";
const CONFIG_KIND: &str = "application/vnd.marklab.soft-class-composition-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.soft-class-composition+json;version=1";
const POLICY: &[u8] = b"serial;complete-probability-simplex-rows;no-threshold;shannon-nats";

/// Cache-addressed soft class-composition node over one typed simplex column.
pub struct SoftClassCompositionAnalysisNode<'a> {
    node: TypedNode<SoftClassCompositionAnalysis<'a>>,
}

struct SoftClassCompositionAnalysis<'a> {
    input: &'a DeclaredScalarPatternInput<'a>,
    mark_id: &'a ScalarMarkId,
    limits: &'a SoftClassCompositionLimits,
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
        let node_spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let node_configuration_digest = config.digest();
        let node_implementation_identity = format!(
            "marklab/{};adapter=soft-class-composition-node-v1",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec: node_spec,
                analysis: SoftClassCompositionAnalysis {
                    input,
                    mark_id,
                    limits,
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

impl NodeAnalysis for SoftClassCompositionAnalysis<'_> {
    type Output = SoftClassCompositionResult;
    type Inputs = [ArtifactRef; 3];

    fn semantic_input_artifacts(&self) -> &[marklab_project::ArtifactId] {
        self.input.semantic_artifact_ids()
    }

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let current = [
            pattern_artifact(self.input.pattern())?,
            self.input
                .declared_artifact_ref()
                .map_err(NodeError::input)?,
            config_artifact(self.input, self.mark_id, self.limits)?,
        ];
        crate::workflow_node::verify_inputs(inputs, current)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::marks::soft_class_composition(self.input, self.mark_id, self.limits)
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

crate::workflow_node::impl_workflow_node!(SoftClassCompositionAnalysisNode<'_> => SoftClassCompositionResult);

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
            != table
                .measurement_status(mark_id)
                .ok_or_else(|| invalid("typed simplex status is absent"))?
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

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
