use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::observation_window_artifact::frame_bound_window_artifact as window_artifact;
use crate::{
    compartment_interface::analysis::measurement_status_name,
    soft_pair_mixing::configuration_digest, workflow::pattern_artifact, DeclaredScalarPatternInput,
    ObservationWindow2D, ScalarMarkId, SoftPairMixingConfig, SoftPairMixingResult,
};

const NODE_KIND: &str = "soft_pair_mixing";
const CONFIG_KIND: &str = "application/vnd.marklab.soft-pair-mixing-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.soft-pair-mixing+json;version=1";
const POLICY: &[u8] = b"serial;exact-directed-physical-radius-pairs;complete-probability-simplex-rows;without-replacement-null";

pub struct SoftPairMixingAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    mark_id: &'a ScalarMarkId,
    config: &'a SoftPairMixingConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> SoftPairMixingAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        mark_id: &'a ScalarMarkId,
        config: &'a SoftPairMixingConfig,
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
                "marklab/{};adapter=soft-pair-mixing-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for SoftPairMixingAnalysisNode<'_> {
    type Output = SoftPairMixingResult;

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
        crate::soft_pair_mixing(self.input, self.window, self.mark_id, self.config)
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
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    radius_um: f64,
    limits: crate::SoftPairMixingLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftPairMixingConfig,
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
    result: &SoftPairMixingResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftPairMixingConfig,
) -> io::Result<()> {
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let levels = table
        .probability_simplex_levels(mark_id)
        .ok_or_else(|| invalid("typed probability simplex is absent"))?;
    let expected_digest =
        configuration_digest(input, window, mark_id, config).map_err(invalid_owned)?;
    let matrix_cells = levels
        .len()
        .checked_mul(levels.len())
        .ok_or_else(|| invalid("soft pair matrix size overflow"))?;
    let expected_products = result
        .directed_pair_visits
        .checked_add(input.pattern().len())
        .and_then(|value| value.checked_mul(matrix_cells))
        .ok_or_else(|| invalid("soft pair product count overflow"))?;
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
        || result.radius_um.to_bits() != config.radius_um().to_bits()
        || result.class_ids != levels
        || result.normalization
            != "expected_source_target_probability_mass_per_directed_radius_pair"
        || result.random_label_null != "complete_probability_rows_without_replacement"
        || result.configuration_digest != expected_digest.to_string()
        || result.graph_digest.len() != 64
        || !result
            .graph_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || result.matrix.len() != matrix_cells
        || result.directed_pair_visits > config.limits().maximum_pair_visits
        || result.probability_product_evaluations != expected_products
        || result.probability_product_evaluations > config.limits().maximum_probability_products
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.limits != config.limits()
    {
        return Err(invalid(
            "soft pair-mixing result does not match its request",
        ));
    }
    let mut mass_sum = 0.0;
    let mut probability_sum = 0.0;
    let mut random_sum = 0.0;
    for (index, cell) in result.matrix.iter().enumerate() {
        let source = index / levels.len();
        let target = index % levels.len();
        let expected_probability = (result.directed_pair_visits > 0)
            .then(|| cell.expected_pair_mass / result.directed_pair_visits as f64);
        let expected_ratio = expected_probability.and_then(|probability| {
            (cell.random_label_probability > 0.0)
                .then(|| probability / cell.random_label_probability)
        });
        if cell.source_class_index != source
            || cell.source_class_id != levels[source]
            || cell.target_class_index != target
            || cell.target_class_id != levels[target]
            || !cell.expected_pair_mass.is_finite()
            || !(0.0..=result.directed_pair_visits as f64).contains(&cell.expected_pair_mass)
            || !cell.random_label_probability.is_finite()
            || !(0.0..=1.0).contains(&cell.random_label_probability)
            || !same_option(cell.pair_probability, expected_probability)
            || !same_option(cell.enrichment_ratio, expected_ratio)
        {
            return Err(invalid("soft pair-mixing matrix cell is inconsistent"));
        }
        mass_sum += cell.expected_pair_mass;
        probability_sum += cell.pair_probability.unwrap_or(0.0);
        random_sum += cell.random_label_probability;
    }
    if !same_simplex_total(mass_sum, result.directed_pair_visits as f64)
        || (result.directed_pair_visits > 0 && !same_simplex_total(probability_sum, 1.0))
        || !same_simplex_total(random_sum, 1.0)
    {
        return Err(invalid("soft pair-mixing matrix aggregate is inconsistent"));
    }
    Ok(())
}

fn same(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= 1e-9 * (1.0 + right.abs())
}

fn same_simplex_total(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= 5e-5 * (1.0 + right.abs())
}

fn same_option(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
