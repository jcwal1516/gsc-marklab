use crate::result_vocabulary::{
    EdgeCorrection, MarkNormalization, MarkNullModel, MarkPermutationUnit, MultiplicityCorrection,
};
use crate::workflow_codec::{invalid, nearly_equal, parse_status, unit_interval};
use crate::workflow_node::{NodeAnalysis, TypedNode};
use std::{io, str::FromStr};

use marklab_data::CoordinateFrameId;
use marklab_project::{ArtifactRef, ContentDigest, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use serde::{Deserialize, Serialize};

use crate::measurement_status_wire::name as status_name;
use crate::observation_window_artifact::frame_bound_window_artifact as window_artifact;
use crate::{
    continuous_mark_correlation::{configuration_digest, mark_statistics},
    marked_workflow::pattern_artifact,
    marks::{
        ContinuousMarkCorrelationComponentInference, ContinuousMarkCorrelationConfig,
        ContinuousMarkCorrelationGeometrySummary, ContinuousMarkCorrelationInferenceSummary,
        ContinuousMarkCorrelationLimits, ContinuousMarkCorrelationPoint,
        ContinuousMarkCorrelationPointStatus, ContinuousMarkCorrelationResult,
        DeclaredScalarPatternInput, ScalarMarkId,
    },
    spatial::ClassicalWindowSummary,
    ObservationWindow2D,
};

const NODE_KIND: &str = "continuous_mark_correlation";
const CONFIG_KIND: &str =
    "application/vnd.marklab.continuous-mark-correlation-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.continuous-mark-correlation-result+json;version=2";
const EXECUTION_POLICY: &[u8] = b"serial;exact-rstar;retained-directed-pairs;standard-border;global-arithmetic-mean-product;complete-continuous-row-random-labeling;erl";
const IMPLEMENTATION: &str = "continuous-mark-correlation-analysis-node-v3";

/// Exact typed continuous mark-correlation node with store-verified MarkTable provenance.
pub struct ContinuousMarkCorrelationAnalysisNode<'a> {
    node: TypedNode<ContinuousMarkCorrelationAnalysis<'a>>,
}

struct ContinuousMarkCorrelationAnalysis<'a> {
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    config: &'a ContinuousMarkCorrelationConfig,
}

impl<'a> ContinuousMarkCorrelationAnalysisNode<'a> {
    /// Bind exact typed marks, physical window, normalization, null controls, and limits.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        config: &'a ContinuousMarkCorrelationConfig,
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
        let node_spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let node_configuration_digest = config_ref.digest();
        let node_implementation_identity = format!(
            "marklab/{};adapter={IMPLEMENTATION}",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec: node_spec,
                analysis: ContinuousMarkCorrelationAnalysis {
                    input,
                    window,
                    config,
                },
                inputs,
                configuration_digest: node_configuration_digest,
                execution_policy: std::borrow::Cow::Borrowed(EXECUTION_POLICY),
                implementation_identity: node_implementation_identity,
            },
        })
    }

    /// Exact versioned node specification.
    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for ContinuousMarkCorrelationAnalysis<'_> {
    type Output = ContinuousMarkCorrelationResult;
    type Inputs = [ArtifactRef; 4];

    fn semantic_input_artifacts(&self) -> &[marklab_project::ArtifactId] {
        self.input.semantic_artifact_ids()
    }

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.input.pattern())?;
        crate::workflow_node::verify_input(inputs, 0, &pattern)?;
        let declared = self
            .input
            .declared_artifact_ref()
            .map_err(NodeError::input)?;
        crate::workflow_node::verify_input(inputs, 1, &declared)?;
        let window = window_artifact(self.window)?;
        crate::workflow_node::verify_input(inputs, 2, &window)?;
        let config = config_artifact(self.config)?;
        crate::workflow_node::verify_input(inputs, 3, &config)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::marks::continuous_mark_correlation(self.input, self.window, self.config)
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
        let result = ContinuousMarkCorrelationResult::try_from(wire).map_err(NodeError::decode)?;
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
            || !nearly_equal(result.global_mark_mean, mean)
            || !nearly_equal(result.global_mark_population_variance, variance)
            || !nearly_equal(result.expected_random_label_correlation, expected)
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
                "decoded continuous mark-correlation result does not match its cache-bound request",
            )));
        }
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(ContinuousMarkCorrelationAnalysisNode<'_> => ContinuousMarkCorrelationResult);

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    radii_um: &'a [f64],
    normalization: MarkNormalization,
    permutations: usize,
    seed: u64,
    alpha: f64,
    limits: LimitsWire,
    logical_digest: String,
}

fn config_artifact(config: &ContinuousMarkCorrelationConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        mark_id: config.mark_id().as_str(),
        radii_um: config.radii_um(),
        normalization: MarkNormalization::GlobalArithmeticMeanProduct,
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

impl From<ContinuousMarkCorrelationLimits> for LimitsWire {
    fn from(value: ContinuousMarkCorrelationLimits) -> Self {
        Self {
            maximum_points: value.maximum_points,
            maximum_radii: value.maximum_radii,
            maximum_directed_pairs: value.maximum_directed_pairs,
            maximum_permutation_pair_evaluations: value.maximum_permutation_pair_evaluations,
            maximum_retained_bytes: value.maximum_retained_bytes,
        }
    }
}

impl TryFrom<LimitsWire> for ContinuousMarkCorrelationLimits {
    type Error = io::Error;

    fn try_from(value: LimitsWire) -> Result<Self, Self::Error> {
        ContinuousMarkCorrelationLimits::new(
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
    normalization: MarkNormalization,
    global_mark_mean: f64,
    global_mark_population_variance: f64,
    expected_random_label_correlation: f64,
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
    geometry_digest: String,
    pair_plan_digest: String,
    edge_correction: EdgeCorrection,
    estimated_storage_bytes: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PointWire {
    radius_um: f64,
    shell_lower_um: f64,
    status: String,
    directed_pairs_in_shell: usize,
    mark_product_sum_in_shell: f64,
    correlation: Option<f64>,
    inference_eligible: bool,
    lower_correlation: Option<f64>,
    upper_correlation: Option<f64>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InferenceWire {
    null_model: MarkNullModel,
    permutation_unit: MarkPermutationUnit,
    #[serde(with = "crate::result_vocabulary::AlternativeWire")]
    alternative: marklab_cohort::InferenceAlternative,
    multiplicity: MultiplicityCorrection,
    correlation: Option<ComponentWire>,
    permutations_requested: usize,
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

impl From<&ContinuousMarkCorrelationResult> for ResultWire {
    fn from(value: &ContinuousMarkCorrelationResult) -> Self {
        Self {
            format: "marklab.continuous-mark-correlation/1".into(),
            mark_id: value.mark_id.as_str().into(),
            measurement_status: status_name(value.measurement_status).into(),
            unit: value.unit.clone(),
            coordinate_frame_id: value.coordinate_frame_id.as_str().into(),
            window: value.window.clone(),
            normalization: value.normalization,
            global_mark_mean: value.global_mark_mean,
            global_mark_population_variance: value.global_mark_population_variance,
            expected_random_label_correlation: value.expected_random_label_correlation,
            geometry: GeometryWire {
                point_count: value.geometry.point_count,
                directed_pair_count: value.geometry.directed_pair_count,
                geometry_digest: value.geometry.geometry_digest.to_string(),
                pair_plan_digest: value.geometry.pair_plan_digest.to_string(),
                edge_correction: value.geometry.edge_correction,
                estimated_storage_bytes: value.geometry.estimated_storage_bytes,
            },
            configuration_digest: value.configuration_digest.to_string(),
            limits: value.limits.into(),
            curve: value.curve.iter().map(PointWire::from).collect(),
            inference: InferenceWire::from(&value.inference),
        }
    }
}

impl From<&ContinuousMarkCorrelationPoint> for PointWire {
    fn from(value: &ContinuousMarkCorrelationPoint) -> Self {
        Self {
            radius_um: value.radius_um,
            shell_lower_um: value.shell_lower_um,
            status: match value.status {
                ContinuousMarkCorrelationPointStatus::Available => "available",
                ContinuousMarkCorrelationPointStatus::NoPairsInShell => "no_pairs_in_shell",
            }
            .into(),
            directed_pairs_in_shell: value.directed_pairs_in_shell,
            mark_product_sum_in_shell: value.mark_product_sum_in_shell,
            correlation: value.correlation,
            inference_eligible: value.inference_eligible,
            lower_correlation: value.lower_correlation,
            upper_correlation: value.upper_correlation,
        }
    }
}

impl From<&ContinuousMarkCorrelationInferenceSummary> for InferenceWire {
    fn from(value: &ContinuousMarkCorrelationInferenceSummary) -> Self {
        Self {
            null_model: value.null_model,
            permutation_unit: value.permutation_unit,
            alternative: value.alternative,
            multiplicity: value.multiplicity,
            correlation: value.correlation.as_ref().map(ComponentWire::from),
            permutations_requested: value.permutations_requested,
            seed: value.seed,
            alpha: value.alpha,
            permutation_pair_evaluations: value.permutation_pair_evaluations,
        }
    }
}

impl From<&ContinuousMarkCorrelationComponentInference> for ComponentWire {
    fn from(value: &ContinuousMarkCorrelationComponentInference) -> Self {
        Self {
            p_global: value.p_global,
            erl_depth: value.erl_depth,
            critical_depth: value.critical_depth,
            eligible_radius_count: value.eligible_radius_count,
        }
    }
}

impl TryFrom<ResultWire> for ContinuousMarkCorrelationResult {
    type Error = io::Error;

    fn try_from(value: ResultWire) -> Result<Self, Self::Error> {
        if value.format != "marklab.continuous-mark-correlation/1" {
            return Err(invalid(
                "unsupported continuous mark-correlation result format",
            ));
        }
        Ok(Self {
            mark_id: ScalarMarkId::new(value.mark_id)
                .map_err(|error| invalid(error.to_string()))?,
            measurement_status: parse_status(&value.measurement_status)?,
            unit: value.unit,
            coordinate_frame_id: CoordinateFrameId::new(value.coordinate_frame_id)
                .map_err(|error| invalid(error.to_string()))?,
            window: value.window,
            normalization: value.normalization,
            global_mark_mean: value.global_mark_mean,
            global_mark_population_variance: value.global_mark_population_variance,
            expected_random_label_correlation: value.expected_random_label_correlation,
            geometry: ContinuousMarkCorrelationGeometrySummary {
                point_count: value.geometry.point_count,
                directed_pair_count: value.geometry.directed_pair_count,
                geometry_digest: ContentDigest::from_str(&value.geometry.geometry_digest)
                    .map_err(|error| invalid(error.to_string()))?,
                pair_plan_digest: ContentDigest::from_str(&value.geometry.pair_plan_digest)
                    .map_err(|error| invalid(error.to_string()))?,
                edge_correction: value.geometry.edge_correction,
                estimated_storage_bytes: value.geometry.estimated_storage_bytes,
            },
            configuration_digest: ContentDigest::from_str(&value.configuration_digest)
                .map_err(|error| invalid(error.to_string()))?,
            limits: value.limits.try_into()?,
            curve: value
                .curve
                .into_iter()
                .map(ContinuousMarkCorrelationPoint::try_from)
                .collect::<Result<_, _>>()?,
            inference: value.inference.into(),
        })
    }
}

impl TryFrom<PointWire> for ContinuousMarkCorrelationPoint {
    type Error = io::Error;

    fn try_from(value: PointWire) -> Result<Self, Self::Error> {
        let status = match value.status.as_str() {
            "available" => ContinuousMarkCorrelationPointStatus::Available,
            "no_pairs_in_shell" => ContinuousMarkCorrelationPointStatus::NoPairsInShell,
            _ => return Err(invalid("invalid continuous mark-correlation point status")),
        };
        Ok(Self {
            radius_um: value.radius_um,
            shell_lower_um: value.shell_lower_um,
            status,
            directed_pairs_in_shell: value.directed_pairs_in_shell,
            mark_product_sum_in_shell: value.mark_product_sum_in_shell,
            correlation: value.correlation,
            inference_eligible: value.inference_eligible,
            lower_correlation: value.lower_correlation,
            upper_correlation: value.upper_correlation,
        })
    }
}

impl From<InferenceWire> for ContinuousMarkCorrelationInferenceSummary {
    fn from(value: InferenceWire) -> Self {
        Self {
            null_model: value.null_model,
            permutation_unit: value.permutation_unit,
            alternative: value.alternative,
            multiplicity: value.multiplicity,
            correlation: value.correlation.map(Into::into),
            permutations_requested: value.permutations_requested,
            seed: value.seed,
            alpha: value.alpha,
            permutation_pair_evaluations: value.permutation_pair_evaluations,
        }
    }
}

impl From<ComponentWire> for ContinuousMarkCorrelationComponentInference {
    fn from(value: ComponentWire) -> Self {
        Self {
            p_global: value.p_global,
            erl_depth: value.erl_depth,
            critical_depth: value.critical_depth,
            eligible_radius_count: value.eligible_radius_count,
        }
    }
}

fn validate_result(result: &ContinuousMarkCorrelationResult) -> io::Result<()> {
    if result.mark_id.as_str() != "nucleus_area_um2"
        || result.unit != "square_micrometer"
        || result.normalization != MarkNormalization::GlobalArithmeticMeanProduct
        || !result.global_mark_mean.is_finite()
        || result.global_mark_mean <= 0.0
        || !result.global_mark_population_variance.is_finite()
        || result.global_mark_population_variance <= 0.0
        || !result.expected_random_label_correlation.is_finite()
        || result.expected_random_label_correlation <= 0.0
        || result.expected_random_label_correlation > 1.0
        || result.geometry.point_count < 2
        || result.geometry.directed_pair_count > result.limits.maximum_directed_pairs
        || result.geometry.edge_correction != EdgeCorrection::StandardBorderReducedSample
        || result.geometry.estimated_storage_bytes > result.limits.maximum_retained_bytes
        || result.curve.is_empty()
        || result.curve.len() > result.limits.maximum_radii
        || result.inference.permutations_requested == 0
        || result.inference.permutation_pair_evaluations
            > result.limits.maximum_permutation_pair_evaluations
        || result.inference.null_model != MarkNullModel::RandomLabeling
        || result.inference.permutation_unit != MarkPermutationUnit::CompleteContinuousMarkRow
        || result.inference.alternative != marklab_cohort::InferenceAlternative::TwoSided
        || result.inference.multiplicity != MultiplicityCorrection::SingleCorrelationErlFamily
        || !result.inference.alpha.is_finite()
        || result.inference.alpha <= 0.0
        || result.inference.alpha >= 1.0
        || (result.inference.permutations_requested.saturating_add(1) as f64)
            * result.inference.alpha
            < 1.0
        || result.inference.permutation_pair_evaluations
            != result
                .geometry
                .directed_pair_count
                .saturating_mul(result.inference.permutations_requested)
    {
        return Err(invalid(
            "invalid continuous mark-correlation result metadata",
        ));
    }
    let normalization = result.global_mark_mean * result.global_mark_mean;
    let mut previous = 0.0;
    let mut eligible = 0;
    for point in &result.curve {
        if !point.radius_um.is_finite()
            || point.radius_um <= previous
            || point.shell_lower_um.to_bits() != previous.to_bits()
            || !point.mark_product_sum_in_shell.is_finite()
            || point.mark_product_sum_in_shell < 0.0
        {
            return Err(invalid(
                "invalid continuous mark-correlation curve identity",
            ));
        }
        previous = point.radius_um;
        match point.status {
            ContinuousMarkCorrelationPointStatus::Available => {
                let Some(correlation) = point.correlation else {
                    return Err(invalid("available continuous correlation lacks a value"));
                };
                if point.directed_pairs_in_shell == 0
                    || !correlation.is_finite()
                    || correlation < 0.0
                    || !nearly_equal(
                        correlation,
                        point.mark_product_sum_in_shell
                            / point.directed_pairs_in_shell as f64
                            / normalization,
                    )
                {
                    return Err(invalid(
                        "invalid normalized continuous correlation identity",
                    ));
                }
            }
            ContinuousMarkCorrelationPointStatus::NoPairsInShell => {
                if point.directed_pairs_in_shell != 0
                    || point.mark_product_sum_in_shell != 0.0
                    || point.correlation.is_some()
                {
                    return Err(invalid(
                        "empty continuous correlation shell contains a value",
                    ));
                }
            }
        }
        eligible += validate_envelope(point)?;
    }
    validate_component(
        result.inference.correlation.as_ref(),
        eligible,
        result.inference.permutations_requested,
    )
}

fn validate_envelope(point: &ContinuousMarkCorrelationPoint) -> io::Result<usize> {
    if point.inference_eligible {
        if point.correlation.is_none()
            || !point.lower_correlation.is_some_and(nonnegative_finite)
            || !point.upper_correlation.is_some_and(nonnegative_finite)
            || point.lower_correlation > point.upper_correlation
        {
            return Err(invalid("invalid continuous mark-correlation envelope"));
        }
        Ok(1)
    } else if point.lower_correlation.is_some() || point.upper_correlation.is_some() {
        Err(invalid(
            "ineligible continuous correlation radius contains an envelope",
        ))
    } else {
        Ok(0)
    }
}

fn validate_component(
    value: Option<&ContinuousMarkCorrelationComponentInference>,
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
        _ => Err(invalid("invalid continuous mark-correlation inference")),
    }
}

fn nonnegative_finite(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
