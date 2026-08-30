use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    cross_pair_correlation::window_artifact, workflow::pattern_artifact,
    DeclaredScalarPatternInput, ObservationWindow2D, PairCorrelationKernel,
    PairCorrelationPointStatus, Pattern, ScalarMarkId,
};

use super::{
    categorical_cross_g::{configuration_digest, inhomogeneous_categorical_cross_pair_correlation},
    codec::validate_intensity_summary,
    InhomogeneousCategoricalCrossPairCorrelationConfig,
    InhomogeneousCategoricalCrossPairCorrelationPoint,
    InhomogeneousCategoricalCrossPairCorrelationResult, InhomogeneousSpatialInference,
    InhomogeneousSpatialLimits,
};

const NODE_KIND: &str = "inhomogeneous_categorical_cross_pair_correlation";
const CONFIG_KIND: &str =
    "application/vnd.marklab.inhomogeneous-categorical-cross-g-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.inhomogeneous-categorical-cross-g-result+json;version=1";
const POLICY: &[u8] = b"serial;typed-categorical-rows;gaussian-2d-type-specific-leave-one-out;cell-centred-window-quadrature;epanechnikov;standard-border-radius-plus-pair-bandwidth-type-specific-inverse-intensity-ratio;independent-fixed-gridded-type-specific-inhomogeneous-binomial;erl";

pub struct InhomogeneousCategoricalCrossPairCorrelationAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    config: &'a InhomogeneousCategoricalCrossPairCorrelationConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> InhomogeneousCategoricalCrossPairCorrelationAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        config: &'a InhomogeneousCategoricalCrossPairCorrelationConfig,
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
                "marklab/{};adapter=inhomogeneous-categorical-cross-g-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for InhomogeneousCategoricalCrossPairCorrelationAnalysisNode<'_> {
    type Output = InhomogeneousCategoricalCrossPairCorrelationResult;

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
        inhomogeneous_categorical_cross_pair_correlation(self.input, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_result(output, self.input, self.window, self.config).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = crate::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.window, self.config).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

pub(crate) fn encode_result(
    output: &InhomogeneousCategoricalCrossPairCorrelationResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &InhomogeneousCategoricalCrossPairCorrelationConfig,
) -> io::Result<Box<[u8]>> {
    validate(output, input, window, config)?;
    crate::exact_float_json::encode(output)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    intensity_bandwidth_um: f64,
    pair_bandwidth_um: f64,
    source_level: &'a str,
    target_level: &'a str,
    integration_grid: [usize; 2],
    simulations: usize,
    seed: u64,
    alpha: f64,
    minimum_intensity_per_um2: f64,
    limits: InhomogeneousSpatialLimits,
    logical_digest: String,
}

fn config_artifact(
    config: &InhomogeneousCategoricalCrossPairCorrelationConfig,
) -> Result<ArtifactRef, NodeError> {
    let intensity = config.intensity_config();
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: intensity.radii_um(),
        intensity_bandwidth_um: intensity.bandwidth_um(),
        pair_bandwidth_um: config.pair_bandwidth_um(),
        source_level: config.source_level(),
        target_level: config.target_level(),
        integration_grid: intensity.integration_grid(),
        simulations: intensity.simulations(),
        seed: intensity.seed(),
        alpha: intensity.alpha(),
        minimum_intensity_per_um2: intensity.minimum_intensity_per_um2(),
        limits: intensity.limits(),
        logical_digest: configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &InhomogeneousCategoricalCrossPairCorrelationResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &InhomogeneousCategoricalCrossPairCorrelationConfig,
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
    let source_code = level_code(levels, config.source_level())?;
    let target_code = level_code(levels, config.target_level())?;
    let source_rows = matching_rows(codes, source_code);
    let target_rows = matching_rows(codes, target_code);
    let source_pattern = subset_pattern(input.pattern(), &source_rows)?;
    let target_pattern = subset_pattern(input.pattern(), &target_rows)?;
    let intensity = config.intensity_config();
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
        || result.source_level != config.source_level()
        || result.target_level != config.target_level()
        || result.source_count != source_rows.len()
        || result.target_count != target_rows.len()
        || result.source_rows != source_rows
        || result.target_rows != target_rows
        || result.kernel != PairCorrelationKernel::Epanechnikov
        || result.pair_bandwidth_um.to_bits() != config.pair_bandwidth_um().to_bits()
        || result.intensity_bandwidth_um.to_bits() != intensity.bandwidth_um().to_bits()
        || result.edge_correction
            != "standard_border_radius_plus_pair_bandwidth_type_specific_inverse_intensity_ratio"
        || result.configuration_digest != configuration_digest(config).to_string()
        || result.estimated_storage_bytes > intensity.limits().maximum_retained_bytes
        || result.observed_pair_visits > result.total_pair_visits
        || result.total_pair_visits > intensity.limits().maximum_pair_visits
        || result.intensity_evaluations > intensity.limits().maximum_intensity_evaluations
        || result.limits != intensity.limits()
        || result.curve.len() != intensity.radii_um().len()
        || result.inference.null_model
            != "independent_fixed_gridded_type_specific_inhomogeneous_binomial"
        || result.inference.randomization_unit
            != "independent_source_and_target_location_patterns_conditioned_on_type_counts"
        || result.inference.simulations_completed != intensity.simulations()
        || result.inference.seed != intensity.seed()
        || result.inference.alpha.to_bits() != intensity.alpha().to_bits()
        || result.inference.null_draws > intensity.limits().maximum_null_draws
    {
        return Err(invalid("result does not match its cache-bound request"));
    }
    validate_intensity_summary(&result.source_intensity, &source_pattern, window, intensity)?;
    validate_intensity_summary(&result.target_intensity, &target_pattern, window, intensity)?;
    validate_curve(&result.curve, &result.inference, intensity.radii_um())
}

fn validate_curve(
    curve: &[InhomogeneousCategoricalCrossPairCorrelationPoint],
    inference: &InhomogeneousSpatialInference,
    radii: &[f64],
) -> io::Result<()> {
    for (point, radius) in curve.iter().zip(radii) {
        let status_consistent = match point.status {
            PairCorrelationPointStatus::Available => {
                point.eligible_source_centers > 0
                    && point.directed_source_target_pairs_in_support > 0
                    && point.inverse_intensity_kernel_sum > 0.0
                    && point.eligible_source_inverse_intensity_sum > 0.0
                    && point.cross_g.is_some()
            }
            PairCorrelationPointStatus::NoEligibleCenters => {
                point.eligible_source_centers == 0
                    && point.directed_source_target_pairs_in_support == 0
                    && point.inverse_intensity_kernel_sum.to_bits() == 0.0_f64.to_bits()
                    && point.eligible_source_inverse_intensity_sum.to_bits() == 0.0_f64.to_bits()
                    && point.cross_g.is_none()
            }
            PairCorrelationPointStatus::NoPairsInKernelSupport => {
                point.eligible_source_centers > 0
                    && point.directed_source_target_pairs_in_support == 0
                    && point.inverse_intensity_kernel_sum.to_bits() == 0.0_f64.to_bits()
                    && point.eligible_source_inverse_intensity_sum > 0.0
                    && point.cross_g.is_none()
            }
        };
        if !status_consistent
            || point.radius_um.to_bits() != radius.to_bits()
            || point
                .cross_g
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point.theoretical_cross_g.to_bits() != 1.0_f64.to_bits()
            || point.lower_cross_g.is_some_and(|value| !value.is_finite())
            || point.upper_cross_g.is_some_and(|value| !value.is_finite())
            || point.lower_cross_g.is_some() != point.upper_cross_g.is_some()
            || point.inference_eligible
                != (point.lower_cross_g.is_some() && point.upper_cross_g.is_some())
        {
            return Err(invalid("cross-g curve is inconsistent or non-finite"));
        }
    }
    let fields = [
        inference.p_global,
        inference.erl_depth,
        inference.critical_depth,
    ];
    if fields.iter().flatten().any(|value| !value.is_finite())
        || fields.iter().any(Option::is_some) != fields.iter().all(Option::is_some)
        || inference.eligible_radius_count
            != curve
                .iter()
                .filter(|point| point.inference_eligible)
                .count()
    {
        return Err(invalid("inference is inconsistent or non-finite"));
    }
    Ok(())
}

fn level_code(levels: &[String], requested: &str) -> io::Result<u32> {
    levels
        .iter()
        .position(|level| level == requested)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid("categorical level is absent"))
}

fn matching_rows(codes: &[u32], requested: u32) -> Vec<usize> {
    codes
        .iter()
        .enumerate()
        .filter_map(|(row, code)| (*code == requested).then_some(row))
        .collect()
}

fn subset_pattern(pattern: &Pattern, rows: &[usize]) -> io::Result<Pattern> {
    Pattern::from_arrays(
        rows.iter().map(|row| pattern.x_um[*row]).collect(),
        rows.iter().map(|row| pattern.y_um[*row]).collect(),
        vec![0; rows.len()],
        pattern.meta.clone(),
    )
    .map_err(invalid_owned)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
