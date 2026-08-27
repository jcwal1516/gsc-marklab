use std::{io, str::FromStr};

use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::{Deserialize, Serialize};

use crate::{
    categorical_pair::{configuration_digest, CategoricalPairConfig},
    workflow::pattern_artifact,
    CategoricalPairComponentInference, CategoricalPairGeometrySummary,
    CategoricalPairInferenceSummary, CategoricalPairLimits, CategoricalPairPoint,
    CategoricalPairPointStatus, CategoricalPairResult, ClassicalWindowSummary,
    DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

const NODE_KIND: &str = "categorical_mark_connection_cross_k";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str = "application/vnd.marklab.categorical-pair-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.categorical-pair-result+json;version=1";
const EXECUTION_POLICY: &[u8] = b"serial;exact-rstar;retained-directed-pairs;standard-border;complete-row-random-labeling;two-component-erl";
const IMPLEMENTATION: &str = "categorical-pair-analysis-node-v1";

/// Exact typed categorical pair workflow node with store-verified MarkTable provenance.
pub struct CategoricalPairAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    config: &'a CategoricalPairConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> CategoricalPairAnalysisNode<'a> {
    /// Bind exact typed marks, physical window, level pair, null controls, and limits.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        config: &'a CategoricalPairConfig,
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

    /// Return the exact versioned node specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for CategoricalPairAnalysisNode<'_> {
    type Output = CategoricalPairResult;

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
        crate::categorical_mark_connection_cross_k(self.input, self.window, self.config)
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
        let result = CategoricalPairResult::try_from(wire).map_err(NodeError::decode)?;
        validate_result(&result).map_err(NodeError::decode)?;
        let table = self
            .input
            .mark_table()
            .ok_or_else(|| NodeError::decode(invalid("typed MarkTable is absent")))?;
        let mark_id = ScalarMarkId::new("histologic_compartment").map_err(NodeError::decode)?;
        let levels = table
            .categorical_levels(&mark_id)
            .ok_or_else(|| NodeError::decode(invalid("decoded categorical levels are absent")))?;
        let codes = table
            .categorical_values(&mark_id)
            .ok_or_else(|| NodeError::decode(invalid("decoded categorical values are absent")))?;
        let source_code = levels
            .iter()
            .position(|level| level == self.config.source_level())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| NodeError::decode(invalid("cache-bound source level is absent")))?;
        let target_code = levels
            .iter()
            .position(|level| level == self.config.target_level())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| NodeError::decode(invalid("cache-bound target level is absent")))?;
        let descriptor = self.window.descriptor();
        if result.configuration_digest != configuration_digest(self.config)
            || result.source_level != self.config.source_level()
            || result.target_level != self.config.target_level()
            || result.limits != self.config.limits()
            || result.inference.permutations_requested != self.config.permutations()
            || result.inference.seed != self.config.seed()
            || result.inference.alpha.to_bits() != self.config.alpha().to_bits()
            || result.coordinate_frame_id != *self.input.coordinate_frame_id()
            || result.mark_id != mark_id
            || result.measurement_status
                != table.measurement_status(&result.mark_id).ok_or_else(|| {
                    NodeError::decode(invalid("decoded categorical mark is absent"))
                })?
            || result.source_count != codes.iter().filter(|code| **code == source_code).count()
            || result.target_count != codes.iter().filter(|code| **code == target_code).count()
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
                "decoded categorical pair result does not match its cache-bound request",
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
    radii_um: &'a [f64],
    source_level: &'a str,
    target_level: &'a str,
    permutations: usize,
    seed: u64,
    alpha: f64,
    limits: LimitsWire,
    logical_digest: String,
}

fn config_artifact(config: &CategoricalPairConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        source_level: config.source_level(),
        target_level: config.target_level(),
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

impl From<CategoricalPairLimits> for LimitsWire {
    fn from(value: CategoricalPairLimits) -> Self {
        Self {
            maximum_points: value.maximum_points,
            maximum_radii: value.maximum_radii,
            maximum_directed_pairs: value.maximum_directed_pairs,
            maximum_permutation_pair_evaluations: value.maximum_permutation_pair_evaluations,
            maximum_retained_bytes: value.maximum_retained_bytes,
        }
    }
}

impl TryFrom<LimitsWire> for CategoricalPairLimits {
    type Error = io::Error;

    fn try_from(value: LimitsWire) -> Result<Self, Self::Error> {
        CategoricalPairLimits::new(
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
    coordinate_frame_id: String,
    window: ClassicalWindowSummary,
    source_level: String,
    target_level: String,
    source_count: usize,
    target_count: usize,
    expected_random_label_connection: f64,
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
    edge_correction: String,
    estimated_storage_bytes: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PointWire {
    radius_um: f64,
    shell_lower_um: f64,
    status: String,
    eligible_source_centers: usize,
    directed_source_target_pairs: usize,
    directed_pairs_in_shell: usize,
    source_target_pairs_in_shell: usize,
    connection_probability: Option<f64>,
    cross_k: Option<f64>,
    theoretical_cross_k: f64,
    connection_inference_eligible: bool,
    lower_connection: Option<f64>,
    upper_connection: Option<f64>,
    cross_k_inference_eligible: bool,
    lower_cross_k: Option<f64>,
    upper_cross_k: Option<f64>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InferenceWire {
    null_model: String,
    permutation_unit: String,
    alternative: String,
    multiplicity: String,
    connection: Option<ComponentWire>,
    cross_k: Option<ComponentWire>,
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

impl From<&CategoricalPairResult> for ResultWire {
    fn from(value: &CategoricalPairResult) -> Self {
        Self {
            format: "marklab.categorical-pair/1".into(),
            mark_id: value.mark_id.as_str().into(),
            measurement_status: status_name(value.measurement_status).into(),
            coordinate_frame_id: value.coordinate_frame_id.as_str().into(),
            window: value.window.clone(),
            source_level: value.source_level.clone(),
            target_level: value.target_level.clone(),
            source_count: value.source_count,
            target_count: value.target_count,
            expected_random_label_connection: value.expected_random_label_connection,
            geometry: GeometryWire {
                point_count: value.geometry.point_count,
                directed_pair_count: value.geometry.directed_pair_count,
                geometry_build_count: value.geometry.geometry_build_count,
                geometry_digest: value.geometry.geometry_digest.to_string(),
                pair_plan_digest: value.geometry.pair_plan_digest.to_string(),
                edge_correction: value.geometry.edge_correction.clone(),
                estimated_storage_bytes: value.geometry.estimated_storage_bytes,
            },
            configuration_digest: value.configuration_digest.to_string(),
            limits: value.limits.into(),
            curve: value.curve.iter().map(PointWire::from).collect(),
            inference: InferenceWire::from(&value.inference),
        }
    }
}

impl From<&CategoricalPairPoint> for PointWire {
    fn from(value: &CategoricalPairPoint) -> Self {
        Self {
            radius_um: value.radius_um,
            shell_lower_um: value.shell_lower_um,
            status: match value.status {
                CategoricalPairPointStatus::Available => "available",
                CategoricalPairPointStatus::NoPairsInShell => "no_pairs_in_shell",
            }
            .into(),
            eligible_source_centers: value.eligible_source_centers,
            directed_source_target_pairs: value.directed_source_target_pairs,
            directed_pairs_in_shell: value.directed_pairs_in_shell,
            source_target_pairs_in_shell: value.source_target_pairs_in_shell,
            connection_probability: value.connection_probability,
            cross_k: value.cross_k,
            theoretical_cross_k: value.theoretical_cross_k,
            connection_inference_eligible: value.connection_inference_eligible,
            lower_connection: value.lower_connection,
            upper_connection: value.upper_connection,
            cross_k_inference_eligible: value.cross_k_inference_eligible,
            lower_cross_k: value.lower_cross_k,
            upper_cross_k: value.upper_cross_k,
        }
    }
}

impl From<&CategoricalPairInferenceSummary> for InferenceWire {
    fn from(value: &CategoricalPairInferenceSummary) -> Self {
        Self {
            null_model: value.null_model.clone(),
            permutation_unit: value.permutation_unit.clone(),
            alternative: value.alternative.clone(),
            multiplicity: value.multiplicity.clone(),
            connection: value.connection.as_ref().map(ComponentWire::from),
            cross_k: value.cross_k.as_ref().map(ComponentWire::from),
            permutations_requested: value.permutations_requested,
            permutations_attempted: value.permutations_attempted,
            permutations_completed: value.permutations_completed,
            seed: value.seed,
            alpha: value.alpha,
            permutation_pair_evaluations: value.permutation_pair_evaluations,
        }
    }
}

impl From<&CategoricalPairComponentInference> for ComponentWire {
    fn from(value: &CategoricalPairComponentInference) -> Self {
        Self {
            p_global: value.p_global,
            erl_depth: value.erl_depth,
            critical_depth: value.critical_depth,
            eligible_radius_count: value.eligible_radius_count,
        }
    }
}

impl TryFrom<ResultWire> for CategoricalPairResult {
    type Error = io::Error;

    fn try_from(value: ResultWire) -> Result<Self, Self::Error> {
        if value.format != "marklab.categorical-pair/1" {
            return Err(invalid("unsupported categorical pair result format"));
        }
        Ok(Self {
            mark_id: ScalarMarkId::new(value.mark_id)
                .map_err(|error| invalid(error.to_string()))?,
            measurement_status: parse_status(&value.measurement_status)?,
            coordinate_frame_id: CoordinateFrameId::new(value.coordinate_frame_id)
                .map_err(|error| invalid(error.to_string()))?,
            window: value.window,
            source_level: value.source_level,
            target_level: value.target_level,
            source_count: value.source_count,
            target_count: value.target_count,
            expected_random_label_connection: value.expected_random_label_connection,
            geometry: CategoricalPairGeometrySummary {
                point_count: value.geometry.point_count,
                directed_pair_count: value.geometry.directed_pair_count,
                geometry_build_count: value.geometry.geometry_build_count,
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
                .map(CategoricalPairPoint::try_from)
                .collect::<Result<_, _>>()?,
            inference: value.inference.try_into()?,
        })
    }
}

impl TryFrom<PointWire> for CategoricalPairPoint {
    type Error = io::Error;

    fn try_from(value: PointWire) -> Result<Self, Self::Error> {
        let status = match value.status.as_str() {
            "available" => CategoricalPairPointStatus::Available,
            "no_pairs_in_shell" => CategoricalPairPointStatus::NoPairsInShell,
            _ => return Err(invalid("invalid categorical pair point status")),
        };
        Ok(Self {
            radius_um: value.radius_um,
            shell_lower_um: value.shell_lower_um,
            status,
            eligible_source_centers: value.eligible_source_centers,
            directed_source_target_pairs: value.directed_source_target_pairs,
            directed_pairs_in_shell: value.directed_pairs_in_shell,
            source_target_pairs_in_shell: value.source_target_pairs_in_shell,
            connection_probability: value.connection_probability,
            cross_k: value.cross_k,
            theoretical_cross_k: value.theoretical_cross_k,
            connection_inference_eligible: value.connection_inference_eligible,
            lower_connection: value.lower_connection,
            upper_connection: value.upper_connection,
            cross_k_inference_eligible: value.cross_k_inference_eligible,
            lower_cross_k: value.lower_cross_k,
            upper_cross_k: value.upper_cross_k,
        })
    }
}

impl TryFrom<InferenceWire> for CategoricalPairInferenceSummary {
    type Error = io::Error;

    fn try_from(value: InferenceWire) -> Result<Self, Self::Error> {
        Ok(Self {
            null_model: value.null_model,
            permutation_unit: value.permutation_unit,
            alternative: value.alternative,
            multiplicity: value.multiplicity,
            connection: value.connection.map(Into::into),
            cross_k: value.cross_k.map(Into::into),
            permutations_requested: value.permutations_requested,
            permutations_attempted: value.permutations_attempted,
            permutations_completed: value.permutations_completed,
            seed: value.seed,
            alpha: value.alpha,
            permutation_pair_evaluations: value.permutation_pair_evaluations,
        })
    }
}

impl From<ComponentWire> for CategoricalPairComponentInference {
    fn from(value: ComponentWire) -> Self {
        Self {
            p_global: value.p_global,
            erl_depth: value.erl_depth,
            critical_depth: value.critical_depth,
            eligible_radius_count: value.eligible_radius_count,
        }
    }
}

fn validate_result(result: &CategoricalPairResult) -> io::Result<()> {
    if result.mark_id.as_str() != "histologic_compartment"
        || result.source_level == result.target_level
        || result.source_count == 0
        || result.target_count == 0
        || result.source_count.saturating_add(result.target_count) > result.geometry.point_count
        || result.geometry.point_count < result.source_count.max(result.target_count)
        || result.geometry.directed_pair_count > result.limits.maximum_directed_pairs
        || result.geometry.geometry_build_count != 1
        || result.geometry.edge_correction != "standard_border_reduced_sample"
        || result.geometry.estimated_storage_bytes > result.limits.maximum_retained_bytes
        || result.curve.is_empty()
        || result.curve.len() > result.limits.maximum_radii
        || result.inference.permutations_requested == 0
        || result.inference.permutations_attempted != result.inference.permutations_requested
        || result.inference.permutations_completed != result.inference.permutations_requested
        || result.inference.permutation_pair_evaluations
            > result.limits.maximum_permutation_pair_evaluations
        || result.inference.null_model != "random_labeling"
        || result.inference.permutation_unit != "complete_categorical_row"
        || result.inference.alternative != "two_sided"
        || result.inference.multiplicity != "separate_componentwise_erl_families"
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
        || !unit_interval(result.expected_random_label_connection)
    {
        return Err(invalid("invalid categorical pair result metadata"));
    }
    let mut previous = 0.0;
    let mut connection_eligible = 0;
    let mut cross_k_eligible = 0;
    for point in &result.curve {
        if !point.radius_um.is_finite()
            || point.radius_um <= previous
            || point.shell_lower_um.to_bits() != previous.to_bits()
            || point.source_target_pairs_in_shell > point.directed_pairs_in_shell
            || point.directed_source_target_pairs > result.geometry.directed_pair_count
            || !same(
                point.theoretical_cross_k,
                std::f64::consts::PI * point.radius_um.powi(2),
            )
        {
            return Err(invalid("invalid categorical pair curve identity"));
        }
        previous = point.radius_um;
        match point.status {
            CategoricalPairPointStatus::Available => {
                let Some(connection) = point.connection_probability else {
                    return Err(invalid("available connection lacks a value"));
                };
                if point.directed_pairs_in_shell == 0
                    || !same(
                        connection,
                        point.source_target_pairs_in_shell as f64
                            / point.directed_pairs_in_shell as f64,
                    )
                {
                    return Err(invalid("invalid mark-connection identity"));
                }
            }
            CategoricalPairPointStatus::NoPairsInShell => {
                if point.directed_pairs_in_shell != 0 || point.connection_probability.is_some() {
                    return Err(invalid("empty connection shell contains a value"));
                }
            }
        }
        if let Some(cross_k) = point.cross_k {
            if point.eligible_source_centers == 0 {
                return Err(invalid("directed cross-K has no eligible source"));
            }
            let denominator = point
                .eligible_source_centers
                .checked_mul(result.target_count)
                .ok_or_else(|| invalid("categorical pair count overflow"))?;
            let expected = result.window.area_um2 * point.directed_source_target_pairs as f64
                / denominator as f64;
            if !cross_k.is_finite() || cross_k < 0.0 || !same(cross_k, expected) {
                return Err(invalid("invalid directed cross-K value"));
            }
        } else if point.eligible_source_centers != 0 {
            return Err(invalid("directed cross-K is missing with eligible sources"));
        }
        connection_eligible += validate_envelope(
            point.connection_inference_eligible,
            point.lower_connection,
            point.upper_connection,
            point.connection_probability,
        )?;
        cross_k_eligible += validate_envelope(
            point.cross_k_inference_eligible,
            point.lower_cross_k,
            point.upper_cross_k,
            point.cross_k,
        )?;
    }
    let expected_connection_denominator = result
        .geometry
        .point_count
        .checked_mul(result.geometry.point_count.saturating_sub(1))
        .ok_or_else(|| invalid("categorical pair count overflow"))?;
    let expected_connection = result.source_count as f64 * result.target_count as f64
        / expected_connection_denominator as f64;
    if !same(result.expected_random_label_connection, expected_connection) {
        return Err(invalid("invalid random-label connection expectation"));
    }
    validate_component(
        result.inference.connection.as_ref(),
        connection_eligible,
        result.inference.permutations_requested,
    )?;
    validate_component(
        result.inference.cross_k.as_ref(),
        cross_k_eligible,
        result.inference.permutations_requested,
    )?;
    Ok(())
}

fn validate_envelope(
    eligible: bool,
    lower: Option<f64>,
    upper: Option<f64>,
    observed: Option<f64>,
) -> io::Result<usize> {
    if eligible {
        if observed.is_none()
            || !lower.is_some_and(f64::is_finite)
            || !upper.is_some_and(f64::is_finite)
            || lower > upper
        {
            return Err(invalid("invalid categorical pair envelope"));
        }
        Ok(1)
    } else if lower.is_some() || upper.is_some() {
        Err(invalid(
            "ineligible categorical pair radius contains an envelope",
        ))
    } else {
        Ok(0)
    }
}

fn validate_component(
    value: Option<&CategoricalPairComponentInference>,
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
        _ => Err(invalid("invalid categorical pair component inference")),
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
