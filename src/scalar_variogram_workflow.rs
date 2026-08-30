use std::{collections::BTreeSet, io, str::FromStr};

use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::{Deserialize, Serialize};

use crate::{
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
    scalar_variogram::pair_plan_digest,
    spatial_autocorrelation_workflow::window_artifact,
    workflow::pattern_artifact,
    ObservationWindow2D, ScalarVariogramBin, ScalarVariogramConditioning,
    ScalarVariogramEnvelopeRow, ScalarVariogramInferenceDesign, ScalarVariogramInferenceLimits,
    ScalarVariogramInferenceResult, ScalarVariogramResult, ScalarVariogramRow,
};

const RESULT_KIND: &str = "application/vnd.marklab.scalar-variogram-inference+json;version=1";
const NODE_KIND: &str = "scalar_variogram_inference";
const ADAPTER_REVISION: &str = "scalar-variogram-inference-workflow-v1";

/// Existing typed scalar-semivariogram inference exposed as one durable workflow node.
pub struct ScalarVariogramAnalysisNode<'input, 'pattern, 'window> {
    spec: NodeSpec,
    input: &'input DeclaredScalarPatternInput<'pattern>,
    window: &'window ObservationWindow2D,
    mark_id: ScalarMarkId,
    bins: Box<[ScalarVariogramBin]>,
    design: ScalarVariogramInferenceDesign,
    limits: ScalarVariogramInferenceLimits,
    maximum_retained_bytes: usize,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
    implementation_identity: String,
}

impl<'input, 'pattern, 'window> ScalarVariogramAnalysisNode<'input, 'pattern, 'window> {
    /// Bind the exact typed input, frame-bound window, lag bins, null, seed, and work ceilings.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'input DeclaredScalarPatternInput<'pattern>,
        window: &'window ObservationWindow2D,
        mark_id: &ScalarMarkId,
        bins: &[ScalarVariogramBin],
        design: &ScalarVariogramInferenceDesign,
        limits: ScalarVariogramInferenceLimits,
        maximum_retained_bytes: usize,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let window_ref = window_artifact(window)?;
        let required_bytes =
            retained_bytes(input.pattern().len(), bins.len(), design.permutations()).ok_or_else(
                || NodeError::input(invalid("scalar variogram retained-byte overflow")),
            )?;
        if maximum_retained_bytes == 0 || required_bytes > maximum_retained_bytes {
            return Err(NodeError::input(invalid(
                "scalar variogram retained-byte limit exceeded",
            )));
        }
        for artifact in [&pattern, &declared, &window_ref] {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        let configuration =
            configuration_bytes(mark_id, bins, design, limits, maximum_retained_bytes);
        let execution_policy = execution_policy(design, limits, maximum_retained_bytes);
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            input,
            window,
            mark_id: mark_id.clone(),
            bins: bins.to_vec().into_boxed_slice(),
            design: design.clone(),
            limits,
            maximum_retained_bytes,
            inputs: [pattern, declared, window_ref],
            configuration_digest: ContentDigest::from_bytes(&configuration),
            execution_policy,
            implementation_identity: format!(
                "marklab/{};adapter={ADAPTER_REVISION}",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Return the exact versioned source-node specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for ScalarVariogramAnalysisNode<'_, '_, '_> {
    type Output = ScalarVariogramInferenceResult;

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
            execution_policy: &self.execution_policy,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::scalar_semivariogram_permutation(
            self.input,
            self.window,
            &self.mark_id,
            &self.bins,
            &self.design,
            self.limits,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate_decoded(output).map_err(NodeError::encoding)?;
        crate::exact_float_json::encode(&ScalarVariogramWire::from(output))
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let wire: ScalarVariogramWire =
            crate::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        let result = ScalarVariogramInferenceResult::try_from(wire).map_err(NodeError::decode)?;
        self.validate_decoded(&result).map_err(NodeError::decode)?;
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

impl ScalarVariogramAnalysisNode<'_, '_, '_> {
    fn validate_decoded(&self, result: &ScalarVariogramInferenceResult) -> io::Result<()> {
        let table = self
            .input
            .mark_table()
            .ok_or_else(|| invalid("decoded scalar variogram has no typed mark table"))?;
        let expected_status = table
            .measurement_status(&self.mark_id)
            .ok_or_else(|| invalid("decoded scalar variogram mark is absent"))?;
        let expected_frame = self
            .window
            .coordinate_frame_id()
            .ok_or_else(|| invalid("decoded scalar variogram window is unframed"))?;
        let declared_digest = self
            .input
            .declared_artifact_ref()
            .map_err(invalid_owned)?
            .digest();
        let expected_pair_digest = pair_plan_digest(
            self.input,
            self.window,
            &self.mark_id,
            declared_digest,
            &self.bins,
        );
        let point_count = self.input.pattern().len();
        let pair_visits = point_count
            .checked_mul(point_count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| invalid("decoded scalar variogram pair count overflows"))?;
        let work = pair_visits
            .checked_mul(self.design.permutations())
            .ok_or_else(|| invalid("decoded scalar variogram work count overflows"))?;
        let (expected_conditioning_mark, expected_conditioning_status, expected_strata) =
            match self.design.conditioning() {
                ScalarVariogramConditioning::None => (None, None, 1),
                ScalarVariogramConditioning::HistologicCompartment => {
                    let id = ScalarMarkId::new("histologic_compartment")
                        .map_err(|_| invalid("invalid built-in conditioning mark ID"))?;
                    let codes = table
                        .categorical_values(&id)
                        .ok_or_else(|| invalid("conditioning categorical values are absent"))?;
                    let strata = codes.iter().copied().collect::<BTreeSet<_>>().len();
                    let status = table
                        .measurement_status(&id)
                        .ok_or_else(|| invalid("conditioning measurement status is absent"))?;
                    (Some(id), Some(status), strata)
                }
            };
        let observed_limits = self.limits.observed();
        if result.observed.mark_id != self.mark_id
            || result.observed.measurement_status != expected_status
            || &result.observed.coordinate_frame_id != expected_frame
            || result.observed.declared_input_digest != declared_digest
            || result.observed.pair_plan_digest != expected_pair_digest
            || result.observed.point_count != point_count
            || result.observed.pair_visits != pair_visits
            || result.observed.edge_correction != "none_fixed_observed_locations"
            || result.observed.inference_status != "observed_only_no_null"
            || point_count > observed_limits.maximum_points
            || pair_visits > observed_limits.maximum_pair_visits
            || work > self.limits.maximum_permutation_pair_evaluations()
            || retained_bytes(point_count, self.bins.len(), self.design.permutations())
                .is_none_or(|required| required > self.maximum_retained_bytes)
            || result.curve.len() != self.bins.len()
            || result.observed.curve.len() != self.bins.len()
            || result.permutations_requested != self.design.permutations()
            || result.permutations_completed != self.design.permutations()
            || result.seed != self.design.seed()
            || result.alpha.to_bits() != self.design.alpha().to_bits()
            || result.conditioning != self.design.conditioning()
            || result.conditioning_mark_id != expected_conditioning_mark
            || result.conditioning_measurement_status != expected_conditioning_status
            || result.stratum_count != expected_strata
            || result.multiplicity_policy != "two_sided_extreme_rank_length_global_envelope"
            || !result.p_global.is_finite()
            || !(0.0..=1.0).contains(&result.p_global)
            || !result.observed_erl_depth.is_finite()
            || !result.critical_erl_depth.is_finite()
        {
            return Err(invalid(
                "decoded scalar variogram does not match its cache-bound request",
            ));
        }
        let mut eligible = 0usize;
        let mut counted_pairs = 0usize;
        for (index, ((row, envelope), bin)) in result
            .observed
            .curve
            .iter()
            .zip(&result.curve)
            .zip(&self.bins)
            .enumerate()
        {
            if envelope.observed != *row
                || row.lower_um.to_bits() != bin.lower_um().to_bits()
                || row.upper_um.to_bits() != bin.upper_um().to_bits()
                || row.upper_inclusive != (index + 1 == self.bins.len())
                || row
                    .semivariance
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
                || envelope
                    .lower_global_envelope
                    .is_some_and(|value| !value.is_finite())
                || envelope
                    .upper_global_envelope
                    .is_some_and(|value| !value.is_finite())
                || envelope.lower_global_envelope.is_some()
                    != envelope.upper_global_envelope.is_some()
                || row.semivariance.is_some() != envelope.lower_global_envelope.is_some()
                || envelope
                    .lower_global_envelope
                    .zip(envelope.upper_global_envelope)
                    .is_some_and(|(lower, upper)| lower > upper)
            {
                return Err(invalid(
                    "decoded scalar variogram curve is inconsistent or non-finite",
                ));
            }
            counted_pairs = counted_pairs
                .checked_add(row.pair_count)
                .ok_or_else(|| invalid("decoded scalar variogram pair sum overflows"))?;
            eligible += usize::from(row.semivariance.is_some());
        }
        if counted_pairs > pair_visits || result.eligible_bin_count != eligible {
            return Err(invalid(
                "decoded scalar variogram bin totals are inconsistent",
            ));
        }
        Ok(())
    }
}

fn configuration_bytes(
    mark_id: &ScalarMarkId,
    bins: &[ScalarVariogramBin],
    design: &ScalarVariogramInferenceDesign,
    limits: ScalarVariogramInferenceLimits,
    maximum_retained_bytes: usize,
) -> Vec<u8> {
    let mut fields = vec![
        b"marklab-scalar-variogram-inference-config-v1".to_vec(),
        mark_id.as_str().as_bytes().to_vec(),
        conditioning_name(design.conditioning()).as_bytes().to_vec(),
        design.permutations().to_be_bytes().to_vec(),
        design.seed().to_be_bytes().to_vec(),
        design.alpha().to_bits().to_be_bytes().to_vec(),
        limits.observed().maximum_points.to_be_bytes().to_vec(),
        limits.observed().maximum_pair_visits.to_be_bytes().to_vec(),
        limits
            .maximum_permutation_pair_evaluations()
            .to_be_bytes()
            .to_vec(),
        maximum_retained_bytes.to_be_bytes().to_vec(),
    ];
    for bin in bins {
        fields.push(bin.lower_um().to_bits().to_be_bytes().to_vec());
        fields.push(bin.upper_um().to_bits().to_be_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
        .as_bytes()
        .to_vec()
}

fn execution_policy(
    design: &ScalarVariogramInferenceDesign,
    limits: ScalarVariogramInferenceLimits,
    maximum_retained_bytes: usize,
) -> Vec<u8> {
    format!(
        "fixed-observed-locations;whole-value-random-labeling;conditioning={};permutations={};seed={};erl-two-sided;max-points={};max-pairs={};max-permutation-pairs={};max-retained-bytes={}",
        conditioning_name(design.conditioning()),
        design.permutations(),
        design.seed(),
        limits.observed().maximum_points,
        limits.observed().maximum_pair_visits,
        limits.maximum_permutation_pair_evaluations(),
        maximum_retained_bytes,
    )
    .into_bytes()
}

fn retained_bytes(points: usize, bins: usize, permutations: usize) -> Option<usize> {
    let curves = permutations
        .checked_add(1)?
        .checked_mul(bins)?
        .checked_mul(16 * std::mem::size_of::<f64>())?;
    let rows = bins.checked_mul(
        std::mem::size_of::<ScalarVariogramRow>()
            + std::mem::size_of::<ScalarVariogramEnvelopeRow>()
            + 8 * std::mem::size_of::<f64>(),
    )?;
    let point_work =
        points.checked_mul(4 * std::mem::size_of::<f64>() + 4 * std::mem::size_of::<usize>())?;
    curves.checked_add(rows)?.checked_add(point_work)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScalarVariogramRowWire {
    lower_um: f64,
    upper_um: f64,
    upper_inclusive: bool,
    pair_count: u64,
    semivariance: Option<f64>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScalarVariogramEnvelopeWire {
    observed: ScalarVariogramRowWire,
    lower_global_envelope: Option<f64>,
    upper_global_envelope: Option<f64>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScalarVariogramWire {
    format: String,
    mark_id: String,
    measurement_status: String,
    coordinate_frame_id: String,
    declared_input_digest: String,
    pair_plan_digest: String,
    point_count: u64,
    pair_visits: u64,
    observed_curve: Vec<ScalarVariogramRowWire>,
    edge_correction: String,
    inference_status: String,
    curve: Vec<ScalarVariogramEnvelopeWire>,
    p_global: f64,
    observed_erl_depth: f64,
    critical_erl_depth: f64,
    alpha: f64,
    eligible_bin_count: u64,
    stratum_count: u64,
    conditioning_mark_id: Option<String>,
    conditioning_measurement_status: Option<String>,
    conditioning: String,
    permutations_requested: u64,
    permutations_completed: u64,
    seed: u64,
    multiplicity_policy: String,
}

impl From<&ScalarVariogramRow> for ScalarVariogramRowWire {
    fn from(value: &ScalarVariogramRow) -> Self {
        Self {
            lower_um: value.lower_um,
            upper_um: value.upper_um,
            upper_inclusive: value.upper_inclusive,
            pair_count: value.pair_count as u64,
            semivariance: value.semivariance,
        }
    }
}

impl TryFrom<ScalarVariogramRowWire> for ScalarVariogramRow {
    type Error = io::Error;

    fn try_from(value: ScalarVariogramRowWire) -> Result<Self, Self::Error> {
        Ok(Self {
            lower_um: value.lower_um,
            upper_um: value.upper_um,
            upper_inclusive: value.upper_inclusive,
            pair_count: to_usize(value.pair_count)?,
            semivariance: value.semivariance,
        })
    }
}

impl From<&ScalarVariogramInferenceResult> for ScalarVariogramWire {
    fn from(value: &ScalarVariogramInferenceResult) -> Self {
        Self {
            format: "marklab.scalar-semivariogram-inference/1".into(),
            mark_id: value.observed.mark_id.as_str().into(),
            measurement_status: status_name(value.observed.measurement_status).into(),
            coordinate_frame_id: value.observed.coordinate_frame_id.as_str().into(),
            declared_input_digest: value.observed.declared_input_digest.to_string(),
            pair_plan_digest: value.observed.pair_plan_digest.to_string(),
            point_count: value.observed.point_count as u64,
            pair_visits: value.observed.pair_visits as u64,
            observed_curve: value
                .observed
                .curve
                .iter()
                .map(ScalarVariogramRowWire::from)
                .collect(),
            edge_correction: value.observed.edge_correction.into(),
            inference_status: value.observed.inference_status.into(),
            curve: value
                .curve
                .iter()
                .map(|row| ScalarVariogramEnvelopeWire {
                    observed: ScalarVariogramRowWire::from(&row.observed),
                    lower_global_envelope: row.lower_global_envelope,
                    upper_global_envelope: row.upper_global_envelope,
                })
                .collect(),
            p_global: value.p_global,
            observed_erl_depth: value.observed_erl_depth,
            critical_erl_depth: value.critical_erl_depth,
            alpha: value.alpha,
            eligible_bin_count: value.eligible_bin_count as u64,
            stratum_count: value.stratum_count as u64,
            conditioning_mark_id: value
                .conditioning_mark_id
                .as_ref()
                .map(|id| id.as_str().into()),
            conditioning_measurement_status: value
                .conditioning_measurement_status
                .map(status_name)
                .map(str::to_owned),
            conditioning: conditioning_name(value.conditioning).into(),
            permutations_requested: value.permutations_requested as u64,
            permutations_completed: value.permutations_completed as u64,
            seed: value.seed,
            multiplicity_policy: value.multiplicity_policy.into(),
        }
    }
}

impl TryFrom<ScalarVariogramWire> for ScalarVariogramInferenceResult {
    type Error = io::Error;

    fn try_from(value: ScalarVariogramWire) -> Result<Self, Self::Error> {
        if value.format != "marklab.scalar-semivariogram-inference/1"
            || value.edge_correction != "none_fixed_observed_locations"
            || value.inference_status != "observed_only_no_null"
            || value.multiplicity_policy != "two_sided_extreme_rank_length_global_envelope"
        {
            return Err(invalid(
                "unsupported scalar variogram result format or policy",
            ));
        }
        let observed = ScalarVariogramResult {
            mark_id: ScalarMarkId::new(value.mark_id)
                .map_err(|_| invalid("invalid scalar variogram mark ID"))?,
            measurement_status: parse_status(&value.measurement_status)?,
            coordinate_frame_id: CoordinateFrameId::new(value.coordinate_frame_id)
                .map_err(|_| invalid("invalid scalar variogram coordinate frame ID"))?,
            declared_input_digest: ContentDigest::from_str(&value.declared_input_digest)
                .map_err(|_| invalid("invalid scalar variogram input digest"))?,
            pair_plan_digest: ContentDigest::from_str(&value.pair_plan_digest)
                .map_err(|_| invalid("invalid scalar variogram pair-plan digest"))?,
            point_count: to_usize(value.point_count)?,
            pair_visits: to_usize(value.pair_visits)?,
            curve: value
                .observed_curve
                .into_iter()
                .map(ScalarVariogramRow::try_from)
                .collect::<io::Result<_>>()?,
            edge_correction: "none_fixed_observed_locations",
            inference_status: "observed_only_no_null",
        };
        let curve = value
            .curve
            .into_iter()
            .map(|row| {
                Ok(ScalarVariogramEnvelopeRow {
                    observed: row.observed.try_into()?,
                    lower_global_envelope: row.lower_global_envelope,
                    upper_global_envelope: row.upper_global_envelope,
                })
            })
            .collect::<io::Result<_>>()?;
        Ok(ScalarVariogramInferenceResult {
            observed,
            curve,
            p_global: value.p_global,
            observed_erl_depth: value.observed_erl_depth,
            critical_erl_depth: value.critical_erl_depth,
            alpha: value.alpha,
            eligible_bin_count: to_usize(value.eligible_bin_count)?,
            stratum_count: to_usize(value.stratum_count)?,
            conditioning_mark_id: value
                .conditioning_mark_id
                .map(ScalarMarkId::new)
                .transpose()
                .map_err(|_| invalid("invalid scalar variogram conditioning mark ID"))?,
            conditioning_measurement_status: value
                .conditioning_measurement_status
                .as_deref()
                .map(parse_status)
                .transpose()?,
            conditioning: parse_conditioning(&value.conditioning)?,
            permutations_requested: to_usize(value.permutations_requested)?,
            permutations_completed: to_usize(value.permutations_completed)?,
            seed: value.seed,
            multiplicity_policy: "two_sided_extreme_rank_length_global_envelope",
        })
    }
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
        _ => Err(invalid("invalid scalar variogram measurement status")),
    }
}

fn conditioning_name(conditioning: ScalarVariogramConditioning) -> &'static str {
    match conditioning {
        ScalarVariogramConditioning::None => "none",
        ScalarVariogramConditioning::HistologicCompartment => "histologic_compartment",
    }
}

fn parse_conditioning(value: &str) -> io::Result<ScalarVariogramConditioning> {
    match value {
        "none" => Ok(ScalarVariogramConditioning::None),
        "histologic_compartment" => Ok(ScalarVariogramConditioning::HistologicCompartment),
        _ => Err(invalid("invalid scalar variogram conditioning")),
    }
}

fn to_usize(value: u64) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| invalid("scalar variogram count exceeds usize"))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
