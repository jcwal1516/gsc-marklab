//! Morisita quadrat dispersion of cell locations inside a declared tissue window.
//!
//! Square quadrats of a declared width tile the plane from a declared origin. Only quadrats
//! lying wholly inside the window are used, so every counted quadrat has the same area; a
//! quadrat that touches the boundary is clipped exactly and kept only when its clipped area
//! equals its full area. Cells are assigned to the half-open quadrat `[a, a + w) × [b, b + w)`;
//! cells in excluded quadrats are reported, not counted.
//!
//! With `q` quadrats holding counts `n_i` and `N = Σ n_i`, Morisita's index is
//! `I = q Σ n_i (n_i − 1) / (N (N − 1))`: about 1 for random placement, above 1 for clumping
//! and below 1 for regularity. The standardized index of Smith-Gill (1975) maps `I` onto
//! `[−1, 1]` using the uniform and clumped indices `M_u = (χ²_{0.025} − q + N) / (N − 1)` and
//! `M_c = (χ²_{0.975} − q + N) / (N − 1)` with `q − 1` degrees of freedom; it is unavailable
//! when none of its four cases applies (vegan then reports `I` itself). The chi-square
//! statistic `I (N − 1) + q − N` with `q − 1` degrees of freedom tests the Poisson-quadrat null
//! (upper tail). These definitions follow `vegan::dispindmorisita`.

use geo::{Coord, LineString, Polygon, Rect};
use serde::{Deserialize, Serialize};
use statrs::distribution::{ChiSquared, ContinuousCDF};

use crate::errors::{MarklabError, Result};
use crate::geom::window::StudyTile;
use crate::{ObservationWindow2D, ObservationWindowLimits};

/// Result format of [`morisita_dispersion_from_json`].
pub const MORISITA_DISPERSION_FORMAT: &str = "marklab.morisita_dispersion";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;
const FULL_AREA_RELATIVE_TOLERANCE: f64 = 1e-12;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    window: serde_json::Value,
    cells: Vec<Cell>,
    quadrat: Quadrat,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    id: String,
    x_um: f64,
    y_um: f64,
}

/// Declared quadrat grid.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Quadrat {
    pub origin_um: [f64; 2],
    pub width_um: f64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_cells: usize,
    maximum_candidate_quadrats: usize,
    maximum_clip_vertices: usize,
}

/// One counted quadrat, by grid index from the origin.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuadratCount {
    pub column: i64,
    pub row: i64,
    pub count: u64,
}

/// Morisita dispersion of one pattern.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MorisitaDispersionResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub quadrat: Quadrat,
    pub window_area_um2: f64,
    pub candidate_quadrats: usize,
    pub included_quadrats: usize,
    pub counted_cells: u64,
    pub excluded_cells: u64,
    pub quadrat_counts: Vec<QuadratCount>,
    /// `I`; unavailable with fewer than two counted cells or quadrats.
    pub morisita_index: Option<f64>,
    pub uniform_index: Option<f64>,
    pub clumped_index: Option<f64>,
    pub standardized_index: Option<f64>,
    pub chi_square: Option<f64>,
    pub degrees_of_freedom: usize,
    pub chi_square_upper_p_value: Option<f64>,
    pub unavailable: Option<String>,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a `marklab.morisita_dispersion_request` version 1 and compute the dispersion.
pub fn morisita_dispersion_from_json(input: &[u8]) -> Result<MorisitaDispersionResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("Morisita input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let limits = request.limits;
    let Quadrat {
        origin_um,
        width_um,
    } = request.quadrat.clone();
    if request.format != "marklab.morisita_dispersion_request"
        || request.version != 1
        || request.coordinate_frame_id.trim().is_empty()
        || !origin_um.iter().all(|value| value.is_finite())
        || !width_um.is_finite()
        || width_um <= 0.0
        || limits.maximum_cells == 0
        || request.cells.len() > limits.maximum_cells
        || limits.maximum_candidate_quadrats == 0
    {
        return Err(invalid(
            "Morisita request format, frame, quadrat or limits are invalid",
        ));
    }
    let window = ObservationWindow2D::from_geojson_str(
        &serde_json::to_string(&request.window)?,
        ObservationWindowLimits::default(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let mut ids = std::collections::HashSet::new();
    for cell in &request.cells {
        if cell.id.trim().is_empty()
            || !ids.insert(cell.id.as_str())
            || !window.contains(cell.x_um, cell.y_um)
        {
            return Err(invalid(
                "Morisita cells need unique identifiers and finite coordinates inside the window",
            ));
        }
    }
    let [min_x, min_y, max_x, max_y] = window.descriptor().bounds_um;
    let index = |value: f64, origin: f64| ((value - origin) / width_um).floor();
    let (first_column, last_column) = (index(min_x, origin_um[0]), index(max_x, origin_um[0]));
    let (first_row, last_row) = (index(min_y, origin_um[1]), index(max_y, origin_um[1]));
    let candidates = (last_column - first_column + 1.0) * (last_row - first_row + 1.0);
    if !candidates.is_finite() || candidates > limits.maximum_candidate_quadrats as f64 {
        return Err(invalid(
            "Morisita quadrat grid exceeds maximum_candidate_quadrats",
        ));
    }
    let columns = (last_column - first_column) as i64 + 1;
    let first = (first_column as i64, first_row as i64);
    let mut included = vec![false; candidates as usize];
    for (slot, flag) in included.iter_mut().enumerate() {
        let column = first.0 + slot as i64 % columns;
        let row = first.1 + slot as i64 / columns;
        *flag = quadrat_inside(
            &window,
            origin_um,
            width_um,
            column,
            row,
            limits.maximum_clip_vertices,
        )?;
    }
    let mut counts = vec![0_u64; included.len()];
    let mut excluded_cells = 0_u64;
    for cell in &request.cells {
        let column = index(cell.x_um, origin_um[0]) as i64 - first.0;
        let row = index(cell.y_um, origin_um[1]) as i64 - first.1;
        let slot = (row * columns + column) as usize;
        if included[slot] {
            counts[slot] += 1;
        } else {
            excluded_cells += 1;
        }
    }
    let quadrat_counts = included
        .iter()
        .zip(&counts)
        .enumerate()
        .filter(|(_, (included, _))| **included)
        .map(|(slot, (_, count))| QuadratCount {
            column: first.0 + slot as i64 % columns,
            row: first.1 + slot as i64 / columns,
            count: *count,
        })
        .collect::<Vec<_>>();
    Ok(dispersion(
        request.coordinate_frame_id,
        request.quadrat,
        window.area_um2(),
        included.len(),
        quadrat_counts,
        excluded_cells,
    ))
}

/// Whether the closed quadrat lies wholly inside the window.
fn quadrat_inside(
    window: &ObservationWindow2D,
    origin: [f64; 2],
    width: f64,
    column: i64,
    row: i64,
    maximum_clip_vertices: usize,
) -> Result<bool> {
    let x0 = origin[0] + column as f64 * width;
    let y0 = origin[1] + row as f64 * width;
    let tile = Rect::new(
        Coord { x: x0, y: y0 },
        Coord {
            x: x0 + width,
            y: y0 + width,
        },
    );
    match window.classify_study_tile(&tile) {
        StudyTile::Inside { .. } => Ok(true),
        StudyTile::Outside => Ok(false),
        StudyTile::Boundary => {
            let polygon = Polygon::new(
                LineString::from(vec![
                    (x0, y0),
                    (x0 + width, y0),
                    (x0 + width, y0 + width),
                    (x0, y0 + width),
                    (x0, y0),
                ]),
                Vec::new(),
            );
            let full = width * width;
            let (clipped, _) = window
                .clip_study_polygon(&polygon, maximum_clip_vertices)
                .map_err(|error| invalid(error.to_string()))?;
            Ok((full - clipped).abs() <= FULL_AREA_RELATIVE_TOLERANCE * full)
        }
    }
}

fn dispersion(
    coordinate_frame_id: String,
    quadrat: Quadrat,
    window_area_um2: f64,
    candidate_quadrats: usize,
    quadrat_counts: Vec<QuadratCount>,
    excluded_cells: u64,
) -> MorisitaDispersionResult {
    let q = quadrat_counts.len();
    let total = quadrat_counts.iter().map(|row| row.count).sum::<u64>();
    let mut result = MorisitaDispersionResult {
        format: MORISITA_DISPERSION_FORMAT.into(),
        version: 1,
        coordinate_frame_id,
        quadrat,
        window_area_um2,
        candidate_quadrats,
        included_quadrats: q,
        counted_cells: total,
        excluded_cells,
        quadrat_counts,
        morisita_index: None,
        uniform_index: None,
        clumped_index: None,
        standardized_index: None,
        chi_square: None,
        degrees_of_freedom: q.saturating_sub(1),
        chi_square_upper_p_value: None,
        unavailable: None,
        claim_scope: "descriptive_quadrat_dispersion_poisson_quadrat_null".into(),
    };
    if q < 2 || total < 2 {
        result.unavailable =
            Some("Morisita dispersion needs at least two quadrats and two counted cells".into());
        return result;
    }
    let (q_f, n) = (q as f64, total as f64);
    let pairs = result
        .quadrat_counts
        .iter()
        .map(|row| row.count as f64 * (row.count as f64 - 1.0))
        .sum::<f64>();
    let index = q_f * pairs / (n * (n - 1.0));
    let chi = ChiSquared::new(q_f - 1.0).expect("positive degrees of freedom");
    let uniform = (chi.inverse_cdf(0.025) - q_f + n) / (n - 1.0);
    let clumped = (chi.inverse_cdf(0.975) - q_f + n) / (n - 1.0);
    let statistic = index * (n - 1.0) + q_f - n;
    result.standardized_index = if index >= clumped && clumped > 1.0 {
        Some(0.5 + 0.5 * (index - clumped) / (q_f - clumped))
    } else if clumped > index && index >= 1.0 {
        Some(0.5 * (index - 1.0) / (clumped - 1.0))
    } else if 1.0 > index && index > uniform {
        Some(-0.5 * (index - 1.0) / (uniform - 1.0))
    } else if 1.0 > uniform && uniform > index {
        Some(-0.5 + 0.5 * (index - uniform) / uniform)
    } else {
        None
    };
    result.morisita_index = Some(index);
    result.uniform_index = Some(uniform);
    result.clumped_index = Some(clumped);
    result.chi_square = Some(statistic);
    result.chi_square_upper_p_value = Some(chi.sf(statistic));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(values: &[u64]) -> Vec<QuadratCount> {
        values
            .iter()
            .enumerate()
            .map(|(index, count)| QuadratCount {
                column: index as i64,
                row: 0,
                count: *count,
            })
            .collect()
    }

    fn compute(values: &[u64]) -> MorisitaDispersionResult {
        let quadrat = Quadrat {
            origin_um: [0.0, 0.0],
            width_um: 1.0,
        };
        dispersion("f".into(), quadrat, 1.0, values.len(), counts(values), 0)
    }

    #[test]
    fn hand_counted_indices_and_degenerate_inputs() {
        // q = 4, N = 4, sum n(n-1) = 2 + 2: I = 4 * 4 / 12.
        let clumped = compute(&[0, 2, 0, 2]);
        assert_eq!(clumped.morisita_index, Some(4.0 * 4.0 / 12.0));
        assert_eq!(clumped.chi_square, Some(4.0 / 3.0 * 3.0 + 4.0 - 4.0));
        // Perfectly even counts have no within-quadrat pairs.
        assert_eq!(compute(&[1, 1, 1, 1]).morisita_index, Some(0.0));
        let empty = compute(&[1, 0, 0]);
        assert!(empty.morisita_index.is_none() && empty.unavailable.is_some());
    }
}
