use std::{io, str::FromStr};

use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeRun,
    NodeSpec, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use crate::{
    geom::window::ObservationWindow2D,
    global_moran_permutation,
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
    workflow::pattern_artifact,
    GlobalMoranAlternative, GlobalMoranConditioning, GlobalMoranDesign, GlobalMoranLimits,
    GlobalMoranResult, GlobalMoranWeightPolicy,
};

const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const MORAN_RESULT_KIND: &str = "application/vnd.marklab.global-moran+json;version=1";
const MORAN_PREPOST_KIND: &str = "application/vnd.marklab.global-moran-prepost+json;version=1";
const MORAN_ADAPTER_REVISION: &str = "global-moran-workflow-v1";
const MORAN_PREPOST_REVISION: &str = "global-moran-prepost-v1";

/// Typed global Moran analysis exposed as one dependency-free workflow node.
pub struct GlobalMoranAnalysisNode<'input, 'pattern, 'window, 'design> {
    spec: NodeSpec,
    input: &'input DeclaredScalarPatternInput<'pattern>,
    window: &'window ObservationWindow2D,
    mark_id: ScalarMarkId,
    radius_um: f64,
    weight_policy: GlobalMoranWeightPolicy,
    design: &'design GlobalMoranDesign,
    limits: GlobalMoranLimits,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
    implementation_identity: String,
}

impl<'input, 'pattern, 'window, 'design>
    GlobalMoranAnalysisNode<'input, 'pattern, 'window, 'design>
{
    /// Bind the exact typed scalar input, framed observation window, method controls, and limits.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'input DeclaredScalarPatternInput<'pattern>,
        window: &'window ObservationWindow2D,
        mark_id: ScalarMarkId,
        radius_um: f64,
        weight_policy: GlobalMoranWeightPolicy,
        design: &'design GlobalMoranDesign,
        limits: GlobalMoranLimits,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        if !radius_um.is_finite() || radius_um <= 0.0 {
            return Err(NodeError::input(invalid_data(
                "invalid global Moran radius",
            )));
        }
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let window_ref = window_artifact(window)?;
        for artifact in [&pattern, &declared, &window_ref] {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        let configuration = configuration_bytes(&mark_id, radius_um, weight_policy, design, limits);
        let execution_policy = execution_policy(design, limits);
        Ok(Self {
            spec: NodeSpec::new(id, "global_moran", 1, Vec::new()).map_err(NodeError::input)?,
            input,
            window,
            mark_id,
            radius_um,
            weight_policy,
            design,
            limits,
            inputs: [pattern, declared, window_ref],
            configuration_digest: ContentDigest::from_bytes(&configuration),
            execution_policy,
            implementation_identity: format!(
                "marklab/{};adapter={MORAN_ADAPTER_REVISION}",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Return the exact versioned source-node specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for GlobalMoranAnalysisNode<'_, '_, '_, '_> {
    type Output = GlobalMoranResult;

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
            .map_err(NodeError::input)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: &self.execution_policy,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        global_moran_permutation(
            self.input,
            self.window,
            &self.mark_id,
            self.radius_um,
            self.weight_policy,
            self.design,
            self.limits,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_moran(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let result = decode_moran(bytes).map_err(NodeError::decode)?;
        self.validate_decoded(&result).map_err(NodeError::decode)?;
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        MORAN_RESULT_KIND
    }
}

impl GlobalMoranAnalysisNode<'_, '_, '_, '_> {
    fn validate_decoded(&self, result: &GlobalMoranResult) -> io::Result<()> {
        let expected_status = self
            .input
            .mark_table()
            .and_then(|table| table.measurement_status(&self.mark_id))
            .ok_or_else(|| invalid_data("decoded global Moran mark is not in the typed table"))?;
        let expected_frame = self.window.coordinate_frame_id().ok_or_else(|| {
            invalid_data("decoded global Moran result belongs to an unframed window")
        })?;
        let expected_conditioning_mark = match self.design.conditioning() {
            GlobalMoranConditioning::None => None,
            GlobalMoranConditioning::HistologicCompartment => Some(
                ScalarMarkId::new("histologic_compartment")
                    .map_err(|_| invalid_data("invalid built-in conditioning mark ID"))?,
            ),
        };
        let expected_conditioning_status = expected_conditioning_mark
            .as_ref()
            .and_then(|mark_id| self.input.mark_table()?.measurement_status(mark_id));
        let permutation_edge_evaluations = result
            .directed_edge_count
            .checked_mul(result.permutations_requested)
            .ok_or_else(|| invalid_data("decoded global Moran work count overflows"))?;
        if result.mark_id != self.mark_id
            || result.measurement_status != expected_status
            || &result.coordinate_frame_id != expected_frame
            || result.radius_um.to_bits() != self.radius_um.to_bits()
            || result.weight_policy != self.weight_policy
            || result.point_count != self.input.pattern().len()
            || result.permutations_requested != self.design.permutations()
            || result.permutations_attempted != self.design.permutations()
            || result.permutations_completed != self.design.permutations()
            || result.seed != self.design.seed()
            || result.alternative != self.design.alternative()
            || result.conditioning != self.design.conditioning()
            || result.conditioning_mark_id != expected_conditioning_mark
            || result.conditioning_measurement_status != expected_conditioning_status
            || result.point_count < 2
            || result.directed_edge_count == 0
            || result.stratum_count == 0
            || result.directed_edge_count > self.limits.maximum_directed_edges
            || permutation_edge_evaluations > self.limits.maximum_permutation_edge_evaluations
            || !result.statistic.is_finite()
            || !result.null_expectation.is_finite()
            || !result.p_value.is_finite()
            || !(0.0..=1.0).contains(&result.p_value)
        {
            return Err(invalid_data(
                "decoded global Moran result does not match the cache-bound request",
            ));
        }
        Ok(())
    }
}

/// Descriptive comparison of two exact typed global Moran results.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalMoranPrePostResult {
    /// Exact pre result supplied by the first dependency.
    pub pre: GlobalMoranResult,
    /// Exact post result supplied by the second dependency.
    pub post: GlobalMoranResult,
    /// Descriptive post-minus-pre difference in global Moran's I.
    pub statistic_delta_post_minus_pre: f64,
}

/// Dependency-bearing descriptive comparison of two produced global Moran artifacts.
pub struct GlobalMoranPrePostNode<'a> {
    spec: NodeSpec,
    pre: &'a GlobalMoranResult,
    post: &'a GlobalMoranResult,
    inputs: [ArtifactRef; 2],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> GlobalMoranPrePostNode<'a> {
    /// Bind two compatible produced Moran runs in explicit pre/post order.
    pub fn new(
        id: NodeId,
        pre_dependency: NodeId,
        pre: &'a NodeRun<GlobalMoranResult>,
        post_dependency: NodeId,
        post: &'a NodeRun<GlobalMoranResult>,
    ) -> Result<Self, NodeError> {
        verify_moran_artifact(&pre.artifact, &pre.output)?;
        verify_moran_artifact(&post.artifact, &post.output)?;
        validate_compatible(&pre.output, &post.output).map_err(NodeError::input)?;
        Ok(Self {
            spec: NodeSpec::new(
                id,
                "global_moran_prepost",
                1,
                vec![pre_dependency, post_dependency],
            )
            .map_err(NodeError::input)?,
            pre: &pre.output,
            post: &post.output,
            inputs: [pre.artifact.clone(), post.artifact.clone()],
            configuration_digest: ContentDigest::from_bytes(
                b"marklab-global-moran-prepost-descriptive-v1",
            ),
            implementation_identity: format!(
                "marklab/{};adapter={MORAN_PREPOST_REVISION}",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Return the exact versioned dependency-bearing specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for GlobalMoranPrePostNode<'_> {
    type Output = GlobalMoranPrePostResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        verify_moran_artifact(&self.inputs[0], self.pre)?;
        verify_moran_artifact(&self.inputs[1], self.post)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: b"descriptive-post-minus-pre;no-paired-inference",
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        let delta = self.post.statistic - self.pre.statistic;
        if !delta.is_finite() {
            return Err(NodeError::execution(invalid_data(
                "global Moran pre/post difference is non-finite",
            )));
        }
        Ok(GlobalMoranPrePostResult {
            pre: self.pre.clone(),
            post: self.post.clone(),
            statistic_delta_post_minus_pre: delta,
        })
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_prepost(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let result = decode_prepost(bytes).map_err(NodeError::decode)?;
        if result.pre != *self.pre
            || result.post != *self.post
            || result.statistic_delta_post_minus_pre.to_bits()
                != (self.post.statistic - self.pre.statistic).to_bits()
        {
            return Err(NodeError::decode(invalid_data(
                "decoded Moran pre/post result does not match its dependencies",
            )));
        }
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        MORAN_PREPOST_KIND
    }
}

fn validate_compatible(pre: &GlobalMoranResult, post: &GlobalMoranResult) -> io::Result<()> {
    if pre.mark_id != post.mark_id
        || pre.measurement_status != post.measurement_status
        || pre.coordinate_frame_id != post.coordinate_frame_id
        || pre.radius_um.to_bits() != post.radius_um.to_bits()
        || pre.weight_policy != post.weight_policy
        || pre.permutations_requested != post.permutations_requested
        || pre.seed != post.seed
        || pre.alternative != post.alternative
        || pre.conditioning != post.conditioning
        || pre.conditioning_mark_id != post.conditioning_mark_id
        || pre.conditioning_measurement_status != post.conditioning_measurement_status
    {
        return Err(invalid_data(
            "pre and post global Moran runs have incompatible estimands or controls",
        ));
    }
    Ok(())
}

fn verify_moran_artifact(
    artifact: &ArtifactRef,
    result: &GlobalMoranResult,
) -> Result<(), NodeError> {
    if artifact.kind() != MORAN_RESULT_KIND {
        return Err(NodeError::input(invalid_data(
            "dependency is not a global Moran result artifact",
        )));
    }
    let encoded = encode_moran(result).map_err(NodeError::input)?;
    let expected =
        ArtifactRef::from_bytes(MORAN_RESULT_KIND, &encoded).map_err(NodeError::input)?;
    artifact
        .verify_identity(expected.digest(), expected.byte_len())
        .map_err(NodeError::input)
}

fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let descriptor = window.descriptor();
    let frame = descriptor
        .coordinate_frame_id
        .as_ref()
        .ok_or_else(|| NodeError::input(invalid_data("observation window is not frame-bound")))?;
    let fields = [
        b"marklab-observation-window-ref-v1".as_slice(),
        descriptor.logical_digest.as_bytes(),
        frame.as_str().as_bytes(),
    ];
    let digest = ContentDigest::from_framed(fields);
    ArtifactRef::from_bytes(WINDOW_KIND, digest.as_bytes()).map_err(NodeError::input)
}

fn configuration_bytes(
    mark_id: &ScalarMarkId,
    radius_um: f64,
    weight_policy: GlobalMoranWeightPolicy,
    design: &GlobalMoranDesign,
    limits: GlobalMoranLimits,
) -> Vec<u8> {
    format!(
        "mark={};radius_bits={:016x};weights={};conditioning={};permutations={};seed={};alternative={};max_points={};max_edges={};max_permutation_edges={}",
        mark_id.as_str(),
        radius_um.to_bits(),
        weight_name(weight_policy),
        conditioning_name(design.conditioning()),
        design.permutations(),
        design.seed(),
        alternative_name(design.alternative()),
        limits.maximum_points,
        limits.maximum_directed_edges,
        limits.maximum_permutation_edge_evaluations,
    )
    .into_bytes()
}

fn execution_policy(design: &GlobalMoranDesign, limits: GlobalMoranLimits) -> Vec<u8> {
    format!(
        "deterministic-random-labeling;permutations={};seed={};max_points={};max_edges={};max_permutation_edges={}",
        design.permutations(),
        design.seed(),
        limits.maximum_points,
        limits.maximum_directed_edges,
        limits.maximum_permutation_edge_evaluations,
    )
    .into_bytes()
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MoranWire {
    format: String,
    mark_id: String,
    measurement_status: String,
    coordinate_frame_id: String,
    radius_um: f64,
    weight_policy: String,
    weights_digest: String,
    point_count: u64,
    directed_edge_count: u64,
    stratum_count: u64,
    conditioning_mark_id: Option<String>,
    conditioning_measurement_status: Option<String>,
    statistic: f64,
    null_expectation: f64,
    p_value: f64,
    permutations_requested: u64,
    permutations_attempted: u64,
    permutations_completed: u64,
    seed: u64,
    alternative: String,
    conditioning: String,
}

impl From<&GlobalMoranResult> for MoranWire {
    fn from(result: &GlobalMoranResult) -> Self {
        Self {
            format: "marklab.global-moran/1".into(),
            mark_id: result.mark_id.as_str().into(),
            measurement_status: status_name(result.measurement_status).into(),
            coordinate_frame_id: result.coordinate_frame_id.as_str().into(),
            radius_um: result.radius_um,
            weight_policy: weight_name(result.weight_policy).into(),
            weights_digest: result.weights_digest.to_string(),
            point_count: result.point_count as u64,
            directed_edge_count: result.directed_edge_count as u64,
            stratum_count: result.stratum_count as u64,
            conditioning_mark_id: result
                .conditioning_mark_id
                .as_ref()
                .map(|id| id.as_str().into()),
            conditioning_measurement_status: result
                .conditioning_measurement_status
                .map(status_name)
                .map(str::to_owned),
            statistic: result.statistic,
            null_expectation: result.null_expectation,
            p_value: result.p_value,
            permutations_requested: result.permutations_requested as u64,
            permutations_attempted: result.permutations_attempted as u64,
            permutations_completed: result.permutations_completed as u64,
            seed: result.seed,
            alternative: alternative_name(result.alternative).into(),
            conditioning: conditioning_name(result.conditioning).into(),
        }
    }
}

impl TryFrom<MoranWire> for GlobalMoranResult {
    type Error = io::Error;

    fn try_from(wire: MoranWire) -> Result<Self, Self::Error> {
        if wire.format != "marklab.global-moran/1" {
            return Err(invalid_data("unsupported global Moran result format"));
        }
        Ok(Self {
            mark_id: ScalarMarkId::new(wire.mark_id)
                .map_err(|_| invalid_data("invalid global Moran mark ID"))?,
            measurement_status: parse_status(&wire.measurement_status)?,
            coordinate_frame_id: CoordinateFrameId::new(wire.coordinate_frame_id)
                .map_err(|_| invalid_data("invalid global Moran coordinate frame ID"))?,
            radius_um: wire.radius_um,
            weight_policy: parse_weight(&wire.weight_policy)?,
            weights_digest: ContentDigest::from_str(&wire.weights_digest)
                .map_err(|_| invalid_data("invalid global Moran weights digest"))?,
            point_count: to_usize(wire.point_count)?,
            directed_edge_count: to_usize(wire.directed_edge_count)?,
            stratum_count: to_usize(wire.stratum_count)?,
            conditioning_mark_id: wire
                .conditioning_mark_id
                .map(ScalarMarkId::new)
                .transpose()
                .map_err(|_| invalid_data("invalid conditioning mark ID"))?,
            conditioning_measurement_status: wire
                .conditioning_measurement_status
                .as_deref()
                .map(parse_status)
                .transpose()?,
            statistic: wire.statistic,
            null_expectation: wire.null_expectation,
            p_value: wire.p_value,
            permutations_requested: to_usize(wire.permutations_requested)?,
            permutations_attempted: to_usize(wire.permutations_attempted)?,
            permutations_completed: to_usize(wire.permutations_completed)?,
            seed: wire.seed,
            alternative: parse_alternative(&wire.alternative)?,
            conditioning: parse_conditioning(&wire.conditioning)?,
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrePostWire {
    format: String,
    pre: MoranWire,
    post: MoranWire,
    statistic_delta_post_minus_pre: f64,
}

fn encode_moran(result: &GlobalMoranResult) -> Result<Box<[u8]>, serde_json::Error> {
    serde_json::to_vec(&MoranWire::from(result)).map(Vec::into_boxed_slice)
}

fn decode_moran(bytes: &[u8]) -> io::Result<GlobalMoranResult> {
    serde_json::from_slice::<MoranWire>(bytes)
        .map_err(|error| invalid_data(error.to_string()))?
        .try_into()
}

fn encode_prepost(result: &GlobalMoranPrePostResult) -> Result<Box<[u8]>, serde_json::Error> {
    serde_json::to_vec(&PrePostWire {
        format: "marklab.global-moran-prepost/1".into(),
        pre: MoranWire::from(&result.pre),
        post: MoranWire::from(&result.post),
        statistic_delta_post_minus_pre: result.statistic_delta_post_minus_pre,
    })
    .map(Vec::into_boxed_slice)
}

fn decode_prepost(bytes: &[u8]) -> io::Result<GlobalMoranPrePostResult> {
    let wire: PrePostWire =
        serde_json::from_slice(bytes).map_err(|error| invalid_data(error.to_string()))?;
    if wire.format != "marklab.global-moran-prepost/1" {
        return Err(invalid_data("unsupported global Moran pre/post format"));
    }
    let pre = GlobalMoranResult::try_from(wire.pre)?;
    let post = GlobalMoranResult::try_from(wire.post)?;
    validate_compatible(&pre, &post)?;
    let expected_delta = post.statistic - pre.statistic;
    if !wire.statistic_delta_post_minus_pre.is_finite()
        || wire.statistic_delta_post_minus_pre.to_bits() != expected_delta.to_bits()
    {
        return Err(invalid_data("invalid global Moran pre/post difference"));
    }
    Ok(GlobalMoranPrePostResult {
        pre,
        post,
        statistic_delta_post_minus_pre: wire.statistic_delta_post_minus_pre,
    })
}

fn status_name(status: MeasurementStatus) -> &'static str {
    match status {
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
        _ => Err(invalid_data("invalid measurement status")),
    }
}

fn weight_name(policy: GlobalMoranWeightPolicy) -> &'static str {
    match policy {
        GlobalMoranWeightPolicy::BinarySymmetric => "binary_symmetric",
        GlobalMoranWeightPolicy::RowStandardized => "row_standardized",
    }
}

fn parse_weight(value: &str) -> io::Result<GlobalMoranWeightPolicy> {
    match value {
        "binary_symmetric" => Ok(GlobalMoranWeightPolicy::BinarySymmetric),
        "row_standardized" => Ok(GlobalMoranWeightPolicy::RowStandardized),
        _ => Err(invalid_data("invalid global Moran weight policy")),
    }
}

fn alternative_name(alternative: GlobalMoranAlternative) -> &'static str {
    match alternative {
        GlobalMoranAlternative::Less => "less",
        GlobalMoranAlternative::Greater => "greater",
        GlobalMoranAlternative::TwoSided => "two_sided",
    }
}

fn parse_alternative(value: &str) -> io::Result<GlobalMoranAlternative> {
    match value {
        "less" => Ok(GlobalMoranAlternative::Less),
        "greater" => Ok(GlobalMoranAlternative::Greater),
        "two_sided" => Ok(GlobalMoranAlternative::TwoSided),
        _ => Err(invalid_data("invalid global Moran alternative")),
    }
}

fn conditioning_name(conditioning: GlobalMoranConditioning) -> &'static str {
    match conditioning {
        GlobalMoranConditioning::None => "none",
        GlobalMoranConditioning::HistologicCompartment => "histologic_compartment",
    }
}

fn parse_conditioning(value: &str) -> io::Result<GlobalMoranConditioning> {
    match value {
        "none" => Ok(GlobalMoranConditioning::None),
        "histologic_compartment" => Ok(GlobalMoranConditioning::HistologicCompartment),
        _ => Err(invalid_data("invalid global Moran conditioning")),
    }
}

fn to_usize(value: u64) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| invalid_data("global Moran count exceeds platform size"))
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
