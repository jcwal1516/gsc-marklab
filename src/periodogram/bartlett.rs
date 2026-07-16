use crate::{
    data::Pattern,
    periodogram::{fft2::fft2_power_spectrum, raster::centered_mark_raster, taper::hann_weight},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BartlettPeriodogramSummary {
    pub raster_width: usize,
    pub raster_height: usize,
    pub n_modes: usize,
    pub low_k_power: f64,
    pub normalized_low_k_power: f64,
}

pub fn marked_bartlett_periodogram(
    pattern: &Pattern,
    cell_size_um: f64,
    low_k_shells: usize,
) -> Option<BartlettPeriodogramSummary> {
    let (spec, mut raster) = centered_mark_raster(pattern, cell_size_um)?;
    if spec.width < 2 || spec.height < 2 {
        return None;
    }

    apply_separable_hann_taper(&mut raster, spec.width, spec.height)?;
    let power = fft2_power_spectrum(&raster, spec.width, spec.height)?;
    let mut shell_power = Vec::new();
    for y in 0..spec.height {
        for x in 0..spec.width {
            if x == 0 && y == 0 {
                continue;
            }
            let fx = x.min(spec.width - x) as f64;
            let fy = y.min(spec.height - y) as f64;
            let shell = (fx * fx + fy * fy).sqrt();
            if shell > 0.0 && shell.is_finite() {
                shell_power.push((shell, power[y * spec.width + x]));
            }
        }
    }
    if shell_power.is_empty() {
        return None;
    }
    shell_power.sort_by(|left, right| left.0.total_cmp(&right.0));
    let low_count = low_k_shells.max(1).min(shell_power.len());
    let low_k_power = shell_power[..low_count]
        .iter()
        .map(|(_, power)| *power)
        .sum::<f64>()
        / low_count as f64;
    let mean_power =
        shell_power.iter().map(|(_, power)| *power).sum::<f64>() / shell_power.len() as f64;
    let normalized_low_k_power = low_k_power / mean_power.max(f64::EPSILON);

    Some(BartlettPeriodogramSummary {
        raster_width: spec.width,
        raster_height: spec.height,
        n_modes: shell_power.len(),
        low_k_power,
        normalized_low_k_power,
    })
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
