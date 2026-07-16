#[derive(Clone, Debug, PartialEq)]
pub struct RasterSpec {
    pub width: usize,
    pub height: usize,
    pub cell_size_um: f64,
}

use crate::data::Pattern;

pub fn centered_mark_raster(
    pattern: &Pattern,
    cell_size_um: f64,
) -> Option<(RasterSpec, Vec<f32>)> {
    centered_mark_raster_for_marks(pattern, &pattern.mark, cell_size_um)
}

pub fn centered_mark_raster_for_marks(
    pattern: &Pattern,
    marks: &[u8],
    cell_size_um: f64,
) -> Option<(RasterSpec, Vec<f32>)> {
    let mut raster = Vec::new();
    let spec = centered_mark_raster_for_marks_into(pattern, marks, cell_size_um, &mut raster)?;
    Some((spec, raster))
}

pub fn centered_mark_raster_for_marks_into(
    pattern: &Pattern,
    marks: &[u8],
    cell_size_um: f64,
    raster: &mut Vec<f32>,
) -> Option<RasterSpec> {
    if pattern.is_empty()
        || marks.len() != pattern.len()
        || marks.iter().any(|mark| *mark != 0 && *mark != 1)
        || cell_size_um <= 0.0
        || !cell_size_um.is_finite()
    {
        return None;
    }

    let min_x = pattern.x_um.iter().copied().reduce(f64::min)?;
    let max_x = pattern.x_um.iter().copied().reduce(f64::max)?;
    let min_y = pattern.y_um.iter().copied().reduce(f64::min)?;
    let max_y = pattern.y_um.iter().copied().reduce(f64::max)?;
    let width = (((max_x - min_x) / cell_size_um).floor() as usize).saturating_add(1);
    let height = (((max_y - min_y) / cell_size_um).floor() as usize).saturating_add(1);
    raster.clear();
    raster.resize(width.checked_mul(height)?, 0.0);
    let p_hat = marks.iter().filter(|mark| **mark == 1).count() as f64 / marks.len() as f64;

    for ((x, y), mark) in pattern
        .x_um
        .iter()
        .copied()
        .zip(pattern.y_um.iter().copied())
        .zip(marks.iter().copied())
    {
        let ix = ((x - min_x) / cell_size_um).floor() as usize;
        let iy = ((y - min_y) / cell_size_um).floor() as usize;
        if ix < width && iy < height {
            raster[iy * width + ix] += (f64::from(mark) - p_hat) as f32;
        }
    }

    Some(RasterSpec {
        width,
        height,
        cell_size_um,
    })
}
