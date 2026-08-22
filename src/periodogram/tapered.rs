use std::collections::BTreeMap;

use crate::{
    data::Pattern,
    periodogram::{fft2::fft2_power_spectrum, raster::centered_mark_raster, taper::hann_weight},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HannTaperedPeriodogramSummary {
    pub raster_width: usize,
    pub raster_height: usize,
    pub n_modes: usize,
    pub n_radial_shells: usize,
    pub low_k_power: f64,
    pub normalized_low_k_power: f64,
}

pub fn hann_tapered_raster_periodogram(
    pattern: &Pattern,
    cell_size_um: f64,
    low_k_shells: usize,
) -> Option<HannTaperedPeriodogramSummary> {
    let (spec, mut raster) = centered_mark_raster(pattern, cell_size_um)?;
    if spec.width < 2 || spec.height < 2 {
        return None;
    }

    apply_separable_hann_taper(&mut raster, spec.width, spec.height)?;
    let power = fft2_power_spectrum(&raster, spec.width, spec.height)?;
    let shell_means = radial_shell_means(&power, spec.width, spec.height, spec.cell_size_um)?;
    let low_count = low_k_shells.max(1).min(shell_means.len());
    let low_k_power = shell_means[..low_count].iter().sum::<f64>() / low_count as f64;
    let mean_power = shell_means.iter().sum::<f64>() / shell_means.len() as f64;
    let normalized_low_k_power = low_k_power / mean_power.max(f64::EPSILON);

    Some(HannTaperedPeriodogramSummary {
        raster_width: spec.width,
        raster_height: spec.height,
        n_modes: power.len().saturating_sub(1),
        n_radial_shells: shell_means.len(),
        low_k_power,
        normalized_low_k_power,
    })
}

fn radial_shell_means(
    power: &[f64],
    width: usize,
    height: usize,
    cell_size_um: f64,
) -> Option<Vec<f64>> {
    if width == 0
        || height == 0
        || power.len() != width.checked_mul(height)?
        || !cell_size_um.is_finite()
        || cell_size_um <= 0.0
        || power.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    // Annuli have the smallest Fourier spacing of the longer raster dimension.
    // Nonempty shell means receive equal weight in the low-frequency summary.
    let shell_width = 1.0 / (width.max(height) as f64 * cell_size_um);
    let mut shells = BTreeMap::<usize, (f64, usize)>::new();
    for y in 0..height {
        for x in 0..width {
            if x == 0 && y == 0 {
                continue;
            }
            let fx = x.min(width - x) as f64 / (width as f64 * cell_size_um);
            let fy = y.min(height - y) as f64 / (height as f64 * cell_size_um);
            let shell = (fx.hypot(fy) / shell_width).floor().max(1.0) as usize;
            let entry = shells.entry(shell).or_default();
            entry.0 += power[y * width + x];
            entry.1 += 1;
        }
    }

    let means = shells
        .into_values()
        .filter_map(|(sum, count)| (count > 0).then_some(sum / count as f64))
        .collect::<Vec<_>>();
    (!means.is_empty()).then_some(means)
}

fn apply_separable_hann_taper(field: &mut [f32], width: usize, height: usize) -> Option<()> {
    if width == 0 || height == 0 || field.len() != width.checked_mul(height)? {
        return None;
    }
    for y in 0..height {
        let wy = hann_weight(y, height);
        for x in 0..width {
            let wx = hann_weight(x, width);
            field[y * width + x] *= (wx * wy) as f32;
        }
    }
    Some(())
}
