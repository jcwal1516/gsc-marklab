//! Landy–Szalay two-point correlation of cells against a declared random catalog.
//!
//! The random catalog samples the observed tissue: its points carry the window's shape,
//! holes and any sampling design, which is how the estimator corrects for edges. For each
//! distance bin `[e_b, e_{b+1})`, with `n` cells and `n_R` random points,
//!
//! `ξ(b) = (DD_n − 2 DR_n + RR_n) / RR_n`, where `DD_n = DD / (n(n−1)/2)`,
//! `DR_n = DR / (n n_R)` and `RR_n = RR / (n_R(n_R−1)/2)`
//!
//! (Landy and Szalay 1993), counting each unordered pair once. `ξ` is unavailable for a bin
//! without random pairs. No uncertainty is reported: pair counts are correlated across bins and
//! Poisson errors understate the variance of clustered patterns.

use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};
use crate::geom::spatial_index::SpatialIndex2D;
use crate::{ObservationWindow2D, ObservationWindowLimits};

/// Result format of [`landy_szalay_from_json`].
pub const LANDY_SZALAY_FORMAT: &str = "marklab.landy_szalay";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    window: serde_json::Value,
    cells: Vec<Point>,
    random_catalog_id: String,
    random_points: Vec<[f64; 2]>,
    bin_edges_um: Vec<f64>,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    id: String,
    x_um: f64,
    y_um: f64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_cells: usize,
    maximum_random_points: usize,
    maximum_pair_evaluations: u64,
}

/// One distance bin.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CorrelationBin {
    pub lower_um: f64,
    pub upper_um: f64,
    pub data_data_pairs: u64,
    pub data_random_pairs: u64,
    pub random_random_pairs: u64,
    pub xi: Option<f64>,
}

/// Landy–Szalay correlation of one pattern.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LandySzalayResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub random_catalog_id: String,
    pub cells: usize,
    pub random_points: usize,
    pub bins: Vec<CorrelationBin>,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a `marklab.landy_szalay_request` version 1 and estimate `ξ`.
pub fn landy_szalay_from_json(input: &[u8]) -> Result<LandySzalayResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("Landy-Szalay input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let (n, n_random) = (request.cells.len(), request.random_points.len());
    let edges = &request.bin_edges_um;
    // Worst case: every pair of every kind lies within the last edge.
    let work = [(n, n), (n, n_random), (n_random, n_random)]
        .iter()
        .fold(0_u64, |total, &(a, b)| {
            total.saturating_add((a as u64).saturating_mul(b as u64))
        });
    if request.format != "marklab.landy_szalay_request"
        || request.version != 1
        || request.coordinate_frame_id.trim().is_empty()
        || request.random_catalog_id.trim().is_empty()
        || !(2..=request.limits.maximum_cells).contains(&n)
        || !(2..=request.limits.maximum_random_points).contains(&n_random)
        || work > request.limits.maximum_pair_evaluations
        || !(2..=257).contains(&edges.len())
        || !edges.iter().all(|edge| edge.is_finite())
        || edges[0] < 0.0
        || edges.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "Landy-Szalay format, catalog sizes, bin edges (2 to 257, increasing, from 0) or \
             limits are invalid",
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
    }) || !request
        .random_points
        .iter()
        .all(|point| window.contains(point[0], point[1]))
    {
        return Err(invalid(
            "cells need unique identifiers, and cells and random points must lie inside the \
             window",
        ));
    }
    let cell_x = request.cells.iter().map(|c| c.x_um).collect::<Vec<_>>();
    let cell_y = request.cells.iter().map(|c| c.y_um).collect::<Vec<_>>();
    let random_x = request
        .random_points
        .iter()
        .map(|p| p[0])
        .collect::<Vec<_>>();
    let random_y = request
        .random_points
        .iter()
        .map(|p| p[1])
        .collect::<Vec<_>>();
    let cells = SpatialIndex2D::new(&cell_x, &cell_y)?;
    let randoms = SpatialIndex2D::new(&random_x, &random_y)?;
    let reach = edges[edges.len() - 1];
    let bins = edges.len() - 1;
    let bin_of = |distance: f64| {
        (distance >= edges[0] && distance < reach)
            .then(|| edges.partition_point(|&edge| edge <= distance) - 1)
    };
    let mut data_data = vec![0_u64; bins];
    let mut data_random = vec![0_u64; bins];
    let mut random_random = vec![0_u64; bins];
    for cell in 0..n {
        cells.visit_within_radius(cell, reach, |neighbor| {
            if neighbor.index > cell {
                if let Some(bin) = bin_of(neighbor.distance_um) {
                    data_data[bin] += 1;
                }
            }
        })?;
        randoms.visit_points_within_radius(cell_x[cell], cell_y[cell], reach, |neighbor| {
            if let Some(bin) = bin_of(neighbor.distance_um) {
                data_random[bin] += 1;
            }
        })?;
    }
    for point in 0..n_random {
        randoms.visit_within_radius(point, reach, |neighbor| {
            if neighbor.index > point {
                if let Some(bin) = bin_of(neighbor.distance_um) {
                    random_random[bin] += 1;
                }
            }
        })?;
    }
    let (nd, nr) = (n as f64, n_random as f64);
    let data_pairs = nd * (nd - 1.0) / 2.0;
    let cross_pairs = nd * nr;
    let random_pairs = nr * (nr - 1.0) / 2.0;
    let bins = (0..bins)
        .map(|bin| {
            let dd = data_data[bin] as f64 / data_pairs;
            let dr = data_random[bin] as f64 / cross_pairs;
            let rr = random_random[bin] as f64 / random_pairs;
            CorrelationBin {
                lower_um: edges[bin],
                upper_um: edges[bin + 1],
                data_data_pairs: data_data[bin],
                data_random_pairs: data_random[bin],
                random_random_pairs: random_random[bin],
                xi: (random_random[bin] > 0).then(|| (dd - 2.0 * dr + rr) / rr),
            }
        })
        .collect();
    Ok(LandySzalayResult {
        format: LANDY_SZALAY_FORMAT.into(),
        version: 1,
        coordinate_frame_id: request.coordinate_frame_id,
        random_catalog_id: request.random_catalog_id,
        cells: n,
        random_points: n_random,
        bins,
        claim_scope: "descriptive_two_point_correlation_against_declared_random_catalog".into(),
    })
}
