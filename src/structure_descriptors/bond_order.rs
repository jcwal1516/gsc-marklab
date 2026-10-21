//! Bond-orientational (k-atic) order of cell positions.
//!
//! For a cell `i` with neighbors `j`, `ψ_k(i) = (1/n_i) Σ_j e^{i k φ_ij}`, where `φ_ij` is the
//! angle of the vector from `i` to `j` measured from the x axis. `|ψ_k(i)|` is 1 when the bonds
//! repeat every `2π/k` (for `k = 6`, a hexagonal arrangement) and near 0 when they are
//! disordered; the phase gives the local lattice orientation. Neighbors are either the
//! declared number of nearest cells (ties at the cutoff broken by input order and flagged) or
//! all cells within a declared radius.
//!
//! Border rule: an unobserved cell beyond the window boundary could change a cell's neighbor
//! set, so a cell is included only when its boundary distance is at least its farthest
//! neighbor distance (nearest-count rule) or the radius (radius rule). Excluded cells are
//! reported, not averaged.

use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};
use crate::geom::spatial_index::SpatialIndex2D;
use crate::{ObservationWindow2D, ObservationWindowLimits};

/// Result format of [`bond_orientational_order_from_json`].
pub const BOND_ORIENTATIONAL_ORDER_FORMAT: &str = "marklab.bond_orientational_order";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    window: serde_json::Value,
    cells: Vec<Cell>,
    symmetry: u32,
    neighbors: NeighborRule,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    id: String,
    x_um: f64,
    y_um: f64,
}

/// Declared neighbor rule.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum NeighborRule {
    NearestCount { count: usize },
    Radius { radius_um: f64 },
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_cells: usize,
}

/// One cell's order parameter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellOrder {
    pub id: String,
    pub neighbors: usize,
    pub included: bool,
    /// Whether the nearest-count neighbor set was decided by a distance tie at its cutoff.
    pub tie_at_cutoff: bool,
    pub psi_real: Option<f64>,
    pub psi_imaginary: Option<f64>,
    pub magnitude: Option<f64>,
}

/// Bond-orientational order of one pattern.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BondOrientationalOrderResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub symmetry: u32,
    pub neighbors: NeighborRule,
    pub cells: Vec<CellOrder>,
    pub included_cells: usize,
    pub excluded_cells: usize,
    /// Mean of `|ψ_k(i)|` over included cells: local order.
    pub mean_magnitude: Option<f64>,
    /// `|mean ψ_k(i)|` over included cells: shared orientation across the pattern.
    pub magnitude_of_mean: Option<f64>,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a `marklab.bond_orientational_order_request` version 1 and compute `ψ_k`.
pub fn bond_orientational_order_from_json(input: &[u8]) -> Result<BondOrientationalOrderResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("bond-orientational order input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let valid_rule = match request.neighbors {
        NeighborRule::NearestCount { count } => (1..=64).contains(&count),
        NeighborRule::Radius { radius_um } => radius_um.is_finite() && radius_um > 0.0,
    };
    if request.format != "marklab.bond_orientational_order_request"
        || request.version != 1
        || request.coordinate_frame_id.trim().is_empty()
        || !(1..=24).contains(&request.symmetry)
        || !valid_rule
        || !(2..=request.limits.maximum_cells).contains(&request.cells.len())
    {
        return Err(invalid(
            "bond-orientational order format, symmetry, neighbor rule or limits are invalid",
        ));
    }
    let window = ObservationWindow2D::from_geojson_str(
        &serde_json::to_string(&request.window)?,
        ObservationWindowLimits::default(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let mut ids = std::collections::HashSet::new();
    if !request.cells.iter().all(|cell| {
        !cell.id.trim().is_empty()
            && ids.insert(cell.id.as_str())
            && window.contains(cell.x_um, cell.y_um)
    }) {
        return Err(invalid(
            "cells need unique identifiers and finite coordinates inside the window",
        ));
    }
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
    let k = f64::from(request.symmetry);
    let mut cells = Vec::with_capacity(xs.len());
    for (row, cell) in request.cells.iter().enumerate() {
        let (neighbors, reach, tie) = match request.neighbors {
            NeighborRule::NearestCount { count } => {
                let mut found = index.k_nearest(row, count + 1)?;
                let tie =
                    found.len() > count && found[count].distance_um == found[count - 1].distance_um;
                found.truncate(count);
                let reach = found.last().map_or(f64::INFINITY, |n| n.distance_um);
                (found, reach, tie)
            }
            NeighborRule::Radius { radius_um } => {
                (index.within_radius(row, radius_um)?, radius_um, false)
            }
        };
        let boundary = window
            .boundary_distance_um(cell.x_um, cell.y_um)
            .map_err(|error| invalid(error.to_string()))?;
        let included = !neighbors.is_empty() && boundary >= reach;
        let psi = included.then(|| {
            let (re, im) = neighbors.iter().fold((0.0, 0.0), |(re, im), neighbor| {
                let angle = (ys[neighbor.index] - cell.y_um).atan2(xs[neighbor.index] - cell.x_um);
                (re + (k * angle).cos(), im + (k * angle).sin())
            });
            let count = neighbors.len() as f64;
            (re / count, im / count)
        });
        cells.push(CellOrder {
            id: cell.id.clone(),
            neighbors: neighbors.len(),
            included,
            tie_at_cutoff: tie,
            psi_real: psi.map(|value| value.0),
            psi_imaginary: psi.map(|value| value.1),
            magnitude: psi.map(|(re, im)| re.hypot(im)),
        });
    }
    let included = cells
        .iter()
        .filter(|cell| cell.included)
        .collect::<Vec<_>>();
    let count = included.len();
    let mean = |value: &dyn Fn(&CellOrder) -> f64| {
        (count > 0).then(|| included.iter().map(|cell| value(cell)).sum::<f64>() / count as f64)
    };
    let mean_magnitude = mean(&|cell| cell.magnitude.unwrap_or(0.0));
    let magnitude_of_mean = mean(&|cell| cell.psi_real.unwrap_or(0.0))
        .zip(mean(&|cell| cell.psi_imaginary.unwrap_or(0.0)))
        .map(|(re, im)| re.hypot(im));
    Ok(BondOrientationalOrderResult {
        format: BOND_ORIENTATIONAL_ORDER_FORMAT.into(),
        version: 1,
        coordinate_frame_id: request.coordinate_frame_id,
        symmetry: request.symmetry,
        neighbors: request.neighbors,
        excluded_cells: cells.len() - count,
        included_cells: count,
        cells,
        mean_magnitude,
        magnitude_of_mean,
        claim_scope: "descriptive_bond_orientational_order".into(),
    })
}
