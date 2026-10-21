//! Box-counting dimension and gliding-box lacunarity of a declared binary mask.
//!
//! The mask is a raster of declared pixel size; rows are strings of `0` and `1`, the first row
//! at the top. Box sizes are declared in pixels and used in increasing order.
//!
//! * **Box counting:** boxes of side `s` tile the raster from its top-left corner; a box that
//!   extends past the right or bottom edge covers only the pixels inside. `N(s)` counts boxes
//!   holding at least one foreground pixel. The dimension estimate is the negative ordinary
//!   least-squares slope of `ln N(s)` on `ln s`, reported with its intercept and `R²`.
//! * **Lacunarity** (Allain and Cloitre 1991): an `s × s` box glides over every position lying
//!   wholly inside the raster; with box masses `M`, `Λ(s) = E[M²] / E[M]²`. It is unavailable when
//!   every box is empty.
//!
//! Both depend on pixel size, the foreground definition, the grid origin and the box sizes, so
//! those are part of the declared design.

use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

/// Result format of [`lacunarity_fractal_dimension_from_json`].
pub const LACUNARITY_FRACTAL_DIMENSION_FORMAT: &str = "marklab.lacunarity_fractal_dimension";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    mask_id: String,
    pixel_size_um: f64,
    foreground_definition: String,
    rows: Vec<String>,
    box_sizes_px: Vec<usize>,
    limits: Limits,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_pixels: usize,
    maximum_box_sizes: usize,
}

/// One box size.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BoxScale {
    pub box_size_px: usize,
    pub box_size_um: f64,
    /// Boxes of the corner-anchored tiling that hold at least one foreground pixel.
    pub occupied_boxes: u64,
    /// Gliding-box positions wholly inside the raster.
    pub gliding_positions: u64,
    pub mean_mass: Option<f64>,
    pub lacunarity: Option<f64>,
}

/// Box-counting dimension and lacunarity of one mask.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LacunarityFractalDimensionResult {
    pub format: String,
    pub version: u32,
    pub mask_id: String,
    pub pixel_size_um: f64,
    pub foreground_definition: String,
    pub width_px: usize,
    pub height_px: usize,
    pub foreground_pixels: u64,
    pub scales: Vec<BoxScale>,
    /// `−slope` of `ln N` on `ln s`; unavailable with fewer than two occupied scales.
    pub box_counting_dimension: Option<f64>,
    pub regression_intercept: Option<f64>,
    /// Unavailable also when `N` is the same at every occupied scale.
    pub regression_r_squared: Option<f64>,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a `marklab.lacunarity_fractal_dimension_request` version 1 and measure it.
pub fn lacunarity_fractal_dimension_from_json(
    input: &[u8],
) -> Result<LacunarityFractalDimensionResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("lacunarity input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let height = request.rows.len();
    let width = request.rows.first().map_or(0, String::len);
    let pixels = width.saturating_mul(height);
    let sizes = &request.box_sizes_px;
    if request.format != "marklab.lacunarity_fractal_dimension_request"
        || request.version != 1
        || request.mask_id.trim().is_empty()
        || request.foreground_definition.trim().is_empty()
        || !request.pixel_size_um.is_finite()
        || request.pixel_size_um <= 0.0
        || width == 0
        || pixels > request.limits.maximum_pixels
        || !request
            .rows
            .iter()
            .all(|row| row.len() == width && row.bytes().all(|byte| byte == b'0' || byte == b'1'))
        || sizes.is_empty()
        || sizes.len() > request.limits.maximum_box_sizes
        || sizes.windows(2).any(|pair| pair[0] >= pair[1])
        || sizes[0] == 0
        || sizes[sizes.len() - 1] > width.min(height)
    {
        return Err(invalid(
            "lacunarity mask, box sizes (increasing, 1..=min side) or limits are invalid",
        ));
    }
    // Summed-area table with one zero row and column: sum over [r0, r1) x [c0, c1).
    let stride = width + 1;
    let mut summed = vec![0_u64; (height + 1) * stride];
    for (row, text) in request.rows.iter().enumerate() {
        let mut running = 0_u64;
        for (column, byte) in text.bytes().enumerate() {
            running += u64::from(byte == b'1');
            summed[(row + 1) * stride + column + 1] = summed[row * stride + column + 1] + running;
        }
    }
    let mass = |r0: usize, c0: usize, r1: usize, c1: usize| {
        summed[r1 * stride + c1] + summed[r0 * stride + c0]
            - summed[r0 * stride + c1]
            - summed[r1 * stride + c0]
    };
    let foreground_pixels = mass(0, 0, height, width);
    let scales = sizes
        .iter()
        .map(|&size| {
            let mut occupied = 0_u64;
            for r0 in (0..height).step_by(size) {
                for c0 in (0..width).step_by(size) {
                    occupied += u64::from(
                        mass(r0, c0, (r0 + size).min(height), (c0 + size).min(width)) > 0,
                    );
                }
            }
            let (mut first, mut second, mut positions) = (0_u128, 0_u128, 0_u64);
            for r0 in 0..=height - size {
                for c0 in 0..=width - size {
                    let value = u128::from(mass(r0, c0, r0 + size, c0 + size));
                    first += value;
                    second += value * value;
                    positions += 1;
                }
            }
            let mean = first as f64 / positions as f64;
            BoxScale {
                box_size_px: size,
                box_size_um: size as f64 * request.pixel_size_um,
                occupied_boxes: occupied,
                gliding_positions: positions,
                mean_mass: Some(mean),
                lacunarity: (first > 0)
                    .then(|| second as f64 * positions as f64 / (first as f64 * first as f64)),
            }
        })
        .collect::<Vec<_>>();
    let points = scales
        .iter()
        .filter(|scale| scale.occupied_boxes > 0)
        .map(|scale| {
            (
                (scale.box_size_px as f64).ln(),
                (scale.occupied_boxes as f64).ln(),
            )
        })
        .collect::<Vec<_>>();
    let (dimension, intercept, r_squared) = regression(&points);
    Ok(LacunarityFractalDimensionResult {
        format: LACUNARITY_FRACTAL_DIMENSION_FORMAT.into(),
        version: 1,
        mask_id: request.mask_id,
        pixel_size_um: request.pixel_size_um,
        foreground_definition: request.foreground_definition,
        width_px: width,
        height_px: height,
        foreground_pixels,
        scales,
        box_counting_dimension: dimension,
        regression_intercept: intercept,
        regression_r_squared: r_squared,
        claim_scope: "descriptive_declared_mask_scaling".into(),
    })
}

/// Ordinary least squares of `y` on `x`: (`−slope`, intercept, `R²`).
fn regression(points: &[(f64, f64)]) -> (Option<f64>, Option<f64>, Option<f64>) {
    let count = points.len() as f64;
    if points.len() < 2 {
        return (None, None, None);
    }
    let mean_x = points.iter().map(|p| p.0).sum::<f64>() / count;
    let mean_y = points.iter().map(|p| p.1).sum::<f64>() / count;
    let sxx = points.iter().map(|p| (p.0 - mean_x).powi(2)).sum::<f64>();
    let sxy = points
        .iter()
        .map(|p| (p.0 - mean_x) * (p.1 - mean_y))
        .sum::<f64>();
    let syy = points.iter().map(|p| (p.1 - mean_y).powi(2)).sum::<f64>();
    let slope = sxy / sxx;
    // R^2 = 1 - RSS / TSS is undefined when the counts are constant (TSS = 0).
    let r_squared = (syy > 0.0).then(|| sxy * sxy / (sxx * syy));
    (Some(-slope), Some(mean_y - slope * mean_x), r_squared)
}
