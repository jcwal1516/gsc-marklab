#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KBand {
    pub k_min: f64,
    pub k_max: f64,
}

impl KBand {
    pub fn from_window(analysis_effective_length_um: f64, d_nn_mean_um: f64) -> Option<Self> {
        if analysis_effective_length_um <= 0.0
            || d_nn_mean_um <= 0.0
            || !analysis_effective_length_um.is_finite()
            || !d_nn_mean_um.is_finite()
        {
            return None;
        }

        Some(Self {
            k_min: 2.0 * std::f64::consts::PI / analysis_effective_length_um,
            k_max: 2.0 * std::f64::consts::PI / d_nn_mean_um,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KMode {
    pub kx: f64,
    pub ky: f64,
    pub k: f64,
    pub shell_index: usize,
}

pub fn resolvable_k_modes(band: KBand, n_shells: usize) -> Vec<KMode> {
    if n_shells == 0
        || band.k_min <= 0.0
        || band.k_max <= 0.0
        || band.k_min > band.k_max
        || !band.k_min.is_finite()
        || !band.k_max.is_finite()
    {
        return Vec::new();
    }

    let k_step = band.k_min;
    let max_index = (band.k_max / k_step).floor() as isize;
    if max_index < 1 {
        return Vec::new();
    }

    let shell_width = ((band.k_max - band.k_min) / n_shells as f64).max(f64::EPSILON);
    let mut modes = Vec::new();
    for mx in -max_index..=max_index {
        for my in -max_index..=max_index {
            if mx == 0 && my == 0 {
                continue;
            }
            let kx = mx as f64 * k_step;
            let ky = my as f64 * k_step;
            let k = (kx * kx + ky * ky).sqrt();
            if k + f64::EPSILON < band.k_min || k - f64::EPSILON > band.k_max {
                continue;
            }

            let shell_index = (((k - band.k_min) / shell_width).floor() as usize).min(n_shells - 1);
            modes.push(KMode {
                kx,
                ky,
                k,
                shell_index,
            });
        }
    }

    modes.sort_by(|left, right| {
        left.k
            .total_cmp(&right.k)
            .then_with(|| left.kx.total_cmp(&right.kx))
            .then_with(|| left.ky.total_cmp(&right.ky))
    });
    modes
}
