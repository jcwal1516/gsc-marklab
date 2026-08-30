use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    workflow::pattern_artifact, DeclaredScalarPatternInput, ObservationWindow2D,
    PairCorrelationKernel, PairCorrelationPointStatus, ScalarMarkId,
};

use super::{
    categorical_cross_pair_correlation, configuration_digest,
    CategoricalCrossPairCorrelationConfig, CategoricalCrossPairCorrelationResult,
};

const NODE_KIND: &str = "categorical_cross_pair_correlation";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str = "application/vnd.marklab.categorical-cross-g-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.categorical-cross-g-result+json;version=1";
const POLICY: &[u8] = b"serial;retained-directed-pairs;epanechnikov;explicit-bandwidth;standard-border-radius-plus-bandwidth;complete-row-random-labeling;erl";

pub struct CategoricalCrossPairCorrelationAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    config: &'a CategoricalCrossPairCorrelationConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> CategoricalCrossPairCorrelationAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        config: &'a CategoricalCrossPairCorrelationConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(config)?;
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
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=categorical-cross-g-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for CategoricalCrossPairCorrelationAnalysisNode<'_> {
    type Output = CategoricalCrossPairCorrelationResult;

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
            config_artifact(self.config)?,
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
        categorical_cross_pair_correlation(self.input, self.window, self.config)
            .map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_result(output, self.input, self.window, self.config).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.window, self.config).map_err(NodeError::decode)?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

pub(crate) fn encode_result(
    output: &CategoricalCrossPairCorrelationResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &CategoricalCrossPairCorrelationConfig,
) -> io::Result<Box<[u8]>> {
    validate(output, input, window, config)?;
    serde_json::to_vec_pretty(output)
        .map(Vec::into_boxed_slice)
        .map_err(io::Error::other)
}

#[derive(Serialize)]
struct WindowArtifact {
    digest: String,
    frame: String,
}

pub(crate) fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or_else(|| NodeError::input(invalid("window has no frame")))?;
    let bytes = serde_json::to_vec(&WindowArtifact {
        digest: window.descriptor().logical_digest.to_string(),
        frame: frame.as_str().into(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(WINDOW_KIND, &bytes).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    bandwidth_um: f64,
    source_level: &'a str,
    target_level: &'a str,
    permutations: usize,
    seed: u64,
    alpha: f64,
    limits: [usize; 5],
    logical_digest: String,
}

fn config_artifact(
    config: &CategoricalCrossPairCorrelationConfig,
) -> Result<ArtifactRef, NodeError> {
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: &config.radii_um,
        bandwidth_um: config.bandwidth_um,
        source_level: &config.source_level,
        target_level: &config.target_level,
        permutations: config.permutations,
        seed: config.seed,
        alpha: config.alpha,
        limits: [
            config.limits.maximum_points,
            config.limits.maximum_radii,
            config.limits.maximum_directed_pairs,
            config.limits.maximum_permutation_pair_evaluations,
            config.limits.maximum_retained_bytes,
        ],
        logical_digest: configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &CategoricalCrossPairCorrelationResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &CategoricalCrossPairCorrelationConfig,
) -> io::Result<()> {
    let mark_id = ScalarMarkId::new("histologic_compartment").map_err(invalid_owned)?;
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let levels = table
        .categorical_levels(&mark_id)
        .ok_or_else(|| invalid("categorical levels are absent"))?;
    let codes = table
        .categorical_values(&mark_id)
        .ok_or_else(|| invalid("categorical values are absent"))?;
    let source_code = levels
        .iter()
        .position(|level| level == &config.source_level)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid("source level is absent"))?;
    let target_code = levels
        .iter()
        .position(|level| level == &config.target_level)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid("target level is absent"))?;
    let expected_status = match table
        .measurement_status(&mark_id)
        .ok_or_else(|| invalid("measurement status is absent"))?
    {
        marklab_data::MeasurementStatus::Measured => "measured",
        marklab_data::MeasurementStatus::ImportedPrediction => "imported_prediction",
        marklab_data::MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        marklab_data::MeasurementStatus::DerivedSummary => "derived_summary",
    };
    if result.mark_id != "histologic_compartment"
        || result.measurement_status != expected_status
        || result.coordinate_frame_id != input.coordinate_frame_id().as_str()
        || result.window.logical_digest != window.descriptor().logical_digest.to_string()
        || result.source_level != config.source_level
        || result.target_level != config.target_level
        || result.kernel != PairCorrelationKernel::Epanechnikov
        || result.bandwidth_um.to_bits() != config.bandwidth_um.to_bits()
        || result.geometry_build_count != 1
        || result.edge_correction != "standard_border_radius_plus_bandwidth"
        || result.source_count != codes.iter().filter(|code| **code == source_code).count()
        || result.target_count != codes.iter().filter(|code| **code == target_code).count()
        || result.configuration_digest != configuration_digest(config).to_string()
        || result.curve.len() != config.radii_um.len()
        || result.inference.null_model != "random_labeling"
        || result.inference.permutation_unit != "complete_categorical_row"
        || result.inference.permutations_completed != config.permutations
        || result.inference.seed != config.seed
        || result.inference.alpha.to_bits() != config.alpha.to_bits()
        || result.estimated_storage_bytes > config.limits.maximum_retained_bytes
        || result.inference.permutation_pair_evaluations
            != result
                .directed_pair_count
                .checked_mul(config.permutations)
                .ok_or_else(|| invalid("permutation work overflow"))?
    {
        return Err(invalid(
            "cross-g result does not match its cache-bound request",
        ));
    }
    for (point, radius) in result.curve.iter().zip(&config.radii_um) {
        if point.radius_um.to_bits() != radius.to_bits()
            || !point.kernel_weight_sum.is_finite()
            || point.kernel_weight_sum < 0.0
            || point.cross_g.is_some() != (point.status == PairCorrelationPointStatus::Available)
            || point.cross_g.is_some_and(|value| !value.is_finite())
            || point.theoretical_cross_g.to_bits() != 1.0_f64.to_bits()
            || point.lower_cross_g.is_some_and(|value| !value.is_finite())
            || point.upper_cross_g.is_some_and(|value| !value.is_finite())
            || point.inference_eligible
                != (point.lower_cross_g.is_some() && point.upper_cross_g.is_some())
        {
            return Err(invalid("cross-g curve is inconsistent or non-finite"));
        }
    }
    let inference_fields = [
        result.inference.p_global,
        result.inference.erl_depth,
        result.inference.critical_depth,
    ];
    if inference_fields
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
        || inference_fields.iter().any(Option::is_some)
            != inference_fields.iter().all(Option::is_some)
        || result.inference.eligible_radius_count
            != result
                .curve
                .iter()
                .filter(|point| point.inference_eligible)
                .count()
    {
        return Err(invalid("cross-g inference is inconsistent or non-finite"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
