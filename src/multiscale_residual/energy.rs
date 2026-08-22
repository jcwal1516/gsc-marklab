use crate::common::stats::population_variance;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelativeScaleEnergies {
    pub local_difference: f64,
    pub residual: f64,
    pub block_mean: f64,
}

pub fn relative_scale_energies_from_field(
    field: &[f32],
    width: usize,
    height: usize,
) -> Option<RelativeScaleEnergies> {
    if width == 0 || height == 0 || field.len() != width.checked_mul(height)? {
        return None;
    }

    let total = population_variance(
        field
            .iter()
            .map(|value| f64::from(*value))
            .collect::<Vec<_>>()
            .as_slice(),
    )?;
    if total <= f64::EPSILON {
        return Some(RelativeScaleEnergies {
            local_difference: 0.0,
            residual: 0.0,
            block_mean: 0.0,
        });
    }

    let local_difference_energy = neighbor_difference_energy(field, width, height);
    let block_mean_field = block_means(field, width, height, 2);
    let block_mean_variance = population_variance(&block_mean_field)?;
    let local_difference_raw = local_difference_energy / (local_difference_energy + total);
    let block_mean_raw = block_mean_variance / total;
    let local_difference = local_difference_raw.clamp(0.0, 1.0);
    let block_mean = block_mean_raw.clamp(0.0, 1.0 - local_difference);
    let residual = (1.0 - local_difference - block_mean).max(0.0);
    let sum = local_difference + residual + block_mean;

    Some(RelativeScaleEnergies {
        local_difference: local_difference / sum,
        residual: residual / sum,
        block_mean: block_mean / sum,
    })
}

fn neighbor_difference_energy(field: &[f32], width: usize, height: usize) -> f64 {
    let mut total = 0.0;
    let mut count = 0;
    for y in 0..height {
        for x in 0..width {
            let value = f64::from(field[y * width + x]);
            if x + 1 < width {
                let delta = value - f64::from(field[y * width + x + 1]);
                total += delta * delta;
                count += 1;
            }
            if y + 1 < height {
                let delta = value - f64::from(field[(y + 1) * width + x]);
                total += delta * delta;
                count += 1;
            }
        }
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

fn block_means(field: &[f32], width: usize, height: usize, block: usize) -> Vec<f64> {
    let mut means = Vec::new();
    for y0 in (0..height).step_by(block) {
        for x0 in (0..width).step_by(block) {
            let mut sum = 0.0;
            let mut count = 0;
            for y in y0..(y0 + block).min(height) {
                for x in x0..(x0 + block).min(width) {
                    sum += f64::from(field[y * width + x]);
                    count += 1;
                }
            }
            if count > 0 {
                means.push(sum / count as f64);
            }
        }
    }
    means
}
