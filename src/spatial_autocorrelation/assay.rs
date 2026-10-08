use std::collections::BTreeSet;

use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;

use crate::{AssayMarkValues, MarkTable, ObservationWindow2D, ScalarMarkId};

use super::{
    admission::reject_duplicate_points, weights::RadiusWeights, GlobalMoranError,
    GlobalMoranWeightPolicy,
};

/// Row-aligned physical coordinates and the shared assay table they locate.
pub struct AssaySpatialInput<'a> {
    /// Canonically ordered cell identities and assay observations.
    pub table: &'a MarkTable,
    /// X coordinates in micrometres, in exactly the table's row order.
    pub x_um: &'a [f64],
    /// Y coordinates in micrometres, in exactly the table's row order.
    pub y_um: &'a [f64],
    /// Installed physical frame shared with the window.
    pub coordinate_frame_id: &'a CoordinateFrameId,
}

/// Work ceilings for the descriptive, multi-channel spatial calculation.
#[derive(Clone, Copy, Debug)]
pub struct AssaySpatialLimits {
    /// Total input cell rows, including unavailable observations.
    pub maximum_points: usize,
    /// Directed edges retained in one observed-row graph.
    pub maximum_directed_edges: usize,
    /// Prespecified selected channels.
    pub maximum_channels: usize,
    /// Sum of directed-edge visits over both statistics and all channels.
    pub maximum_edge_evaluations: usize,
}

/// Why a required channel cannot supply a spatial endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssaySpatialUnavailable {
    /// Fewer than three observed values remain after complete-case selection.
    InsufficientObservedPoints,
    /// At least one observed point has no observed neighbour under the declared radius.
    IsolatedObservedPoint,
    /// The observed values have no variance.
    ZeroVariance,
    /// Finite input overflowed or otherwise failed numerical evaluation.
    NumericalFailure,
}

/// Descriptive statistics or a typed unavailable endpoint; no cell-level p-value is implied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AssaySpatialOutcome {
    /// Canonical Moran I and Geary C on the same observed rows and radius graph.
    Available { moran_i: f64, geary_c: f64 },
    /// A required endpoint remains visible when it cannot be estimated.
    Unavailable { reason: AssaySpatialUnavailable },
}

/// One endpoint's explicit complete-case denominator and geometry identity.
#[derive(Clone, Debug, PartialEq)]
pub struct AssaySpatialSummary {
    pub mark_id: ScalarMarkId,
    pub measurement_status: MeasurementStatus,
    pub observed_rows: usize,
    pub missing_rows: usize,
    pub directed_edges: usize,
    pub weights_digest: Option<ContentDigest>,
    pub outcome: AssaySpatialOutcome,
}

/// Compute both established radius statistics for prespecified quantitative/binary channels.
///
/// Missing values select an explicit per-channel observed-row subgraph; no zero imputation or
/// nominal-category arithmetic occurs. Consecutive channels with identical observed rows share
/// one graph, and both statistics always share it. Only one graph is retained. Expected work is
/// index construction plus output-sensitive radius queries and O(edges) per statistic; the
/// explicit limits bound point, channel, graph and evaluation counts. Coordinate/selection or
/// resource errors abort; scientific unavailability stays in the returned endpoint list.
#[allow(clippy::too_many_arguments)]
pub fn summarize_assay_spatial(
    input: AssaySpatialInput<'_>,
    window: &ObservationWindow2D,
    selection: &[ScalarMarkId],
    radius_um: f64,
    policy: GlobalMoranWeightPolicy,
    limits: AssaySpatialLimits,
) -> Result<Vec<AssaySpatialSummary>, GlobalMoranError> {
    validate_input(&input, window, selection, radius_um, limits)?;
    let mut results = Vec::with_capacity(selection.len());
    let mut previous: Option<(Vec<usize>, RadiusWeights)> = None;
    let mut evaluations = 0usize;
    for mark_id in selection {
        let (declaration, column) = input.table.assay_column(mark_id).ok_or_else(|| {
            GlobalMoranError::InvalidDesign(format!("assay channel {} is absent", mark_id.as_str()))
        })?;
        let (rows, values) = observed_values(column)?;
        let mut result = AssaySpatialSummary {
            mark_id: mark_id.clone(),
            measurement_status: declaration.measurement_status(),
            observed_rows: rows.len(),
            missing_rows: input.table.cell_ids().len() - rows.len(),
            directed_edges: 0,
            weights_digest: None,
            outcome: AssaySpatialOutcome::Unavailable {
                reason: AssaySpatialUnavailable::InsufficientObservedPoints,
            },
        };
        if rows.len() < 3 {
            results.push(result);
            continue;
        }
        if previous
            .as_ref()
            .is_none_or(|(prior_rows, _)| *prior_rows != rows)
        {
            // Release the previous graph before constructing a different missingness support.
            previous = None;
            let x = rows.iter().map(|&row| input.x_um[row]).collect::<Vec<_>>();
            let y = rows.iter().map(|&row| input.y_um[row]).collect::<Vec<_>>();
            match RadiusWeights::build(&x, &y, radius_um, policy, limits.maximum_directed_edges) {
                Ok(weights) => previous = Some((rows, weights)),
                Err(GlobalMoranError::IsolatedPoint { .. }) => {
                    result.outcome = AssaySpatialOutcome::Unavailable {
                        reason: AssaySpatialUnavailable::IsolatedObservedPoint,
                    };
                    results.push(result);
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        let weights = &previous.as_ref().expect("successful graph admission").1;
        evaluations = weights
            .edges
            .len()
            .checked_mul(2)
            .and_then(|work| evaluations.checked_add(work))
            .ok_or(GlobalMoranError::SizeOverflow)?;
        if evaluations > limits.maximum_edge_evaluations {
            return Err(GlobalMoranError::InvalidDesign(format!(
                "assay edge evaluations {evaluations} exceed {}",
                limits.maximum_edge_evaluations
            )));
        }
        result.directed_edges = weights.edges.len();
        result.weights_digest = Some(weights.digest);
        result.outcome = match weights.evaluate(&values).and_then(|moran_i| {
            weights
                .evaluate_geary(&values)
                .map(|geary_c| (moran_i, geary_c))
        }) {
            Ok((moran_i, geary_c)) => AssaySpatialOutcome::Available { moran_i, geary_c },
            Err(GlobalMoranError::ZeroVariance) => AssaySpatialOutcome::Unavailable {
                reason: AssaySpatialUnavailable::ZeroVariance,
            },
            Err(GlobalMoranError::NumericalFailure) => AssaySpatialOutcome::Unavailable {
                reason: AssaySpatialUnavailable::NumericalFailure,
            },
            Err(error) => return Err(error),
        };
        results.push(result);
    }
    Ok(results)
}

fn observed_values(column: &AssayMarkValues) -> Result<(Vec<usize>, Vec<f64>), GlobalMoranError> {
    let observed: Vec<(usize, f64)> = match column {
        AssayMarkValues::Continuous(values) => values
            .iter()
            .enumerate()
            .filter_map(|(row, value)| value.map(|value| (row, value)))
            .collect(),
        AssayMarkValues::Binary(values) => values
            .iter()
            .enumerate()
            .filter_map(|(row, value)| value.map(|value| (row, f64::from(u8::from(value)))))
            .collect(),
        AssayMarkValues::Categorical { .. } => {
            return Err(GlobalMoranError::InvalidDesign(
                "nominal categories cannot be selected as numeric Moran/Geary channels".into(),
            ))
        }
    };
    Ok(observed.into_iter().unzip())
}

fn validate_input(
    input: &AssaySpatialInput<'_>,
    window: &ObservationWindow2D,
    selection: &[ScalarMarkId],
    radius_um: f64,
    limits: AssaySpatialLimits,
) -> Result<(), GlobalMoranError> {
    if [
        limits.maximum_points,
        limits.maximum_directed_edges,
        limits.maximum_channels,
        limits.maximum_edge_evaluations,
    ]
    .contains(&0)
    {
        return Err(GlobalMoranError::InvalidResourceLimit);
    }
    if !radius_um.is_finite() || radius_um <= 0.0 {
        return Err(GlobalMoranError::InvalidRadius);
    }
    if selection.is_empty()
        || selection.len() > limits.maximum_channels
        || selection.iter().collect::<BTreeSet<_>>().len() != selection.len()
    {
        return Err(GlobalMoranError::InvalidDesign(
            "assay selection must be nonempty, unique and within the channel limit".into(),
        ));
    }
    let frame = window
        .coordinate_frame_id()
        .ok_or(GlobalMoranError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id {
        return Err(GlobalMoranError::CoordinateFrameMismatch {
            expected: input.coordinate_frame_id.clone(),
            observed: frame.clone(),
        });
    }
    let rows = input.table.cell_ids().len();
    if rows > limits.maximum_points {
        return Err(GlobalMoranError::PointLimitExceeded {
            observed: rows,
            maximum: limits.maximum_points,
        });
    }
    for values in [input.x_um, input.y_um] {
        if values.len() != rows {
            return Err(GlobalMoranError::RowCountMismatch {
                expected: rows,
                observed: values.len(),
            });
        }
    }
    for (row, (&x, &y)) in input.x_um.iter().zip(input.y_um).enumerate() {
        if !x.is_finite() || !y.is_finite() || !window.contains(x, y) {
            return Err(GlobalMoranError::PointOutsideWindow { row });
        }
    }
    reject_duplicate_points(input.x_um, input.y_um)
}
