use std::collections::BTreeMap;

use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_numerics::extreme_rank_length_envelope;
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::{
    geom::window::ObservationWindow2D,
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
};

/// One half-open physical lag interval `[lower_um, upper_um)`, except the final bin includes its
/// upper edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalarVariogramBin {
    lower_um: f64,
    upper_um: f64,
}

impl ScalarVariogramBin {
    /// Validate one finite nonnegative increasing lag interval.
    pub fn new(lower_um: f64, upper_um: f64) -> Result<Self, ScalarVariogramError> {
        if !lower_um.is_finite() || !upper_um.is_finite() || lower_um < 0.0 || upper_um <= lower_um
        {
            return Err(ScalarVariogramError::InvalidBins);
        }
        Ok(Self {
            lower_um: normalize_zero(lower_um),
            upper_um: normalize_zero(upper_um),
        })
    }

    /// Inclusive lower lag in micrometres.
    pub fn lower_um(self) -> f64 {
        self.lower_um
    }

    /// Upper lag in micrometres, inclusive only for the final declared bin.
    pub fn upper_um(self) -> f64 {
        self.upper_um
    }
}

/// Explicit point and unordered-pair ceilings for one observed scalar semivariogram.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarVariogramLimits {
    /// Maximum point rows.
    pub maximum_points: usize,
    /// Maximum unordered pair visits, including pairs outside the declared lag range.
    pub maximum_pair_visits: usize,
}

impl ScalarVariogramLimits {
    /// Validate positive point and pair-visit ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_pair_visits: usize,
    ) -> Result<Self, ScalarVariogramError> {
        if maximum_points == 0 || maximum_pair_visits == 0 {
            return Err(ScalarVariogramError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_pair_visits,
        })
    }
}

/// Whole-value random-labeling controls for one scalar-semivariogram global envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramInferenceDesign {
    conditioning: ScalarVariogramConditioning,
    permutations: usize,
    seed: u64,
    alpha: f64,
}

impl ScalarVariogramInferenceDesign {
    /// Declare unstratified whole-value random labeling across fixed cell locations.
    pub fn random_labeling(
        permutations: usize,
        seed: u64,
        alpha: f64,
    ) -> Result<Self, ScalarVariogramError> {
        Self::new(ScalarVariogramConditioning::None, permutations, seed, alpha)
    }

    /// Declare whole-value random labeling within exact typed histologic compartments.
    pub fn histologic_compartment_random_labeling(
        permutations: usize,
        seed: u64,
        alpha: f64,
    ) -> Result<Self, ScalarVariogramError> {
        Self::new(
            ScalarVariogramConditioning::HistologicCompartment,
            permutations,
            seed,
            alpha,
        )
    }

    fn new(
        conditioning: ScalarVariogramConditioning,
        permutations: usize,
        seed: u64,
        alpha: f64,
    ) -> Result<Self, ScalarVariogramError> {
        let curve_count = permutations
            .checked_add(1)
            .ok_or(ScalarVariogramError::SizeOverflow)?;
        if permutations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || curve_count as f64 * alpha < 1.0
        {
            return Err(ScalarVariogramError::InvalidInferenceDesign);
        }
        Ok(Self {
            conditioning,
            permutations,
            seed,
            alpha,
        })
    }

    /// Declared random-labeling conditioning.
    pub fn conditioning(&self) -> ScalarVariogramConditioning {
        self.conditioning
    }

    /// Requested deterministic permutation count.
    pub fn permutations(&self) -> usize {
        self.permutations
    }

    /// Base deterministic seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Family-wise alpha for the two-sided ERL global envelope.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
}

/// Conditioning admitted for scalar-semivariogram random labeling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarVariogramConditioning {
    /// Permute complete scalar values across every row.
    None,
    /// Permute complete scalar values only within exact histologic-compartment codes.
    HistologicCompartment,
}

/// Resource ceilings for observed and permuted scalar-semivariogram evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarVariogramInferenceLimits {
    observed: ScalarVariogramLimits,
    maximum_permutation_pair_evaluations: usize,
}

impl ScalarVariogramInferenceLimits {
    /// Validate point, all-pair, and permutation-by-pair ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_pair_visits: usize,
        maximum_permutation_pair_evaluations: usize,
    ) -> Result<Self, ScalarVariogramError> {
        if maximum_permutation_pair_evaluations == 0 {
            return Err(ScalarVariogramError::InvalidResourceLimit);
        }
        Ok(Self {
            observed: ScalarVariogramLimits::new(maximum_points, maximum_pair_visits)?,
            maximum_permutation_pair_evaluations,
        })
    }

    pub(crate) fn observed(self) -> ScalarVariogramLimits {
        self.observed
    }

    pub(crate) fn maximum_permutation_pair_evaluations(self) -> usize {
        self.maximum_permutation_pair_evaluations
    }
}

/// One observed scalar semivariance lag row.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramRow {
    /// Inclusive lower lag in micrometres.
    pub lower_um: f64,
    /// Upper lag in micrometres.
    pub upper_um: f64,
    /// Whether this row includes its upper edge; true only for the final row.
    pub upper_inclusive: bool,
    /// Exact unordered pair count in this bin.
    pub pair_count: usize,
    /// Half the mean squared scalar difference, absent for an empty bin.
    pub semivariance: Option<f64>,
}

/// Complete bounded observed scalar semivariogram.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramResult {
    /// Stable continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical coordinate frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Exact typed MarkTable/row/provenance/value identity.
    pub declared_input_digest: ContentDigest,
    /// Canonical identity of rows, coordinates, window, mark, and lag bins.
    pub pair_plan_digest: ContentDigest,
    /// Number of point rows.
    pub point_count: usize,
    /// Exact unordered pairs visited, including pairs outside the lag range.
    pub pair_visits: usize,
    /// Explicit observed-only curve.
    pub curve: Vec<ScalarVariogramRow>,
    /// Explicit correction status for this first fixed-location descriptive curve.
    pub edge_correction: &'static str,
    /// Explicit inference status; no null distribution is implied.
    pub inference_status: &'static str,
}

/// One observed semivariance row with its simultaneous global envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramEnvelopeRow {
    /// Exact observed lag row.
    pub observed: ScalarVariogramRow,
    /// Lower simultaneous ERL bound, absent for an empty bin.
    pub lower_global_envelope: Option<f64>,
    /// Upper simultaneous ERL bound, absent for an empty bin.
    pub upper_global_envelope: Option<f64>,
}

/// Complete blocked-permutation scalar-semivariogram global inference result.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramInferenceResult {
    /// Unchanged exact observed scalar-semivariogram result.
    pub observed: ScalarVariogramResult,
    /// Observed rows with simultaneous bounds on nonempty bins.
    pub curve: Vec<ScalarVariogramEnvelopeRow>,
    /// Inclusive-plus-one two-sided ERL global p-value.
    pub p_global: f64,
    /// Observed extreme-rank-length depth.
    pub observed_erl_depth: f64,
    /// Critical depth defining the retained envelope curves.
    pub critical_erl_depth: f64,
    /// Family-wise envelope alpha.
    pub alpha: f64,
    /// Number of nonempty bins included in the curve family.
    pub eligible_bin_count: usize,
    /// Number of exact exchangeability strata.
    pub stratum_count: usize,
    /// Typed categorical conditioning mark, absent when unstratified.
    pub conditioning_mark_id: Option<ScalarMarkId>,
    /// Conditioning-mark measurement status, absent when unstratified.
    pub conditioning_measurement_status: Option<MeasurementStatus>,
    /// Explicit random-labeling conditioning.
    pub conditioning: ScalarVariogramConditioning,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Successfully completed permutations.
    pub permutations_completed: usize,
    /// Exact base seed.
    pub seed: u64,
    /// Explicit curve-family multiplicity policy.
    pub multiplicity_policy: &'static str,
}

/// Compute an observed scalar semivariogram in exact caller-declared physical lag bins.
pub fn scalar_semivariogram(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    bins: &[ScalarVariogramBin],
    limits: ScalarVariogramLimits,
) -> Result<ScalarVariogramResult, ScalarVariogramError> {
    validate_bins(bins)?;
    let frame = window
        .coordinate_frame_id()
        .ok_or(ScalarVariogramError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(ScalarVariogramError::CoordinateFrameMismatch {
            expected: input.coordinate_frame_id().clone(),
            observed: frame.clone(),
        });
    }
    let pattern = input.pattern();
    let point_count = pattern.len();
    if point_count < 2 {
        return Err(ScalarVariogramError::InsufficientPoints);
    }
    if point_count > limits.maximum_points {
        return Err(ScalarVariogramError::PointLimitExceeded {
            observed: point_count,
            maximum: limits.maximum_points,
        });
    }
    let pair_visits = point_count
        .checked_mul(point_count - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(ScalarVariogramError::SizeOverflow)?;
    if pair_visits > limits.maximum_pair_visits {
        return Err(ScalarVariogramError::PairVisitLimitExceeded {
            observed: pair_visits,
            maximum: limits.maximum_pair_visits,
        });
    }
    validate_points(pattern.x_um.as_ref(), pattern.y_um.as_ref(), window)?;
    let table = input
        .mark_table()
        .ok_or(ScalarVariogramError::TypedMarkTableRequired)?;
    let values = table.continuous_values(mark_id).ok_or_else(|| {
        ScalarVariogramError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    if values.len() != point_count {
        return Err(ScalarVariogramError::RowCountMismatch {
            expected: point_count,
            observed: values.len(),
        });
    }
    let measurement_status = table.measurement_status(mark_id).ok_or_else(|| {
        ScalarVariogramError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    let declared_input_digest = input
        .declared_artifact_ref()
        .map_err(|error| ScalarVariogramError::InvalidInput(error.to_string()))?
        .digest();

    let values = values
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let curve = evaluate_curve(&pattern.x_um, &pattern.y_um, &values, bins)?;

    Ok(ScalarVariogramResult {
        mark_id: mark_id.clone(),
        measurement_status,
        coordinate_frame_id: frame.clone(),
        declared_input_digest,
        pair_plan_digest: pair_plan_digest(input, window, mark_id, declared_input_digest, bins),
        point_count,
        pair_visits,
        curve,
        edge_correction: "none_fixed_observed_locations",
        inference_status: "observed_only_no_null",
    })
}

/// Compute a deterministic blocked whole-value ERL envelope for the scalar semivariogram.
pub fn scalar_semivariogram_permutation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    bins: &[ScalarVariogramBin],
    design: &ScalarVariogramInferenceDesign,
    limits: ScalarVariogramInferenceLimits,
) -> Result<ScalarVariogramInferenceResult, ScalarVariogramError> {
    let observed = scalar_semivariogram(input, window, mark_id, bins, limits.observed)?;
    let work = observed
        .pair_visits
        .checked_mul(design.permutations)
        .ok_or(ScalarVariogramError::SizeOverflow)?;
    if work > limits.maximum_permutation_pair_evaluations {
        return Err(ScalarVariogramError::PermutationWorkExceeded {
            observed: work,
            maximum: limits.maximum_permutation_pair_evaluations,
        });
    }
    let table = input
        .mark_table()
        .ok_or(ScalarVariogramError::TypedMarkTableRequired)?;
    let values = table
        .continuous_values(mark_id)
        .ok_or_else(|| ScalarVariogramError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        })?
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let (inference_design, conditioning_mark_id, conditioning_measurement_status) =
        match design.conditioning {
            ScalarVariogramConditioning::None => {
                if !has_distinct_values(&values) {
                    return Err(ScalarVariogramError::DegenerateNull);
                }
                (
                    InferenceDesign::random_labeling(
                        values.len(),
                        design.permutations,
                        design.seed,
                        InferenceAlternative::TwoSided,
                    )
                    .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?,
                    None,
                    None,
                )
            }
            ScalarVariogramConditioning::HistologicCompartment => {
                let compartment_id = ScalarMarkId::new("histologic_compartment")
                    .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?;
                let compartments = table
                    .categorical_values(&compartment_id)
                    .ok_or(ScalarVariogramError::MissingCompartmentStratum)?;
                if compartments.len() != values.len() {
                    return Err(ScalarVariogramError::RowCountMismatch {
                        expected: values.len(),
                        observed: compartments.len(),
                    });
                }
                if !has_stratified_distinct_values(&values, compartments) {
                    return Err(ScalarVariogramError::DegenerateNull);
                }
                let status = table
                    .measurement_status(&compartment_id)
                    .ok_or(ScalarVariogramError::MissingCompartmentStratum)?;
                (
                    InferenceDesign::stratified_random_labeling(
                        compartments,
                        design.permutations,
                        design.seed,
                        InferenceAlternative::TwoSided,
                    )
                    .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?,
                    Some(compartment_id),
                    Some(status),
                )
            }
        };
    let eligible = observed
        .curve
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.semivariance.map(|value| (index, value)))
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return Err(ScalarVariogramError::NoEligibleBins);
    }
    let observed_eligible = eligible.iter().map(|(_, value)| *value).collect::<Vec<_>>();
    let pattern = input.pattern();
    let mut null_curves = Vec::with_capacity(design.permutations);
    for replicate in 0..design.permutations {
        let indices = inference_design
            .permuted_indices(replicate)
            .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?;
        let permuted = indices
            .iter()
            .map(|source| values[*source])
            .collect::<Vec<_>>();
        let curve = evaluate_curve(&pattern.x_um, &pattern.y_um, &permuted, bins)?;
        null_curves.push(
            eligible
                .iter()
                .map(|(index, _)| {
                    curve[*index]
                        .semivariance
                        .ok_or(ScalarVariogramError::NoEligibleBins)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let envelope = extreme_rank_length_envelope(&observed_eligible, &null_curves, design.alpha)
        .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?;
    let mut eligible_position = vec![None; bins.len()];
    for (position, (index, _)) in eligible.iter().enumerate() {
        eligible_position[*index] = Some(position);
    }
    let curve = observed
        .curve
        .iter()
        .enumerate()
        .map(|(index, row)| ScalarVariogramEnvelopeRow {
            observed: row.clone(),
            lower_global_envelope: eligible_position[index]
                .map(|position| envelope.lower[position]),
            upper_global_envelope: eligible_position[index]
                .map(|position| envelope.upper[position]),
        })
        .collect();

    Ok(ScalarVariogramInferenceResult {
        observed,
        curve,
        p_global: envelope.p_global,
        observed_erl_depth: envelope.observed_depth,
        critical_erl_depth: envelope.critical_depth,
        alpha: design.alpha,
        eligible_bin_count: eligible.len(),
        stratum_count: inference_design.block_count(),
        conditioning_mark_id,
        conditioning_measurement_status,
        conditioning: design.conditioning,
        permutations_requested: design.permutations,
        permutations_completed: design.permutations,
        seed: design.seed,
        multiplicity_policy: "two_sided_extreme_rank_length_global_envelope",
    })
}

fn evaluate_curve(
    x: &[f64],
    y: &[f64],
    values: &[f64],
    bins: &[ScalarVariogramBin],
) -> Result<Vec<ScalarVariogramRow>, ScalarVariogramError> {
    if x.len() != y.len() || x.len() != values.len() {
        return Err(ScalarVariogramError::RowCountMismatch {
            expected: x.len(),
            observed: values.len(),
        });
    }
    let mut counts = vec![0_usize; bins.len()];
    let mut sums = vec![CompensatedSum::default(); bins.len()];
    for left in 0..x.len() {
        for right in (left + 1)..x.len() {
            let distance = (x[left] - x[right]).hypot(y[left] - y[right]);
            if let Some(bin) = find_bin(distance, bins) {
                let difference = values[left] - values[right];
                let contribution = 0.5 * difference * difference;
                if !contribution.is_finite() {
                    return Err(ScalarVariogramError::NumericalFailure);
                }
                counts[bin] = counts[bin]
                    .checked_add(1)
                    .ok_or(ScalarVariogramError::SizeOverflow)?;
                sums[bin].add(contribution);
            }
        }
    }
    bins.iter()
        .enumerate()
        .map(|(index, bin)| {
            let semivariance = if counts[index] == 0 {
                None
            } else {
                let value = sums[index].total() / counts[index] as f64;
                if !value.is_finite() {
                    return Err(ScalarVariogramError::NumericalFailure);
                }
                Some(value)
            };
            Ok(ScalarVariogramRow {
                lower_um: bin.lower_um,
                upper_um: bin.upper_um,
                upper_inclusive: index + 1 == bins.len(),
                pair_count: counts[index],
                semivariance,
            })
        })
        .collect()
}

fn has_distinct_values(values: &[f64]) -> bool {
    values
        .first()
        .is_some_and(|first| values[1..].iter().any(|value| value != first))
}

fn has_stratified_distinct_values(values: &[f64], strata: &[u32]) -> bool {
    if values.len() != strata.len() {
        return false;
    }
    let mut first_by_stratum = BTreeMap::<u32, f64>::new();
    for (value, stratum) in values.iter().zip(strata) {
        match first_by_stratum.get(stratum) {
            Some(first) if first != value => return true,
            Some(_) => {}
            None => {
                first_by_stratum.insert(*stratum, *value);
            }
        }
    }
    false
}

fn validate_bins(bins: &[ScalarVariogramBin]) -> Result<(), ScalarVariogramError> {
    if bins.is_empty()
        || bins.iter().any(|bin| {
            !bin.lower_um.is_finite()
                || !bin.upper_um.is_finite()
                || bin.lower_um < 0.0
                || bin.upper_um <= bin.lower_um
        })
        || bins
            .windows(2)
            .any(|pair| pair[0].upper_um.to_bits() != pair[1].lower_um.to_bits())
    {
        return Err(ScalarVariogramError::InvalidBins);
    }
    Ok(())
}

fn validate_points(
    x: &[f64],
    y: &[f64],
    window: &ObservationWindow2D,
) -> Result<(), ScalarVariogramError> {
    if x.len() != y.len() {
        return Err(ScalarVariogramError::PointShapeMismatch {
            x_rows: x.len(),
            y_rows: y.len(),
        });
    }
    let mut coordinates = BTreeMap::new();
    for row in 0..x.len() {
        if !window.contains(x[row], y[row]) {
            return Err(ScalarVariogramError::PointOutsideWindow { row });
        }
        let key = (canonical_bits(x[row]), canonical_bits(y[row]));
        if let Some(first_row) = coordinates.insert(key, row) {
            return Err(ScalarVariogramError::DuplicatePoint {
                first_row,
                second_row: row,
            });
        }
    }
    Ok(())
}

fn canonical_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn find_bin(distance: f64, bins: &[ScalarVariogramBin]) -> Option<usize> {
    bins.iter().enumerate().position(|(index, bin)| {
        distance >= bin.lower_um
            && (distance < bin.upper_um || (index + 1 == bins.len() && distance <= bin.upper_um))
    })
}

pub(crate) fn pair_plan_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    declared_input_digest: ContentDigest,
    bins: &[ScalarVariogramBin],
) -> ContentDigest {
    let pattern = input.pattern();
    let mut fields = vec![
        b"marklab-scalar-variogram-pair-plan-v1".to_vec(),
        input.coordinate_frame_id().as_str().as_bytes().to_vec(),
        window.descriptor().logical_digest.as_bytes().to_vec(),
        mark_id.as_str().as_bytes().to_vec(),
        declared_input_digest.as_bytes().to_vec(),
    ];
    for ((cell_id, x), y) in input
        .cell_ids()
        .iter()
        .zip(&pattern.x_um)
        .zip(&pattern.y_um)
    {
        fields.push(cell_id.as_str().as_bytes().to_vec());
        fields.push(x.to_bits().to_be_bytes().to_vec());
        fields.push(y.to_bits().to_be_bytes().to_vec());
    }
    for bin in bins {
        fields.push(bin.lower_um.to_bits().to_be_bytes().to_vec());
        fields.push(bin.upper_um.to_bits().to_be_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

#[derive(Clone, Copy, Default)]
struct CompensatedSum {
    sum: f64,
    correction: f64,
}

impl CompensatedSum {
    fn add(&mut self, value: f64) {
        let adjusted = value - self.correction;
        let next = self.sum + adjusted;
        self.correction = (next - self.sum) - adjusted;
        self.sum = next;
    }

    fn total(self) -> f64 {
        self.sum
    }
}

/// Invalid scalar-semivariogram input, plan, resource, or numerical state.
#[derive(Debug, Error, PartialEq)]
pub enum ScalarVariogramError {
    /// Typed input identity could not be recomputed.
    #[error("scalar variogram input identity is invalid: {0}")]
    InvalidInput(String),
    /// Lag bins are empty, invalid, overlapping, or noncontiguous.
    #[error("scalar variogram bins must be finite, increasing, nonnegative, and contiguous")]
    InvalidBins,
    /// A resource ceiling is zero.
    #[error("scalar variogram resource limits must be positive")]
    InvalidResourceLimit,
    /// Permutation count or family-wise alpha is invalid or unresolvable.
    #[error("scalar variogram inference requires positive permutations, alpha in (0,1), and (B+1)*alpha >= 1")]
    InvalidInferenceDesign,
    /// The observation window has no installed frame binding.
    #[error("scalar variogram requires a coordinate-frame-bound observation window")]
    UnboundObservationWindow,
    /// Point and window frames differ.
    #[error("scalar variogram frame mismatch: expected {expected}, observed {observed}")]
    CoordinateFrameMismatch {
        /// Point/mark input frame.
        expected: CoordinateFrameId,
        /// Window frame.
        observed: CoordinateFrameId,
    },
    /// Fewer than two point rows were supplied.
    #[error("scalar variogram requires at least two points")]
    InsufficientPoints,
    /// Point rows exceed the explicit limit.
    #[error("scalar variogram has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Explicit maximum.
        maximum: usize,
    },
    /// Required unordered visits exceed the explicit limit.
    #[error("scalar variogram pair visits are {observed}; maximum is {maximum}")]
    PairVisitLimitExceeded {
        /// Required visits.
        observed: usize,
        /// Explicit maximum.
        maximum: usize,
    },
    /// Permutation-by-pair work exceeds the explicit ceiling.
    #[error("scalar variogram permutation work is {observed}; maximum is {maximum}")]
    PermutationWorkExceeded {
        /// Required pair evaluations.
        observed: usize,
        /// Explicit maximum.
        maximum: usize,
    },
    /// Coordinate columns have different row counts.
    #[error("scalar variogram coordinate row mismatch: x has {x_rows}, y has {y_rows}")]
    PointShapeMismatch {
        /// X-coordinate rows.
        x_rows: usize,
        /// Y-coordinate rows.
        y_rows: usize,
    },
    /// A point lies outside the exact window.
    #[error("scalar variogram point row {row} lies outside the observation window")]
    PointOutsideWindow {
        /// Outside row.
        row: usize,
    },
    /// Two point rows have the same exact coordinates.
    #[error("scalar variogram duplicate point rows {first_row} and {second_row}")]
    DuplicatePoint {
        /// First duplicate row.
        first_row: usize,
        /// Second duplicate row.
        second_row: usize,
    },
    /// Input was not constructed from a typed MarkTable.
    #[error("scalar variogram requires a typed MarkTable input")]
    TypedMarkTableRequired,
    /// Requested continuous mark is absent.
    #[error("scalar variogram continuous mark {mark_id:?} is absent")]
    ContinuousMarkMissing {
        /// Missing mark identity.
        mark_id: ScalarMarkId,
    },
    /// Typed mark values do not align with point rows.
    #[error("scalar variogram row count mismatch: expected {expected}, observed {observed}")]
    RowCountMismatch {
        /// Required point rows.
        expected: usize,
        /// Observed mark rows.
        observed: usize,
    },
    /// Requested typed histologic-compartment conditioning is absent.
    #[error("scalar variogram histologic_compartment stratum is missing")]
    MissingCompartmentStratum,
    /// No declared block contains exchangeable distinct scalar values.
    #[error("scalar variogram random-labeling null is degenerate")]
    DegenerateNull,
    /// Every declared lag bin is empty.
    #[error("scalar variogram has no nonempty bin eligible for curve inference")]
    NoEligibleBins,
    /// Shared blocked-permutation or ERL evaluation failed.
    #[error("scalar variogram inference failed: {0}")]
    Inference(String),
    /// Checked count arithmetic overflowed.
    #[error("scalar variogram size arithmetic overflow")]
    SizeOverflow,
    /// Finite inputs produced a non-finite contribution.
    #[error("scalar variogram produced a non-finite numerical result")]
    NumericalFailure,
}
