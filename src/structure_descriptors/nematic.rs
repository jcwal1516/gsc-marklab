//! Weighted two-dimensional nematic order and topological-defect screening.
//!
//! Every input orientation is axial: `theta` and `theta + pi` represent the same direction.
//! MarkLab therefore averages the doubled-angle vectors and reports
//! `Q = <2 n n^T - I>_w`, whose independent entries are
//! `Q_xx = <cos(2 theta)>_w`, `Q_xy = <sin(2 theta)>_w`, and `Q_yy = -Q_xx`.
//! The scalar order is `S = hypot(Q_xx, Q_xy)` and the director is
//! `0.5 atan2(Q_xy, Q_xx)` modulo `pi`. A caller-declared order floor suppresses unstable
//! directors without discarding the measured tensor or scalar order.
//!
//! Local and regular-grid estimates use complete circular support: centers closer to the
//! observation-window exterior or a hole than their support radius are reported as boundary
//! truncated. Defects are screened on regular-grid plaquettes by winding the doubled director
//! phase counterclockwise. A winding of `+1` or `-1` is a nematic charge of `+1/2` or `-1/2`.
//! Plaquettes are classified only when the complete plaquette plus field-support radius lies
//! inside the window, all four corners have enough support and order, every wrapped phase step
//! is below the declared ambiguity limit, and the winding is close to an integer.

use marklab_numerics::summation::NeumaierSum;
use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};
use crate::geom::spatial_index::SpatialIndex2D;
use crate::{ObservationWindow2D, ObservationWindowLimits};

/// Result format of [`nematic_orientation_from_json`].
pub const NEMATIC_ORIENTATION_FORMAT: &str = "marklab.nematic_orientation";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_GRID_POINTS: usize = 250_000;
const MAXIMUM_PLAQUETTES: usize = 250_000;
const MAXIMUM_NEIGHBOR_VISITS: usize = 250_000_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    orientation_source: String,
    weight_name: String,
    window: serde_json::Value,
    cells: Vec<OrientedCell>,
    minimum_resolved_order: f64,
    local: LocalNematicConfig,
    field: NematicFieldConfig,
    defects: DefectDetectionConfig,
    limits: NematicLimits,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrientedCell {
    id: String,
    x_um: f64,
    y_um: f64,
    orientation_rad: f64,
    weight: f64,
}

/// Complete-disk neighborhood used for cell-centered local order.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalNematicConfig {
    pub radius_um: f64,
    pub minimum_cells: usize,
}

/// Regular-grid sampling of the weighted orientation field.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NematicFieldConfig {
    pub origin_um: [f64; 2],
    pub spacing_um: f64,
    pub dimensions: [usize; 2],
    pub support_radius_um: f64,
    pub minimum_cells: usize,
}

/// Robustness thresholds for doubled-phase plaquette winding.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DefectDetectionConfig {
    pub minimum_corner_order: f64,
    pub maximum_edge_phase_step_rad: f64,
    pub winding_tolerance: f64,
}

/// Explicit point, query-work, grid and plaquette ceilings.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NematicLimits {
    pub maximum_cells: usize,
    pub maximum_grid_points: usize,
    pub maximum_neighbor_visits: usize,
    pub maximum_plaquettes: usize,
}

/// Availability of one weighted nematic estimate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NematicEstimateStatus {
    Available,
    LowOrder,
    OutsideWindow,
    BoundaryTruncated,
    InsufficientSupport,
}

/// Independent entries of `Q = <2 n n^T - I>_w`, scalar order and axial director.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NematicTensor {
    pub q_xx: Option<f64>,
    pub q_xy: Option<f64>,
    pub q_yy: Option<f64>,
    pub scalar_order: Option<f64>,
    /// Canonical axial direction in `[0, pi)`, absent below the declared order floor.
    pub director_rad: Option<f64>,
}

/// One weighted estimate with explicit support and availability.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NematicEstimate {
    pub status: NematicEstimateStatus,
    pub support_cells: usize,
    pub support_weight: f64,
    pub tensor: NematicTensor,
}

/// Cell-centered local nematic estimate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalNematicOrder {
    pub id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub estimate: NematicEstimate,
}

/// One regular-grid field sample.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NematicFieldSample {
    pub grid_index: [usize; 2],
    pub x_um: f64,
    pub y_um: f64,
    pub estimate: NematicEstimate,
}

/// Resolution state of one regular-grid plaquette.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefectStatus {
    PositiveHalf,
    NegativeHalf,
    NoDefect,
    WindowTruncated,
    IncompleteSupport,
    LowOrder,
    BranchAmbiguous,
    NonQuantizedWinding,
    UnsupportedWinding,
}

/// Counterclockwise doubled-phase winding on one grid plaquette.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NematicDefectPlaquette {
    pub lower_left_grid_index: [usize; 2],
    pub center_um: [f64; 2],
    pub status: DefectStatus,
    /// Winding of the doubled phase before integer resolution.
    pub doubled_phase_winding: Option<f64>,
    /// Resolved nematic charge: `-0.5`, `0`, or `0.5`.
    pub topological_charge: Option<f64>,
}

/// Weighted global/local nematic order, sampled director field and robust half-defect screen.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NematicOrientationResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub orientation_source: String,
    pub weight_name: String,
    pub cell_count: usize,
    pub minimum_resolved_order: f64,
    pub local_config: LocalNematicConfig,
    pub field_config: NematicFieldConfig,
    pub defect_config: DefectDetectionConfig,
    pub limits: NematicLimits,
    pub global: NematicEstimate,
    pub local: Vec<LocalNematicOrder>,
    pub field: Vec<NematicFieldSample>,
    pub plaquettes: Vec<NematicDefectPlaquette>,
    pub positive_half_defects: usize,
    pub negative_half_defects: usize,
    pub unresolved_plaquettes: usize,
    pub neighbor_visits: usize,
    pub orientation_convention: String,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a strict `marklab.nematic_orientation_request` version 1 and analyze it.
pub fn nematic_orientation_from_json(input: &[u8]) -> Result<NematicOrientationResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("nematic-orientation input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    validate_request(&request)?;
    let window = ObservationWindow2D::from_geojson_str(
        &serde_json::to_string(&request.window)?,
        ObservationWindowLimits::default(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    validate_cells(&request.cells, &window)?;

    let xs = request
        .cells
        .iter()
        .map(|cell| cell.x_um)
        .collect::<Vec<_>>();
    let ys = request
        .cells
        .iter()
        .map(|cell| cell.y_um)
        .collect::<Vec<_>>();
    let index = SpatialIndex2D::new(&xs, &ys)?;
    let all = (0..request.cells.len()).collect::<Vec<_>>();
    let global = estimate(
        &request.cells,
        &all,
        request.minimum_resolved_order,
        request.cells.len(),
    );

    let mut neighbor_visits = 0_usize;
    let mut local = Vec::with_capacity(request.cells.len());
    for (row, cell) in request.cells.iter().enumerate() {
        let estimate = if window
            .boundary_distance_um(cell.x_um, cell.y_um)
            .map_err(|error| invalid(error.to_string()))?
            < request.local.radius_um
        {
            unavailable(NematicEstimateStatus::BoundaryTruncated)
        } else {
            let mut support = index
                .within_radius(row, request.local.radius_um)?
                .into_iter()
                .map(|neighbor| neighbor.index)
                .collect::<Vec<_>>();
            support.push(row);
            support.sort_unstable();
            add_visits(&mut neighbor_visits, support.len(), request.limits)?;
            estimate(
                &request.cells,
                &support,
                request.minimum_resolved_order,
                request.local.minimum_cells,
            )
        };
        local.push(LocalNematicOrder {
            id: cell.id.clone(),
            x_um: cell.x_um,
            y_um: cell.y_um,
            estimate,
        });
    }

    let [width, height] = request.field.dimensions;
    let mut field = Vec::with_capacity(width * height);
    for y_index in 0..height {
        for x_index in 0..width {
            let x_um = grid_coordinate(
                request.field.origin_um[0],
                request.field.spacing_um,
                x_index,
            )?;
            let y_um = grid_coordinate(
                request.field.origin_um[1],
                request.field.spacing_um,
                y_index,
            )?;
            let estimate = if !window.contains(x_um, y_um) {
                unavailable(NematicEstimateStatus::OutsideWindow)
            } else if window
                .boundary_distance_um(x_um, y_um)
                .map_err(|error| invalid(error.to_string()))?
                < request.field.support_radius_um
            {
                unavailable(NematicEstimateStatus::BoundaryTruncated)
            } else {
                let mut support = index
                    .points_within_radius(x_um, y_um, request.field.support_radius_um)?
                    .into_iter()
                    .map(|neighbor| neighbor.index)
                    .collect::<Vec<_>>();
                support.sort_unstable();
                add_visits(&mut neighbor_visits, support.len(), request.limits)?;
                estimate(
                    &request.cells,
                    &support,
                    request.minimum_resolved_order,
                    request.field.minimum_cells,
                )
            };
            field.push(NematicFieldSample {
                grid_index: [x_index, y_index],
                x_um,
                y_um,
                estimate,
            });
        }
    }

    let plaquettes = detect_defects(&request, &window, &field)?;
    let positive_half_defects = plaquettes
        .iter()
        .filter(|row| row.status == DefectStatus::PositiveHalf)
        .count();
    let negative_half_defects = plaquettes
        .iter()
        .filter(|row| row.status == DefectStatus::NegativeHalf)
        .count();
    let unresolved_plaquettes = plaquettes
        .iter()
        .filter(|row| {
            !matches!(
                row.status,
                DefectStatus::PositiveHalf | DefectStatus::NegativeHalf | DefectStatus::NoDefect
            )
        })
        .count();

    Ok(NematicOrientationResult {
        format: NEMATIC_ORIENTATION_FORMAT.into(),
        version: 1,
        coordinate_frame_id: request.coordinate_frame_id,
        orientation_source: request.orientation_source,
        weight_name: request.weight_name,
        cell_count: request.cells.len(),
        minimum_resolved_order: request.minimum_resolved_order,
        local_config: request.local,
        field_config: request.field,
        defect_config: request.defects,
        limits: request.limits,
        global,
        local,
        field,
        plaquettes,
        positive_half_defects,
        negative_half_defects,
        unresolved_plaquettes,
        neighbor_visits,
        orientation_convention: "axial_radians_modulo_pi; Q=<2nnT-I>_weight; S=sqrt(Q_xx^2+Q_xy^2)"
            .into(),
        claim_scope: "descriptive_static_nematic_shape_analysis_not_force_or_causality".into(),
    })
}

fn validate_request(request: &Request) -> Result<()> {
    let limits = request.limits;
    let field = request.field;
    let local = request.local;
    let defects = request.defects;
    if request.format != "marklab.nematic_orientation_request"
        || request.version != 1
        || request.coordinate_frame_id.trim().is_empty()
        || request.orientation_source.trim().is_empty()
        || request.orientation_source.trim() != request.orientation_source
        || request.weight_name.trim().is_empty()
        || request.weight_name.trim() != request.weight_name
        || limits.maximum_cells == 0
        || limits.maximum_grid_points == 0
        || limits.maximum_neighbor_visits == 0
        || limits.maximum_plaquettes == 0
        || request.cells.is_empty()
        || request.cells.len() > limits.maximum_cells
        || !request.minimum_resolved_order.is_finite()
        || !(0.0..=1.0).contains(&request.minimum_resolved_order)
        || request.minimum_resolved_order == 0.0
        || !local.radius_um.is_finite()
        || local.radius_um <= 0.0
        || local.minimum_cells == 0
        || !field.origin_um.iter().all(|value| value.is_finite())
        || !field.spacing_um.is_finite()
        || field.spacing_um <= 0.0
        || !field.support_radius_um.is_finite()
        || field.support_radius_um <= 0.0
        || field.minimum_cells == 0
        || field.dimensions.iter().any(|dimension| *dimension < 2)
        || !defects.minimum_corner_order.is_finite()
        || defects.minimum_corner_order < request.minimum_resolved_order
        || defects.minimum_corner_order > 1.0
        || !defects.maximum_edge_phase_step_rad.is_finite()
        || defects.maximum_edge_phase_step_rad <= 0.0
        || defects.maximum_edge_phase_step_rad >= std::f64::consts::PI
        || !defects.winding_tolerance.is_finite()
        || defects.winding_tolerance <= 0.0
        || defects.winding_tolerance >= 0.5
    {
        return Err(invalid(
            "nematic-orientation format, configuration, thresholds or limits are invalid",
        ));
    }
    let grid_points = field.dimensions[0]
        .checked_mul(field.dimensions[1])
        .ok_or_else(|| invalid("nematic field grid size overflowed"))?;
    let plaquettes = (field.dimensions[0] - 1)
        .checked_mul(field.dimensions[1] - 1)
        .ok_or_else(|| invalid("nematic plaquette count overflowed"))?;
    if grid_points > limits.maximum_grid_points
        || grid_points > MAXIMUM_GRID_POINTS
        || plaquettes > limits.maximum_plaquettes
        || plaquettes > MAXIMUM_PLAQUETTES
    {
        return Err(invalid(
            "nematic field grid or plaquette count exceeds a declared or built-in limit",
        ));
    }
    for axis in 0..2 {
        grid_coordinate(
            field.origin_um[axis],
            field.spacing_um,
            field.dimensions[axis] - 1,
        )?;
    }
    Ok(())
}

fn validate_cells(cells: &[OrientedCell], window: &ObservationWindow2D) -> Result<()> {
    let mut ids = std::collections::HashSet::new();
    if cells.iter().any(|cell| {
        cell.id.trim().is_empty()
            || !ids.insert(cell.id.as_str())
            || !cell.x_um.is_finite()
            || !cell.y_um.is_finite()
            || !cell.orientation_rad.is_finite()
            || !cell.weight.is_finite()
            || cell.weight <= 0.0
            || !window.contains(cell.x_um, cell.y_um)
    }) {
        return Err(invalid(
            "nematic cells require unique identifiers, finite in-window coordinates, finite axial orientations and positive weights",
        ));
    }
    let mut total_weight = NeumaierSum::new();
    for cell in cells {
        total_weight.add(cell.weight);
    }
    if !total_weight.total().is_finite() {
        return Err(invalid("nematic cell weights have a non-finite total"));
    }
    Ok(())
}

fn grid_coordinate(origin: f64, spacing: f64, index: usize) -> Result<f64> {
    let coordinate = origin + spacing * index as f64;
    if coordinate.is_finite() {
        Ok(coordinate)
    } else {
        Err(invalid("nematic field coordinate is not finite"))
    }
}

fn add_visits(total: &mut usize, increment: usize, limits: NematicLimits) -> Result<()> {
    *total = total
        .checked_add(increment)
        .ok_or_else(|| invalid("nematic neighbor-visit count overflowed"))?;
    if *total > limits.maximum_neighbor_visits || *total > MAXIMUM_NEIGHBOR_VISITS {
        return Err(invalid(format!(
            "nematic neighbor visits {} exceed a declared or built-in maximum",
            *total
        )));
    }
    Ok(())
}

fn unavailable(status: NematicEstimateStatus) -> NematicEstimate {
    NematicEstimate {
        status,
        support_cells: 0,
        support_weight: 0.0,
        tensor: NematicTensor {
            q_xx: None,
            q_xy: None,
            q_yy: None,
            scalar_order: None,
            director_rad: None,
        },
    }
}

fn estimate(
    cells: &[OrientedCell],
    support: &[usize],
    minimum_resolved_order: f64,
    minimum_cells: usize,
) -> NematicEstimate {
    if support.len() < minimum_cells {
        return NematicEstimate {
            status: NematicEstimateStatus::InsufficientSupport,
            support_cells: support.len(),
            support_weight: support.iter().map(|index| cells[*index].weight).sum(),
            tensor: NematicTensor {
                q_xx: None,
                q_xy: None,
                q_yy: None,
                scalar_order: None,
                director_rad: None,
            },
        };
    }
    let mut weight = NeumaierSum::new();
    let mut xx = NeumaierSum::new();
    let mut xy = NeumaierSum::new();
    let weight_scale = support
        .iter()
        .map(|index| cells[*index].weight)
        .fold(0.0_f64, f64::max);
    for &index in support {
        let cell = &cells[index];
        let scaled_weight = cell.weight / weight_scale;
        let axial_angle = cell.orientation_rad.rem_euclid(std::f64::consts::PI);
        weight.add(scaled_weight);
        xx.add(scaled_weight * (2.0 * axial_angle).cos());
        xy.add(scaled_weight * (2.0 * axial_angle).sin());
    }
    let normalized_weight = weight.total();
    let support_weight = normalized_weight * weight_scale;
    let q_xx = xx.total() / normalized_weight;
    let q_xy = xy.total() / normalized_weight;
    let scalar_order = q_xx.hypot(q_xy).min(1.0);
    let resolved = scalar_order >= minimum_resolved_order;
    let director_rad = resolved.then(|| (0.5 * q_xy.atan2(q_xx)).rem_euclid(std::f64::consts::PI));
    NematicEstimate {
        status: if resolved {
            NematicEstimateStatus::Available
        } else {
            NematicEstimateStatus::LowOrder
        },
        support_cells: support.len(),
        support_weight,
        tensor: NematicTensor {
            q_xx: Some(q_xx),
            q_xy: Some(q_xy),
            q_yy: Some(-q_xx),
            scalar_order: Some(scalar_order),
            director_rad,
        },
    }
}

fn detect_defects(
    request: &Request,
    window: &ObservationWindow2D,
    field: &[NematicFieldSample],
) -> Result<Vec<NematicDefectPlaquette>> {
    let [width, height] = request.field.dimensions;
    let mut output = Vec::with_capacity((width - 1) * (height - 1));
    let complete_radius = request.field.support_radius_um
        + request.field.spacing_um * std::f64::consts::FRAC_1_SQRT_2;
    for y_index in 0..height - 1 {
        for x_index in 0..width - 1 {
            let center = [
                grid_coordinate(
                    request.field.origin_um[0],
                    request.field.spacing_um,
                    x_index,
                )? + 0.5 * request.field.spacing_um,
                grid_coordinate(
                    request.field.origin_um[1],
                    request.field.spacing_um,
                    y_index,
                )? + 0.5 * request.field.spacing_um,
            ];
            let base = y_index * width + x_index;
            let corners = [base, base + 1, base + width + 1, base + width];
            let status = if !window.contains(center[0], center[1])
                || window
                    .boundary_distance_um(center[0], center[1])
                    .map_err(|error| invalid(error.to_string()))?
                    < complete_radius
            {
                (DefectStatus::WindowTruncated, None, None)
            } else if corners.iter().any(|corner| {
                !matches!(
                    field[*corner].estimate.status,
                    NematicEstimateStatus::Available | NematicEstimateStatus::LowOrder
                )
            }) {
                (DefectStatus::IncompleteSupport, None, None)
            } else if corners.iter().any(|corner| {
                field[*corner]
                    .estimate
                    .tensor
                    .scalar_order
                    .is_none_or(|order| order < request.defects.minimum_corner_order)
            }) {
                (DefectStatus::LowOrder, None, None)
            } else {
                classify_winding(request, field, corners)
            };
            output.push(NematicDefectPlaquette {
                lower_left_grid_index: [x_index, y_index],
                center_um: center,
                status: status.0,
                doubled_phase_winding: status.1,
                topological_charge: status.2,
            });
        }
    }
    Ok(output)
}

fn classify_winding(
    request: &Request,
    field: &[NematicFieldSample],
    corners: [usize; 4],
) -> (DefectStatus, Option<f64>, Option<f64>) {
    let phases = corners.map(|corner| {
        let tensor = &field[corner].estimate.tensor;
        tensor.q_xy.unwrap_or(0.0).atan2(tensor.q_xx.unwrap_or(0.0))
    });
    let mut sum = NeumaierSum::new();
    for edge in 0..4 {
        let delta = wrapped_phase_delta(phases[(edge + 1) % 4] - phases[edge]);
        if delta.abs() > request.defects.maximum_edge_phase_step_rad {
            return (DefectStatus::BranchAmbiguous, None, None);
        }
        sum.add(delta);
    }
    let winding = sum.total() / std::f64::consts::TAU;
    let quantized = winding.round();
    if (winding - quantized).abs() > request.defects.winding_tolerance {
        return (DefectStatus::NonQuantizedWinding, Some(winding), None);
    }
    match quantized as i32 {
        0 => (DefectStatus::NoDefect, Some(winding), Some(0.0)),
        1 => (DefectStatus::PositiveHalf, Some(winding), Some(0.5)),
        -1 => (DefectStatus::NegativeHalf, Some(winding), Some(-0.5)),
        _ => (DefectStatus::UnsupportedWinding, Some(winding), None),
    }
}

fn wrapped_phase_delta(delta: f64) -> f64 {
    (delta + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}
