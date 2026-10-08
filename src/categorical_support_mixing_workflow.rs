use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::observation_window_artifact::frame_bound_window_artifact as window_artifact;
use crate::{
    categorical_support_mixing::{configuration_digest, CATEGORICAL_SUPPORT_SEMANTICS},
    compartment_interface::analysis::measurement_status_name,
    workflow::pattern_artifact,
    CategoricalSupportMixingConfig, CategoricalSupportMixingResult, DeclaredScalarPatternInput,
    ObservationWindow2D, ScalarMarkId,
};

const NODE_KIND: &str = "categorical_support_mixing";
const CONFIG_KIND: &str =
    "application/vnd.marklab.categorical-support-mixing-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.categorical-support-mixing+json;version=1";
const POLICY: &[u8] = b"serial;exact-physical-radius-graph;fixed-hard-labels;winner-type-pixel-support-product-sensitivity;not-class-posterior";

/// Cache-addressed hard-label mixing node with continuous pixel-support sensitivity weights.
pub struct CategoricalSupportMixingAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    categorical_mark_id: &'a ScalarMarkId,
    support_mark_id: &'a ScalarMarkId,
    config: &'a CategoricalSupportMixingConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> CategoricalSupportMixingAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        categorical_mark_id: &'a ScalarMarkId,
        support_mark_id: &'a ScalarMarkId,
        config: &'a CategoricalSupportMixingConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let window_ref = window_artifact(window)?;
        let config_ref =
            config_artifact(input, window, categorical_mark_id, support_mark_id, config)?;
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
            categorical_mark_id,
            support_mark_id,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=categorical-support-mixing-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for CategoricalSupportMixingAnalysisNode<'_> {
    type Output = CategoricalSupportMixingResult;

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
            config_artifact(
                self.input,
                self.window,
                self.categorical_mark_id,
                self.support_mark_id,
                self.config,
            )?,
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
        crate::categorical_support_mixing(
            self.input,
            self.window,
            self.categorical_mark_id,
            self.support_mark_id,
            self.config,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(
            output,
            self.input,
            self.window,
            self.categorical_mark_id,
            self.support_mark_id,
            self.config,
        )
        .map_err(NodeError::encoding)?;
        crate::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = crate::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        validate(
            &output,
            self.input,
            self.window,
            self.categorical_mark_id,
            self.support_mark_id,
            self.config,
        )
        .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    categorical_mark_id: &'a str,
    support_mark_id: &'a str,
    radius_um: f64,
    limits: crate::CategoricalSupportMixingLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    support_mark_id: &ScalarMarkId,
    config: &CategoricalSupportMixingConfig,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, window, categorical_mark_id, support_mark_id, config)
        .map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        categorical_mark_id: categorical_mark_id.as_str(),
        support_mark_id: support_mark_id.as_str(),
        radius_um: config.radius_um(),
        limits: config.limits(),
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &CategoricalSupportMixingResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    support_mark_id: &ScalarMarkId,
    config: &CategoricalSupportMixingConfig,
) -> io::Result<()> {
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let levels = table
        .categorical_levels(categorical_mark_id)
        .ok_or_else(|| invalid("categorical levels are absent"))?;
    let categorical_status = table
        .measurement_status(categorical_mark_id)
        .ok_or_else(|| invalid("categorical measurement status is absent"))?;
    let support_status = table
        .measurement_status(support_mark_id)
        .ok_or_else(|| invalid("support measurement status is absent"))?;
    let expected =
        configuration_digest(input, window, categorical_mark_id, support_mark_id, config)
            .map_err(invalid_owned)?;
    let matrix_len = levels
        .len()
        .checked_mul(levels.len())
        .ok_or_else(|| invalid("matrix size overflow"))?;
    if result.case_id != input.pattern().meta.case_id
        || result.timepoint != input.pattern().meta.timepoint
        || result.categorical_mark_id != categorical_mark_id.as_str()
        || result.support_mark_id != support_mark_id.as_str()
        || result.categorical_measurement_status != measurement_status_name(categorical_status)
        || result.support_measurement_status != measurement_status_name(support_status)
        || result.coordinate_frame_id != input.coordinate_frame_id().as_str()
        || result.window_digest != window.descriptor().logical_digest.to_string()
        || result.radius_um.to_bits() != config.radius_um().to_bits()
        || result.class_ids != levels
        || result.support_semantics != CATEGORICAL_SUPPORT_SEMANTICS
        || result.configuration_digest != expected.to_string()
        || result.graph_digest.len() != 64
        || result.point_count != input.pattern().len()
        || result.directed_pair_visits == 0
        || result.directed_pair_visits > config.limits().maximum_pair_visits
        || result.matrix.len() != matrix_len
        || !result.total_support_weighted_pair_mass.is_finite()
        || result.total_support_weighted_pair_mass < 0.0
        || result.total_support_weighted_pair_mass > result.directed_pair_visits as f64
        || result.total_support_retention_fraction.to_bits()
            != (result.total_support_weighted_pair_mass / result.directed_pair_visits as f64)
                .to_bits()
        || result.total_support_retention_fraction > 1.0
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.limits != config.limits()
    {
        return Err(invalid(
            "categorical support mixing does not match its request",
        ));
    }
    let mut hard_count = 0_usize;
    let mut support_mass = 0.0_f64;
    let mut support_correction = 0.0_f64;
    for (index, cell) in result.matrix.iter().enumerate() {
        let source = index / levels.len();
        let target = index % levels.len();
        let expected_mean = (cell.hard_directed_pair_count > 0)
            .then_some(cell.support_weighted_pair_mass / cell.hard_directed_pair_count as f64);
        if cell.source_class_index != source
            || cell.target_class_index != target
            || cell.source_class_id != levels[source]
            || cell.target_class_id != levels[target]
            || cell.hard_directed_pair_count > result.directed_pair_visits
            || cell.hard_directed_pair_fraction.to_bits()
                != (cell.hard_directed_pair_count as f64 / result.directed_pair_visits as f64)
                    .to_bits()
            || !cell.support_weighted_pair_mass.is_finite()
            || cell.support_weighted_pair_mass < 0.0
            || cell.support_weighted_pair_mass > cell.hard_directed_pair_count as f64
            || cell.support_weighted_pair_fraction.to_bits()
                != (cell.support_weighted_pair_mass / result.directed_pair_visits as f64).to_bits()
            || cell.mean_pair_support.map(f64::to_bits) != expected_mean.map(f64::to_bits)
        {
            return Err(invalid("categorical support-mixing matrix cell is invalid"));
        }
        hard_count = hard_count
            .checked_add(cell.hard_directed_pair_count)
            .ok_or_else(|| invalid("hard pair-count sum overflow"))?;
        crate::common::summation::kahan_add(
            &mut support_mass,
            &mut support_correction,
            cell.support_weighted_pair_mass,
        );
    }
    if hard_count != result.directed_pair_visits
        || support_mass.to_bits() != result.total_support_weighted_pair_mass.to_bits()
    {
        return Err(invalid(
            "categorical support-mixing matrix totals are invalid",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
