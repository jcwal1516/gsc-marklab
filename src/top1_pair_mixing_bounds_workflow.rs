use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::observation_window_artifact::frame_bound_window_artifact as window_artifact;
use crate::{
    compartment_interface::analysis::measurement_status_name,
    top1_pair_mixing_bounds::{configuration_digest, TOP1_PAIR_MIXING_UNCERTAINTY_SEMANTICS},
    workflow::pattern_artifact,
    DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId, Top1PairMixingBoundsConfig,
    Top1PairMixingBoundsResult,
};

const NODE_KIND: &str = "top1_pair_mixing_bounds";
const CONFIG_KIND: &str = "application/vnd.marklab.top1-pair-mixing-bounds-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.top1-pair-mixing-bounds+json;version=1";
const POLICY: &[u8] = b"serial;exact-physical-radius-graph;top1-winner-probability;coordinatewise-conservative-pair-bounds;no-simplex-imputation";

/// Cache-addressed top-1 multiclass uncertainty-bounds node.
pub struct Top1PairMixingBoundsAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    categorical_mark_id: &'a ScalarMarkId,
    confidence_mark_id: &'a ScalarMarkId,
    config: &'a Top1PairMixingBoundsConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> Top1PairMixingBoundsAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        categorical_mark_id: &'a ScalarMarkId,
        confidence_mark_id: &'a ScalarMarkId,
        config: &'a Top1PairMixingBoundsConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(
            input,
            window,
            categorical_mark_id,
            confidence_mark_id,
            config,
        )?;
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
            confidence_mark_id,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=top1-pair-mixing-bounds-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for Top1PairMixingBoundsAnalysisNode<'_> {
    type Output = Top1PairMixingBoundsResult;

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
                self.confidence_mark_id,
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
        crate::top1_pair_mixing_bounds(
            self.input,
            self.window,
            self.categorical_mark_id,
            self.confidence_mark_id,
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
            self.confidence_mark_id,
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
            self.confidence_mark_id,
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
    confidence_mark_id: &'a str,
    radius_um: f64,
    source_class_count: usize,
    limits: crate::Top1PairMixingBoundsLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    confidence_mark_id: &ScalarMarkId,
    config: &Top1PairMixingBoundsConfig,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(
        input,
        window,
        categorical_mark_id,
        confidence_mark_id,
        config,
    )
    .map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        categorical_mark_id: categorical_mark_id.as_str(),
        confidence_mark_id: confidence_mark_id.as_str(),
        radius_um: config.radius_um(),
        source_class_count: config.source_class_count(),
        limits: config.limits(),
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &Top1PairMixingBoundsResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    confidence_mark_id: &ScalarMarkId,
    config: &Top1PairMixingBoundsConfig,
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
    let confidence_status = table
        .measurement_status(confidence_mark_id)
        .ok_or_else(|| invalid("confidence measurement status is absent"))?;
    let expected = configuration_digest(
        input,
        window,
        categorical_mark_id,
        confidence_mark_id,
        config,
    )
    .map_err(invalid_owned)?;
    let matrix_len = levels
        .len()
        .checked_mul(levels.len())
        .ok_or_else(|| invalid("matrix size overflow"))?;
    let expected_products = result
        .directed_pair_visits
        .checked_mul(matrix_len)
        .ok_or_else(|| invalid("bound-product count overflow"))?;
    if result.case_id != input.pattern().meta.case_id
        || result.timepoint != input.pattern().meta.timepoint
        || result.categorical_mark_id != categorical_mark_id.as_str()
        || result.confidence_mark_id != confidence_mark_id.as_str()
        || result.categorical_measurement_status != measurement_status_name(categorical_status)
        || result.confidence_measurement_status != measurement_status_name(confidence_status)
        || result.coordinate_frame_id != input.coordinate_frame_id().as_str()
        || result.window_digest != window.descriptor().logical_digest.to_string()
        || result.radius_um.to_bits() != config.radius_um().to_bits()
        || result.class_ids != levels
        || result.source_class_count != config.source_class_count()
        || result.uncertainty_semantics != TOP1_PAIR_MIXING_UNCERTAINTY_SEMANTICS
        || result.configuration_digest != expected.to_string()
        || result.graph_digest.len() != 64
        || result.point_count != input.pattern().len()
        || result.matrix.len() != matrix_len
        || result.directed_pair_visits == 0
        || result.directed_pair_visits > config.limits().maximum_pair_visits
        || result.bound_product_evaluations != expected_products
        || result.bound_product_evaluations > config.limits().maximum_bound_products
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.limits != config.limits()
    {
        return Err(invalid("top-1 pair bounds do not match their request"));
    }
    for (index, cell) in result.matrix.iter().enumerate() {
        let source = index / levels.len();
        let target = index % levels.len();
        if cell.source_class_index != source
            || cell.target_class_index != target
            || cell.source_class_id != levels[source]
            || cell.target_class_id != levels[target]
            || !cell.lower_expected_pair_mass.is_finite()
            || !cell.upper_expected_pair_mass.is_finite()
            || cell.lower_expected_pair_mass < 0.0
            || cell.lower_expected_pair_mass > cell.upper_expected_pair_mass
            || cell.upper_expected_pair_mass > result.directed_pair_visits as f64
            || !cell.lower_pair_fraction.is_finite()
            || !cell.upper_pair_fraction.is_finite()
            || cell.lower_pair_fraction < 0.0
            || cell.lower_pair_fraction > cell.upper_pair_fraction
            || cell.upper_pair_fraction > 1.0
            || cell.lower_pair_fraction.to_bits()
                != (cell.lower_expected_pair_mass / result.directed_pair_visits as f64).to_bits()
            || cell.upper_pair_fraction.to_bits()
                != (cell.upper_expected_pair_mass / result.directed_pair_visits as f64).to_bits()
        {
            return Err(invalid("top-1 pair-bound matrix cell is invalid"));
        }
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
