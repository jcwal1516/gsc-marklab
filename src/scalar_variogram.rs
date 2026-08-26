use std::collections::BTreeMap;

use marklab_data::{CoordinateFrameId, MeasurementStatus};
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

    let mut counts = vec![0_usize; bins.len()];
    let mut sums = vec![CompensatedSum::default(); bins.len()];
    for left in 0..point_count {
        for right in (left + 1)..point_count {
            let distance = (pattern.x_um[left] - pattern.x_um[right])
                .hypot(pattern.y_um[left] - pattern.y_um[right]);
            if let Some(bin) = find_bin(distance, bins) {
                let difference = f64::from(values[left]) - f64::from(values[right]);
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
    let curve = bins
        .iter()
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
        .collect::<Result<Vec<_>, _>>()?;

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

fn pair_plan_digest(
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
    /// Checked count arithmetic overflowed.
    #[error("scalar variogram size arithmetic overflow")]
    SizeOverflow,
    /// Finite inputs produced a non-finite contribution.
    #[error("scalar variogram produced a non-finite numerical result")]
    NumericalFailure,
}
