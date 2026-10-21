//! Structure factor and hyperuniformity indices of cells in a fully observed rectangle.
//!
//! * **Structure factor:** the scattering intensity `S(k) = |Σ_j e^{−i k·x_j}|² / N` at the
//!   allowed wavevectors `k = 2π (m_x / L_x, m_y / L_y)`, `m ≠ 0`, with `|m_a| ≤ ⌊k_max L_a / 2π⌋`.
//!   At these wavevectors the rectangle's Fourier transform vanishes, so the window adds no
//!   bias (Hawat et al. 2023). `S → 1` at large `k` for any simple point pattern; Poisson
//!   patterns have `S = 1` everywhere.
//! * **Binned spectrum:** the mean of `S` over wavevectors with `|k|` in each declared bin
//!   `[e_b, e_{b+1})`, with the standard error `sd / √count` (population standard deviation).
//! * **Indices** from the nonempty bins whose centres lie below the declared stop:
//!   `Ŝ(0)`, the intercept of the least-squares line through the binned means;
//!   `H = Ŝ(0) / max(S_peak, 1)` with `S_peak` the first local maximum of the binned means
//!   that reaches 1 (Torquato 2018); and `S ≈ c |k|^α` by least squares, solved exactly for
//!   `c` at each `α` and minimized over `α` in `[−10, 10]`.
//!
//! Every cell of the pattern must lie in the rectangle and the rectangle must be fully observed;
//! empty tissue inside it reads as large-scale density fluctuation.

use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

/// Result format of [`hyperuniformity_from_json`].
pub const HYPERUNIFORMITY_FORMAT: &str = "marklab.hyperuniformity";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    window: Rectangle,
    cells: Vec<Cell>,
    maximum_wavenumber_per_axis: f64,
    wavenumber_bin_edges: Vec<f64>,
    fit_wavenumber_stop: f64,
    limits: Limits,
}

/// Fully observed rectangle `[x0, x0 + L_x) × [y0, y0 + L_y)`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rectangle {
    pub origin_um: [f64; 2],
    pub side_um: [f64; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    id: String,
    x_um: f64,
    y_um: f64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_cells: usize,
    maximum_wavevector_evaluations: u64,
}

/// `S` at one allowed wavevector.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WavevectorValue {
    pub m: [i64; 2],
    pub k_per_um: [f64; 2],
    pub structure_factor: f64,
}

/// Binned spectrum.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WavenumberBin {
    pub lower: f64,
    pub upper: f64,
    pub center: f64,
    pub wavevectors: usize,
    pub mean: Option<f64>,
    pub standard_error: Option<f64>,
}

/// Structure factor and hyperuniformity indices of one pattern.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HyperuniformityResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub window: Rectangle,
    pub cells: usize,
    pub wavevectors: Vec<WavevectorValue>,
    pub bins: Vec<WavenumberBin>,
    pub fit_wavenumber_stop: f64,
    pub fitted_bins: usize,
    /// Intercept and slope of the line through the fitted binned means.
    pub s0_intercept: Option<f64>,
    pub line_slope: Option<f64>,
    /// Index into `bins` of the first local maximum reaching 1.
    pub first_peak_bin: Option<usize>,
    pub h_index: Option<f64>,
    pub power_law_alpha: Option<f64>,
    pub power_law_c: Option<f64>,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a `marklab.hyperuniformity_request` version 1 and compute it.
pub fn hyperuniformity_from_json(input: &[u8]) -> Result<HyperuniformityResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("hyperuniformity input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let window = request.window;
    let k_max = request.maximum_wavenumber_per_axis;
    let edges = &request.wavenumber_bin_edges;
    let valid_window = window.origin_um.iter().all(|value| value.is_finite())
        && window
            .side_um
            .iter()
            .all(|side| side.is_finite() && *side > 0.0);
    if request.format != "marklab.hyperuniformity_request"
        || request.version != 1
        || request.coordinate_frame_id.trim().is_empty()
        || !valid_window
        || !(k_max.is_finite() && k_max > 0.0)
        || !(2..=request.limits.maximum_cells).contains(&request.cells.len())
        || !(2..=1025).contains(&edges.len())
        || !edges.iter().all(|edge| edge.is_finite())
        || edges[0] < 0.0
        || edges.windows(2).any(|pair| pair[0] >= pair[1])
        || !request.fit_wavenumber_stop.is_finite()
    {
        return Err(invalid(
            "hyperuniformity format, window, wavenumber limit, bin edges (2 to 1025, \
             increasing, from 0), fit stop or cell count are invalid",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    let inside = |value: f64, axis: usize| {
        value >= window.origin_um[axis] && value < window.origin_um[axis] + window.side_um[axis]
    };
    if !request.cells.iter().all(|cell| {
        !cell.id.trim().is_empty()
            && ids.insert(cell.id.as_str())
            && inside(cell.x_um, 0)
            && inside(cell.y_um, 1)
    }) {
        return Err(invalid(
            "cells need unique identifiers and positions inside the rectangle",
        ));
    }
    let tau = std::f64::consts::TAU;
    let reach = window
        .side_um
        .map(|side| (k_max * side / tau).floor() as i64);
    let count = request.cells.len();
    let vectors = (2 * reach[0] as u64 + 1) * (2 * reach[1] as u64 + 1) - 1;
    if reach.iter().any(|&m| m > 100_000)
        || vectors == 0
        || vectors.saturating_mul(count as u64) > request.limits.maximum_wavevector_evaluations
    {
        return Err(invalid(
            "no allowed wavevector lies within the limit, or cells times wavevectors exceed \
             the declared evaluation limit",
        ));
    }
    let offsets = request
        .cells
        .iter()
        .map(|cell| {
            (
                cell.x_um - window.origin_um[0],
                cell.y_um - window.origin_um[1],
            )
        })
        .collect::<Vec<_>>();
    let mut wavevectors = Vec::with_capacity(vectors as usize);
    for mx in -reach[0]..=reach[0] {
        for my in -reach[1]..=reach[1] {
            if mx == 0 && my == 0 {
                continue;
            }
            let k = [
                tau * mx as f64 / window.side_um[0],
                tau * my as f64 / window.side_um[1],
            ];
            let (re, im) = offsets.iter().fold((0.0, 0.0), |(re, im), &(dx, dy)| {
                let phase = k[0] * dx + k[1] * dy;
                (re + phase.cos(), im - phase.sin())
            });
            wavevectors.push(WavevectorValue {
                m: [mx, my],
                k_per_um: k,
                structure_factor: (re * re + im * im) / count as f64,
            });
        }
    }

    let mut groups = vec![Vec::<f64>::new(); edges.len() - 1];
    for value in &wavevectors {
        let norm = value.k_per_um[0].hypot(value.k_per_um[1]);
        if norm >= edges[0] && norm < edges[edges.len() - 1] {
            groups[edges.partition_point(|&edge| edge <= norm) - 1].push(value.structure_factor);
        }
    }
    let bins = groups
        .iter()
        .enumerate()
        .map(|(bin, values)| {
            let n = values.len() as f64;
            let mean = (!values.is_empty()).then(|| values.iter().sum::<f64>() / n);
            WavenumberBin {
                lower: edges[bin],
                upper: edges[bin + 1],
                center: (edges[bin] + edges[bin + 1]) / 2.0,
                wavevectors: values.len(),
                mean,
                standard_error: mean.map(|mean| {
                    (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n).sqrt() / n.sqrt()
                }),
            }
        })
        .collect::<Vec<_>>();
    let observed = bins
        .iter()
        .enumerate()
        .filter_map(|(index, bin)| bin.mean.map(|mean| (index, bin.center, mean)))
        .collect::<Vec<_>>();
    let fitted = observed
        .iter()
        .filter(|&&(_, center, _)| center < request.fit_wavenumber_stop)
        .map(|&(_, center, mean)| (center, mean))
        .collect::<Vec<_>>();
    let line = line_fit(&fitted);
    let first_peak = observed.windows(3).find_map(|triple| {
        let (before, (index, _, value), after) = (triple[0].2, triple[1], triple[2].2);
        (value > before && value > after && value >= 1.0).then_some((index, value))
    });
    let power = power_fit(&fitted);
    Ok(HyperuniformityResult {
        format: HYPERUNIFORMITY_FORMAT.into(),
        version: 1,
        coordinate_frame_id: request.coordinate_frame_id,
        window,
        cells: count,
        wavevectors,
        bins,
        fit_wavenumber_stop: request.fit_wavenumber_stop,
        fitted_bins: fitted.len(),
        s0_intercept: line.map(|value| value.0),
        line_slope: line.map(|value| value.1),
        first_peak_bin: first_peak.map(|value| value.0),
        h_index: line.map(|(intercept, _)| intercept / first_peak.map_or(1.0, |p| p.1.max(1.0))),
        power_law_alpha: power.map(|value| value.0),
        power_law_c: power.map(|value| value.1),
        claim_scope: "descriptive_structure_factor_of_fully_observed_rectangle".into(),
    })
}

/// Least-squares line `y = a + b x`; needs two distinct `x`.
fn line_fit(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    let n = points.len() as f64;
    let mean_x = points.iter().map(|p| p.0).sum::<f64>() / n;
    let mean_y = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx = points.iter().map(|p| (p.0 - mean_x).powi(2)).sum::<f64>();
    let sxy = points
        .iter()
        .map(|p| (p.0 - mean_x) * (p.1 - mean_y))
        .sum::<f64>();
    (points.len() >= 2 && sxx > 0.0).then(|| {
        let slope = sxy / sxx;
        (mean_y - slope * mean_x, slope)
    })
}

/// Least squares of `y = c x^α` over `α ∈ [−10, 10]`: for each `α` the best `c` is
/// `Σ y x^α / Σ x^{2α}`, leaving `Σ y² − (Σ y x^α)² / Σ x^{2α}` to minimize. A grid of step
/// 0.005 locates the minimum and a golden-section search refines it.
fn power_fit(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    if points.len() < 2 || points.iter().any(|&(x, _)| x <= 0.0) {
        return None;
    }
    let parts = |alpha: f64| {
        points.iter().fold((0.0, 0.0), |(cross, square), &(x, y)| {
            let power = x.powf(alpha);
            (cross + y * power, square + power * power)
        })
    };
    let loss = |alpha: f64| {
        let (cross, square) = parts(alpha);
        -(cross * cross) / square
    };
    let steps = 4000;
    let grid = |step: usize| -10.0 + 20.0 * step as f64 / steps as f64;
    let best = (0..=steps)
        .map(grid)
        .min_by(|a, b| loss(*a).total_cmp(&loss(*b)))?;
    let (mut low, mut high) = ((best - 0.005).max(-10.0), (best + 0.005).min(10.0));
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..200 {
        let (left, right) = (high - ratio * (high - low), low + ratio * (high - low));
        if loss(left) <= loss(right) {
            high = right;
        } else {
            low = left;
        }
    }
    let alpha = (low + high) / 2.0;
    let (cross, square) = parts(alpha);
    (square > 0.0 && square.is_finite()).then_some((alpha, cross / square))
}
