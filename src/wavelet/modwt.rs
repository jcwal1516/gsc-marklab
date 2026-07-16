#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VarianceFractions {
    pub fine: f64,
    pub intermediate: f64,
    pub coarse: f64,
}

pub fn variance_fractions_from_field(
    field: &[f32],
    width: usize,
    height: usize,
) -> Option<VarianceFractions> {
    if width == 0 || height == 0 || field.len() != width.checked_mul(height)? {
        return None;
    }

    let total = variance(
        field
            .iter()
            .map(|value| f64::from(*value))
            .collect::<Vec<_>>()
            .as_slice(),
    );
    if total <= f64::EPSILON {
        return Some(VarianceFractions {
            fine: 0.0,
            intermediate: 0.0,
            coarse: 0.0,
        });
    }

    let fine_energy = neighbor_difference_energy(field, width, height);
    let coarse_field = block_means(field, width, height, 2);
    let coarse_energy = variance(&coarse_field);
    let fine_raw = fine_energy / (fine_energy + total);
    let coarse_raw = coarse_energy / total;
    let fine = fine_raw.clamp(0.0, 1.0);
    let coarse = coarse_raw.clamp(0.0, 1.0 - fine);
    let intermediate = (1.0 - fine - coarse).max(0.0);
    let sum = fine + intermediate + coarse;

    Some(VarianceFractions {
        fine: fine / sum,
        intermediate: intermediate / sum,
        coarse: coarse / sum,
    })
}

fn variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64
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
