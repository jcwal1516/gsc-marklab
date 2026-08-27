use std::{io, str::FromStr};

use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::{Deserialize, Serialize};

use crate::{
    continuous_mark_correlation::mark_statistics, mark_weighted_k::configuration_digest,
    workflow::pattern_artifact, ClassicalWindowSummary, DeclaredScalarPatternInput,
    MarkWeightedKComponentInference, MarkWeightedKConfig, MarkWeightedKGeometrySummary,
    MarkWeightedKInferenceSummary, MarkWeightedKLimits, MarkWeightedKPoint,
    MarkWeightedKPointStatus, MarkWeightedKResult, ObservationWindow2D, ScalarMarkId,
};

const NODE_KIND: &str = "continuous_mark_weighted_k";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str = "application/vnd.marklab.mark-weighted-k-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.mark-weighted-k-result+json;version=1";
const EXECUTION_POLICY: &[u8] = b"serial;exact-rstar;retained-directed-pairs;cumulative-standard-border;product-over-global-mean-squared;unweighted-k-baseline;complete-continuous-row-random-labeling;erl";
const IMPLEMENTATION: &str = "mark-weighted-k-analysis-node-v1";

/// Exact typed mark-weighted K node with store-verified MarkTable provenance.
pub struct MarkWeightedKAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    config: &'a MarkWeightedKConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> MarkWeightedKAnalysisNode<'a> {
    /// Bind exact marks, window, weight, edge policy, null controls, and limits.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        config: &'a MarkWeightedKConfig,
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
                "marklab/{};adapter={IMPLEMENTATION}",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Exact versioned node specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for MarkWeightedKAnalysisNode<'_> {
    type Output = MarkWeightedKResult;

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
        let pattern = pattern_artifact(self.input.pattern())?;
        self.inputs[0]
            .verify_identity(pattern.digest(), pattern.byte_len())
            .map_err(NodeError::input)?;
        let declared = self
            .input
            .declared_artifact_ref()
            .map_err(NodeError::input)?;
        self.inputs[1]
            .verify_identity(declared.digest(), declared.byte_len())
            .map_err(NodeError::input)?;
        let window = window_artifact(self.window)?;
        self.inputs[2]
            .verify_identity(window.digest(), window.byte_len())
            .map_err(NodeError::input)?;
        let config = config_artifact(self.config)?;
        self.inputs[3]
            .verify_identity(config.digest(), config.byte_len())
            .map_err(NodeError::input)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: EXECUTION_POLICY,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::continuous_mark_weighted_k(self.input, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate_result(output).map_err(NodeError::encoding)?;
        serde_json::to_vec_pretty(&ResultWire::from(output))
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let wire: ResultWire = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        let result = MarkWeightedKResult::try_from(wire).map_err(NodeError::decode)?;
        validate_result(&result).map_err(NodeError::decode)?;
        let table = self
            .input
            .mark_table()
            .ok_or_else(|| NodeError::decode(invalid("typed MarkTable is absent")))?;
        let marks = table
            .continuous_values(self.config.mark_id())
            .ok_or_else(|| NodeError::decode(invalid("cache-bound continuous mark is absent")))?;
        let (mean, variance, expected) = mark_statistics(marks).map_err(NodeError::decode)?;
        let descriptor = self.window.descriptor();
        if result.configuration_digest != configuration_digest(self.config)
            || result.mark_id != *self.config.mark_id()
            || result.limits != self.config.limits()
            || result.inference.permutations_requested != self.config.permutations()
            || result.inference.seed != self.config.seed()
            || result.inference.alpha.to_bits() != self.config.alpha().to_bits()
            || result.coordinate_frame_id != *self.input.coordinate_frame_id()
            || result.measurement_status
                != table.measurement_status(&result.mark_id).ok_or_else(|| {
                    NodeError::decode(invalid("decoded continuous mark is absent"))
                })?
            || !same(result.global_mark_mean, mean)
            || !same(result.global_mark_population_variance, variance)
            || !same(result.expected_random_label_weight, expected)
            || result.geometry.point_count != self.input.pattern().len()
            || result.window.logical_digest != descriptor.logical_digest.to_string()
            || result.window.area_um2.to_bits() != descriptor.area_um2.to_bits()
            || result.window.perimeter_um.to_bits() != descriptor.perimeter_um.to_bits()
            || result.curve.len() != self.config.radii_um().len()
            || result
                .curve
                .iter()
                .zip(self.config.radii_um())
                .any(|(point, radius)| point.radius_um.to_bits() != radius.to_bits())
        {
            return Err(NodeError::decode(invalid(
                "decoded mark-weighted K result does not match its cache-bound request",
            )));
        }
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct WindowArtifact<'a> {
    logical_digest: String,
    coordinate_frame_id: &'a str,
}

fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or_else(|| NodeError::input(invalid("observation window is not frame-bound")))?;
    let encoded = serde_json::to_vec(&WindowArtifact {
        logical_digest: window.descriptor().logical_digest.to_string(),
        coordinate_frame_id: frame.as_str(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(WINDOW_KIND, &encoded).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    radii_um: &'a [f64],
    weight_function: &'static str,
    edge_correction: &'static str,
    permutations: usize,
    seed: u64,
    alpha: f64,
    limits: LimitsWire,
    logical_digest: String,
}

fn config_artifact(config: &MarkWeightedKConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        mark_id: config.mark_id().as_str(),
        radii_um: config.radii_um(),
        weight_function: "product_over_global_arithmetic_mean_squared",
        edge_correction: "standard_border_reduced_sample",
        permutations: config.permutations(),
        seed: config.seed(),
        alpha: config.alpha(),
        limits: config.limits().into(),
        logical_digest: configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &encoded).map_err(NodeError::input)
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LimitsWire {
    maximum_points: usize,
    maximum_radii: usize,
    maximum_directed_pairs: usize,
    maximum_permutation_pair_evaluations: usize,
    maximum_retained_bytes: usize,
}

impl From<MarkWeightedKLimits> for LimitsWire {
    fn from(value: MarkWeightedKLimits) -> Self {
        Self {
            maximum_points: value.maximum_points,
            maximum_radii: value.maximum_radii,
            maximum_directed_pairs: value.maximum_directed_pairs,
            maximum_permutation_pair_evaluations: value.maximum_permutation_pair_evaluations,
            maximum_retained_bytes: value.maximum_retained_bytes,
        }
    }
}

impl TryFrom<LimitsWire> for MarkWeightedKLimits {
    type Error = io::Error;
    fn try_from(value: LimitsWire) -> Result<Self, Self::Error> {
        MarkWeightedKLimits::new(
            value.maximum_points,
            value.maximum_radii,
            value.maximum_directed_pairs,
            value.maximum_permutation_pair_evaluations,
            value.maximum_retained_bytes,
        )
        .map_err(|error| invalid(error.to_string()))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultWire {
    format: String,
    mark_id: String,
    measurement_status: String,
    unit: String,
    coordinate_frame_id: String,
    window: ClassicalWindowSummary,
    weight_function: String,
    edge_correction: String,
    global_mark_mean: f64,
    global_mark_population_variance: f64,
    expected_random_label_weight: f64,
    geometry: GeometryWire,
    configuration_digest: String,
    limits: LimitsWire,
    curve: Vec<PointWire>,
    inference: InferenceWire,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GeometryWire {
    point_count: usize,
    directed_pair_count: usize,
    geometry_build_count: usize,
    geometry_digest: String,
    pair_plan_digest: String,
    estimated_storage_bytes: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PointWire {
    radius_um: f64,
    status: String,
    eligible_centers: usize,
    directed_pairs: usize,
    mark_product_sum: f64,
    normalized_weight_sum: f64,
    weighted_k: Option<f64>,
    unweighted_k: Option<f64>,
    expected_random_label_weighted_k: Option<f64>,
    theoretical_k: f64,
    inference_eligible: bool,
    lower_weighted_k: Option<f64>,
    upper_weighted_k: Option<f64>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InferenceWire {
    null_model: String,
    permutation_unit: String,
    alternative: String,
    multiplicity: String,
    weighted_k: Option<ComponentWire>,
    permutations_requested: usize,
    permutations_attempted: usize,
    permutations_completed: usize,
    seed: u64,
    alpha: f64,
    permutation_pair_evaluations: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ComponentWire {
    p_global: f64,
    erl_depth: f64,
    critical_depth: f64,
    eligible_radius_count: usize,
}

impl From<&MarkWeightedKResult> for ResultWire {
    fn from(value: &MarkWeightedKResult) -> Self {
        Self {
            format: "marklab.mark-weighted-k/1".into(),
            mark_id: value.mark_id.as_str().into(),
            measurement_status: status_name(value.measurement_status).into(),
            unit: value.unit.clone(),
            coordinate_frame_id: value.coordinate_frame_id.as_str().into(),
            window: value.window.clone(),
            weight_function: value.weight_function.clone(),
            edge_correction: value.edge_correction.clone(),
            global_mark_mean: value.global_mark_mean,
            global_mark_population_variance: value.global_mark_population_variance,
            expected_random_label_weight: value.expected_random_label_weight,
            geometry: GeometryWire {
                point_count: value.geometry.point_count,
                directed_pair_count: value.geometry.directed_pair_count,
                geometry_build_count: value.geometry.geometry_build_count,
                geometry_digest: value.geometry.geometry_digest.to_string(),
                pair_plan_digest: value.geometry.pair_plan_digest.to_string(),
                estimated_storage_bytes: value.geometry.estimated_storage_bytes,
            },
            configuration_digest: value.configuration_digest.to_string(),
            limits: value.limits.into(),
            curve: value.curve.iter().map(PointWire::from).collect(),
            inference: InferenceWire::from(&value.inference),
        }
    }
}

impl From<&MarkWeightedKPoint> for PointWire {
    fn from(value: &MarkWeightedKPoint) -> Self {
        Self {
            radius_um: value.radius_um,
            status: match value.status {
                MarkWeightedKPointStatus::Available => "available",
                MarkWeightedKPointStatus::NoEligibleCenters => "no_eligible_centers",
            }
            .into(),
            eligible_centers: value.eligible_centers,
            directed_pairs: value.directed_pairs,
            mark_product_sum: value.mark_product_sum,
            normalized_weight_sum: value.normalized_weight_sum,
            weighted_k: value.weighted_k,
            unweighted_k: value.unweighted_k,
            expected_random_label_weighted_k: value.expected_random_label_weighted_k,
            theoretical_k: value.theoretical_k,
            inference_eligible: value.inference_eligible,
            lower_weighted_k: value.lower_weighted_k,
            upper_weighted_k: value.upper_weighted_k,
        }
    }
}

impl From<&MarkWeightedKInferenceSummary> for InferenceWire {
    fn from(value: &MarkWeightedKInferenceSummary) -> Self {
        Self {
            null_model: value.null_model.clone(),
            permutation_unit: value.permutation_unit.clone(),
            alternative: value.alternative.clone(),
            multiplicity: value.multiplicity.clone(),
            weighted_k: value.weighted_k.as_ref().map(ComponentWire::from),
            permutations_requested: value.permutations_requested,
            permutations_attempted: value.permutations_attempted,
            permutations_completed: value.permutations_completed,
            seed: value.seed,
            alpha: value.alpha,
            permutation_pair_evaluations: value.permutation_pair_evaluations,
        }
    }
}

impl From<&MarkWeightedKComponentInference> for ComponentWire {
    fn from(value: &MarkWeightedKComponentInference) -> Self {
        Self {
            p_global: value.p_global,
            erl_depth: value.erl_depth,
            critical_depth: value.critical_depth,
            eligible_radius_count: value.eligible_radius_count,
        }
    }
}

impl TryFrom<ResultWire> for MarkWeightedKResult {
    type Error = io::Error;
    fn try_from(value: ResultWire) -> Result<Self, Self::Error> {
        if value.format != "marklab.mark-weighted-k/1" {
            return Err(invalid("unsupported mark-weighted K result format"));
        }
        Ok(Self {
            mark_id: ScalarMarkId::new(value.mark_id)
                .map_err(|error| invalid(error.to_string()))?,
            measurement_status: parse_status(&value.measurement_status)?,
            unit: value.unit,
            coordinate_frame_id: CoordinateFrameId::new(value.coordinate_frame_id)
                .map_err(|error| invalid(error.to_string()))?,
            window: value.window,
            weight_function: value.weight_function,
            edge_correction: value.edge_correction,
            global_mark_mean: value.global_mark_mean,
            global_mark_population_variance: value.global_mark_population_variance,
            expected_random_label_weight: value.expected_random_label_weight,
            geometry: MarkWeightedKGeometrySummary {
                point_count: value.geometry.point_count,
                directed_pair_count: value.geometry.directed_pair_count,
                geometry_build_count: value.geometry.geometry_build_count,
                geometry_digest: ContentDigest::from_str(&value.geometry.geometry_digest)
                    .map_err(|error| invalid(error.to_string()))?,
                pair_plan_digest: ContentDigest::from_str(&value.geometry.pair_plan_digest)
                    .map_err(|error| invalid(error.to_string()))?,
                estimated_storage_bytes: value.geometry.estimated_storage_bytes,
            },
            configuration_digest: ContentDigest::from_str(&value.configuration_digest)
                .map_err(|error| invalid(error.to_string()))?,
            limits: value.limits.try_into()?,
            curve: value
                .curve
                .into_iter()
                .map(MarkWeightedKPoint::try_from)
                .collect::<Result<_, _>>()?,
            inference: value.inference.into(),
        })
    }
}

impl TryFrom<PointWire> for MarkWeightedKPoint {
    type Error = io::Error;
    fn try_from(value: PointWire) -> Result<Self, Self::Error> {
        let status = match value.status.as_str() {
            "available" => MarkWeightedKPointStatus::Available,
            "no_eligible_centers" => MarkWeightedKPointStatus::NoEligibleCenters,
            _ => return Err(invalid("invalid mark-weighted K point status")),
        };
        Ok(Self {
            radius_um: value.radius_um,
            status,
            eligible_centers: value.eligible_centers,
            directed_pairs: value.directed_pairs,
            mark_product_sum: value.mark_product_sum,
            normalized_weight_sum: value.normalized_weight_sum,
            weighted_k: value.weighted_k,
            unweighted_k: value.unweighted_k,
            expected_random_label_weighted_k: value.expected_random_label_weighted_k,
            theoretical_k: value.theoretical_k,
            inference_eligible: value.inference_eligible,
            lower_weighted_k: value.lower_weighted_k,
            upper_weighted_k: value.upper_weighted_k,
        })
    }
}

impl From<InferenceWire> for MarkWeightedKInferenceSummary {
    fn from(value: InferenceWire) -> Self {
        Self {
            null_model: value.null_model,
            permutation_unit: value.permutation_unit,
            alternative: value.alternative,
            multiplicity: value.multiplicity,
            weighted_k: value.weighted_k.map(Into::into),
            permutations_requested: value.permutations_requested,
            permutations_attempted: value.permutations_attempted,
            permutations_completed: value.permutations_completed,
            seed: value.seed,
            alpha: value.alpha,
            permutation_pair_evaluations: value.permutation_pair_evaluations,
        }
    }
}

impl From<ComponentWire> for MarkWeightedKComponentInference {
    fn from(value: ComponentWire) -> Self {
        Self {
            p_global: value.p_global,
            erl_depth: value.erl_depth,
            critical_depth: value.critical_depth,
            eligible_radius_count: value.eligible_radius_count,
        }
    }
}

fn validate_result(result: &MarkWeightedKResult) -> io::Result<()> {
    if result.mark_id.as_str() != "nucleus_area_um2"
        || result.unit != "square_micrometer"
        || result.weight_function != "product_over_global_arithmetic_mean_squared"
        || result.edge_correction != "standard_border_reduced_sample"
        || !positive_finite(result.global_mark_mean)
        || !positive_finite(result.global_mark_population_variance)
        || !positive_finite(result.expected_random_label_weight)
        || result.expected_random_label_weight > 1.0
        || result.geometry.point_count < 2
        || result.geometry.directed_pair_count > result.limits.maximum_directed_pairs
        || result.geometry.geometry_build_count != 1
        || result.geometry.estimated_storage_bytes > result.limits.maximum_retained_bytes
        || result.curve.is_empty()
        || result.curve.len() > result.limits.maximum_radii
        || result.inference.permutations_requested == 0
        || result.inference.permutations_attempted != result.inference.permutations_requested
        || result.inference.permutations_completed != result.inference.permutations_requested
        || result.inference.permutation_pair_evaluations
            != result
                .geometry
                .directed_pair_count
                .saturating_mul(result.inference.permutations_requested)
        || result.inference.permutation_pair_evaluations
            > result.limits.maximum_permutation_pair_evaluations
        || result.inference.null_model != "random_labeling"
        || result.inference.permutation_unit != "complete_continuous_mark_row"
        || result.inference.alternative != "two_sided"
        || result.inference.multiplicity != "single_weighted_k_erl_family"
        || !(result.inference.alpha.is_finite()
            && result.inference.alpha > 0.0
            && result.inference.alpha < 1.0)
        || (result.inference.permutations_requested.saturating_add(1) as f64)
            * result.inference.alpha
            < 1.0
    {
        return Err(invalid("invalid mark-weighted K metadata"));
    }
    let normalization = result.global_mark_mean * result.global_mark_mean;
    let mut previous = 0.0;
    let mut eligible = 0;
    for point in &result.curve {
        if !point.radius_um.is_finite()
            || point.radius_um <= previous
            || point.directed_pairs > result.geometry.directed_pair_count
            || point.eligible_centers > result.geometry.point_count
            || !nonnegative_finite(point.mark_product_sum)
            || !nonnegative_finite(point.normalized_weight_sum)
            || !same(
                point.normalized_weight_sum,
                point.mark_product_sum / normalization,
            )
            || !same(
                point.theoretical_k,
                std::f64::consts::PI * point.radius_um.powi(2),
            )
        {
            return Err(invalid("invalid mark-weighted K curve identity"));
        }
        previous = point.radius_um;
        match point.status {
            MarkWeightedKPointStatus::Available => {
                let denominator = result
                    .geometry
                    .point_count
                    .checked_mul(point.eligible_centers)
                    .ok_or_else(|| invalid("mark-weighted K count overflow"))?;
                let (Some(weighted), Some(unweighted), Some(expected)) = (
                    point.weighted_k,
                    point.unweighted_k,
                    point.expected_random_label_weighted_k,
                ) else {
                    return Err(invalid("available mark-weighted K lacks a value"));
                };
                if denominator == 0
                    || !nonnegative_finite(weighted)
                    || !nonnegative_finite(unweighted)
                    || !nonnegative_finite(expected)
                    || !same(
                        weighted,
                        result.window.area_um2 * point.normalized_weight_sum / denominator as f64,
                    )
                    || !same(
                        unweighted,
                        result.window.area_um2 * point.directed_pairs as f64 / denominator as f64,
                    )
                    || !same(expected, unweighted * result.expected_random_label_weight)
                {
                    return Err(invalid("invalid mark-weighted K formula"));
                }
            }
            MarkWeightedKPointStatus::NoEligibleCenters => {
                if point.eligible_centers != 0
                    || point.directed_pairs != 0
                    || point.mark_product_sum != 0.0
                    || point.weighted_k.is_some()
                    || point.unweighted_k.is_some()
                    || point.expected_random_label_weighted_k.is_some()
                {
                    return Err(invalid("unavailable mark-weighted K contains a value"));
                }
            }
        }
        if point.inference_eligible != point.weighted_k.is_some() {
            return Err(invalid("invalid mark-weighted K inference eligibility"));
        }
        eligible += validate_envelope(point)?;
    }
    validate_component(
        result.inference.weighted_k.as_ref(),
        eligible,
        result.inference.permutations_requested,
    )
}

fn validate_envelope(point: &MarkWeightedKPoint) -> io::Result<usize> {
    if point.inference_eligible {
        if point.weighted_k.is_none()
            || !point.lower_weighted_k.is_some_and(nonnegative_finite)
            || !point.upper_weighted_k.is_some_and(nonnegative_finite)
            || point.lower_weighted_k > point.upper_weighted_k
        {
            return Err(invalid("invalid mark-weighted K envelope"));
        }
        Ok(1)
    } else if point.lower_weighted_k.is_some() || point.upper_weighted_k.is_some() {
        Err(invalid("ineligible mark-weighted K contains an envelope"))
    } else {
        Ok(0)
    }
}

fn validate_component(
    value: Option<&MarkWeightedKComponentInference>,
    eligible: usize,
    permutations: usize,
) -> io::Result<()> {
    match value {
        Some(value)
            if eligible > 0
                && value.eligible_radius_count == eligible
                && unit_interval(value.p_global)
                && value.p_global >= 1.0 / permutations.saturating_add(1) as f64
                && unit_interval(value.erl_depth)
                && unit_interval(value.critical_depth) =>
        {
            Ok(())
        }
        None if eligible == 0 => Ok(()),
        _ => Err(invalid("invalid mark-weighted K inference")),
    }
}

fn status_name(value: MeasurementStatus) -> &'static str {
    match value {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

fn parse_status(value: &str) -> io::Result<MeasurementStatus> {
    match value {
        "measured" => Ok(MeasurementStatus::Measured),
        "imported_prediction" => Ok(MeasurementStatus::ImportedPrediction),
        "morphology_prediction" => Ok(MeasurementStatus::MorphologyPrediction),
        "derived_summary" => Ok(MeasurementStatus::DerivedSummary),
        _ => Err(invalid("invalid measurement status")),
    }
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
fn nonnegative_finite(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
fn same(left: f64, right: f64) -> bool {
    left == right
        || (left.is_finite()
            && right.is_finite()
            && (left - right).abs() <= 16.0 * f64::EPSILON * left.abs().max(right.abs()).max(1.0))
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
