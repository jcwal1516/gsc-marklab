//! Ordered spectrum preprocessing steps and fixed-edge m/z binning.

use serde::{Deserialize, Serialize};

use crate::{MassSpectrum, SpectrumError};

/// One preprocessing operation. Steps run in the order they are declared.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreprocessStep {
    /// Replace each intensity by its square root.
    Sqrt,
    /// Savitzky-Golay smoothing over `2 half_window + 1` points, then negative values set to
    /// zero.
    SavitzkyGolay {
        /// Points on each side of the smoothed point.
        half_window: usize,
        /// Degree of the local least-squares polynomial.
        polynomial_order: usize,
    },
    /// Subtract a SNIP baseline estimated with clipping windows `1..=iterations`.
    SnipBaseline {
        /// Largest clipping half-window.
        iterations: usize,
        /// Apply clipping windows from largest to smallest.
        decreasing: bool,
    },
    /// Divide intensities by the trapezoidal area under the spectrum over m/z.
    TotalIonCurrent,
    /// Keep the points with `min_mz <= m/z <= max_mz`.
    Trim {
        /// Smallest kept m/z.
        min_mz: f64,
        /// Largest kept m/z.
        max_mz: f64,
    },
}

/// Equal-width m/z bins: `bins` intervals between `min_mz` and `max_mz`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BinSpec {
    /// Lower edge of the first bin.
    pub min_mz: f64,
    /// Upper edge of the last bin.
    pub max_mz: f64,
    /// Number of bins.
    pub bins: usize,
}

/// Summed intensity per bin.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BinnedSpectrum {
    /// Lower edge of the first bin.
    pub min_mz: f64,
    /// Upper edge of the last bin.
    pub max_mz: f64,
    /// Number of bins.
    pub bins: usize,
    /// Spectrum points below `min_mz` or above `max_mz`, which no bin received.
    pub points_outside: usize,
    /// Summed intensity of each bin, in m/z order.
    pub intensity: Vec<f64>,
}

const MAXIMUM_SAVITZKY_GOLAY_HALF_WINDOW: usize = 500;
const MAXIMUM_POLYNOMIAL_ORDER: usize = 10;
const MAXIMUM_SNIP_ITERATIONS: usize = 1000;
const MAXIMUM_BINS: usize = 1_000_000;

impl PreprocessStep {
    fn name(&self) -> &'static str {
        match self {
            Self::Sqrt => "sqrt",
            Self::SavitzkyGolay { .. } => "savitzky_golay",
            Self::SnipBaseline { .. } => "snip_baseline",
            Self::TotalIonCurrent => "total_ion_current",
            Self::Trim { .. } => "trim",
        }
    }
}

/// Apply `steps` to `spectrum` in order. Every intermediate spectrum is revalidated, so a step
/// that would produce a non-finite value fails instead of passing it on.
pub fn preprocess_spectrum(
    spectrum: &MassSpectrum,
    steps: &[PreprocessStep],
) -> Result<MassSpectrum, SpectrumError> {
    steps
        .iter()
        .enumerate()
        .try_fold(spectrum.clone(), |current, (index, step)| {
            apply(&current, step).map_err(|message| SpectrumError::Step {
                index,
                step: step.name(),
                message,
            })
        })
}

fn apply(spectrum: &MassSpectrum, step: &PreprocessStep) -> Result<MassSpectrum, String> {
    let (mz, values) = (&spectrum.mz, &spectrum.intensity);
    let intensity = match *step {
        PreprocessStep::Sqrt => values.iter().map(|value| value.sqrt()).collect(),
        PreprocessStep::SavitzkyGolay {
            half_window,
            polynomial_order,
        } => savitzky_golay(values, half_window, polynomial_order)?,
        PreprocessStep::SnipBaseline {
            iterations,
            decreasing,
        } => {
            if !(1..=MAXIMUM_SNIP_ITERATIONS).contains(&iterations) {
                return Err(format!(
                    "iterations must be in 1..={MAXIMUM_SNIP_ITERATIONS}"
                ));
            }
            let baseline = snip_baseline(values, iterations, decreasing);
            values.iter().zip(&baseline).map(|(y, b)| y - b).collect()
        }
        PreprocessStep::TotalIonCurrent => {
            let area: f64 = mz
                .windows(2)
                .zip(values.windows(2))
                .map(|(m, y)| (y[0] + y[1]) / 2.0 * (m[1] - m[0]))
                .sum();
            if !(area.is_finite() && area > 0.0) {
                return Err(format!(
                    "total ion current is {area}; intensities cannot be divided by it"
                ));
            }
            values.iter().map(|value| value / area).collect()
        }
        PreprocessStep::Trim { min_mz, max_mz } => {
            if !(min_mz.is_finite() && max_mz.is_finite() && min_mz < max_mz) {
                return Err(format!(
                    "min_mz {min_mz} and max_mz {max_mz} must be finite with min_mz < max_mz"
                ));
            }
            let (kept_mz, kept): (Vec<f64>, Vec<f64>) = mz
                .iter()
                .zip(values)
                .filter(|(mz, _)| (min_mz..=max_mz).contains(*mz))
                .unzip();
            if kept_mz.is_empty() {
                return Err(format!("no points lie between {min_mz} and {max_mz}"));
            }
            return MassSpectrum::new(kept_mz, kept).map_err(|error| error.to_string());
        }
    };
    MassSpectrum::new(mz.clone(), intensity).map_err(|error| error.to_string())
}

/// Smooth with local least-squares polynomials. Interior points use the centred window; the
/// first and last `half_window` points use the first or last full window evaluated at their own
/// position, as MALDIquant does. Negative smoothed values are set to zero.
fn savitzky_golay(values: &[f64], half_window: usize, order: usize) -> Result<Vec<f64>, String> {
    if !(1..=MAXIMUM_SAVITZKY_GOLAY_HALF_WINDOW).contains(&half_window) {
        return Err(format!(
            "half_window must be in 1..={MAXIMUM_SAVITZKY_GOLAY_HALF_WINDOW}"
        ));
    }
    if order > MAXIMUM_POLYNOMIAL_ORDER {
        return Err(format!(
            "polynomial_order must be at most {MAXIMUM_POLYNOMIAL_ORDER}"
        ));
    }
    let window = 2 * half_window + 1;
    let n = values.len();
    if window > n {
        return Err(format!(
            "the {window}-point window is longer than the {n}-point spectrum"
        ));
    }
    if order >= window {
        return Err(format!(
            "polynomial_order {order} must be smaller than the {window}-point window"
        ));
    }
    let weights = local_fit_weights(window, order)?;
    Ok((0..n)
        .map(|point| {
            let (start, position) = if point < half_window {
                (0, point)
            } else if point >= n - half_window {
                (n - window, point + window - n)
            } else {
                (point - half_window, half_window)
            };
            let smoothed: f64 = weights[position]
                .iter()
                .zip(&values[start..start + window])
                .map(|(weight, value)| weight * value)
                .sum();
            if smoothed < 0.0 {
                0.0
            } else {
                smoothed
            }
        })
        .collect())
}

/// Row `i` gives the value at window position `i` of the least-squares polynomial fitted to the
/// window: row `i` of the hat matrix `Q Qᵀ`, where `Q` is an orthonormal basis of polynomials of
/// degree at most `order` on the window, built by modified Gram-Schmidt with reorthogonalization.
fn local_fit_weights(window: usize, order: usize) -> Result<Vec<Vec<f64>>, String> {
    let half = (window / 2) as f64;
    let x: Vec<f64> = (0..window)
        .map(|index| (index as f64 - half) / half)
        .collect();
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(order + 1);
    for degree in 0..=order {
        let mut column: Vec<f64> = x.iter().map(|value| value.powi(degree as i32)).collect();
        let initial_norm = dot(&column, &column).sqrt();
        for _ in 0..2 {
            for q in &basis {
                let projection = dot(q, &column);
                column
                    .iter_mut()
                    .zip(q)
                    .for_each(|(value, q)| *value -= projection * q);
            }
        }
        let norm = dot(&column, &column).sqrt();
        if norm.is_nan() || norm <= 1e-8 * initial_norm {
            return Err("the local polynomial fit is singular".to_owned());
        }
        column.iter_mut().for_each(|value| *value /= norm);
        basis.push(column);
    }
    Ok((0..window)
        .map(|row| {
            (0..window)
                .map(|column| basis.iter().map(|q| q[row] * q[column]).sum())
                .collect()
        })
        .collect())
}

/// SNIP baseline (Ryan et al. 1988): for each clipping half-window `p`, every point at least `p`
/// from both ends becomes the smaller of itself and the mean of the points `p` away, all
/// computed from the previous pass. With `decreasing`, `p` runs from `iterations` down to 1
/// (Morháč 2009); otherwise from 1 up.
fn snip_baseline(values: &[f64], iterations: usize, decreasing: bool) -> Vec<f64> {
    let n = values.len();
    let mut baseline = values.to_vec();
    let mut clipped = values.to_vec();
    let mut clip = |window: usize| {
        if 2 * window >= n {
            return;
        }
        for point in window..n - window {
            let average = (baseline[point - window] + baseline[point + window]) / 2.0;
            clipped[point] = if average < baseline[point] {
                average
            } else {
                baseline[point]
            };
        }
        baseline[window..n - window].copy_from_slice(&clipped[window..n - window]);
    };
    if decreasing {
        (1..=iterations).rev().for_each(&mut clip);
    } else {
        (1..=iterations).for_each(&mut clip);
    }
    baseline
}

/// Sum intensities into the bins of `spec`. Bin `i` covers `[e_i, e_{i+1})` with
/// `e_i = min_mz + i (max_mz - min_mz) / bins` and the last bin also includes `max_mz`, the
/// boundaries of `numpy.histogram` over `numpy.linspace` edges. Each bin sums its own points
/// in m/z order.
pub fn bin_spectrum(
    spectrum: &MassSpectrum,
    spec: &BinSpec,
) -> Result<BinnedSpectrum, SpectrumError> {
    let &BinSpec {
        min_mz,
        max_mz,
        bins,
    } = spec;
    if !(min_mz.is_finite() && max_mz.is_finite() && min_mz < max_mz) {
        return Err(SpectrumError::Binning(format!(
            "min_mz {min_mz} and max_mz {max_mz} must be finite with min_mz < max_mz"
        )));
    }
    if !(1..=MAXIMUM_BINS).contains(&bins) {
        return Err(SpectrumError::Binning(format!(
            "bins must be in 1..={MAXIMUM_BINS}"
        )));
    }
    let step = (max_mz - min_mz) / bins as f64;
    let edges: Vec<f64> = (0..bins)
        .map(|index| index as f64 * step + min_mz)
        .chain([max_mz])
        .collect();
    if !step.is_finite() || edges.windows(2).any(|pair| pair[1] <= pair[0]) {
        return Err(SpectrumError::Binning(
            "the bin edges cannot be represented in floating point".to_owned(),
        ));
    }
    let mut intensity = vec![0.0; bins];
    let mut points_outside = 0;
    for (&mz, &value) in spectrum.mz.iter().zip(&spectrum.intensity) {
        if !(min_mz..=max_mz).contains(&mz) {
            points_outside += 1;
            continue;
        }
        let bin = (edges.partition_point(|edge| *edge <= mz) - 1).min(bins - 1);
        intensity[bin] += value;
    }
    if intensity.iter().any(|value| !value.is_finite()) {
        return Err(SpectrumError::Binning(
            "a bin sum overflows to infinity".to_owned(),
        ));
    }
    Ok(BinnedSpectrum {
        min_mz,
        max_mz,
        bins,
        points_outside,
        intensity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spectrum(intensity: &[f64]) -> MassSpectrum {
        let mz = (1..=intensity.len()).map(|index| index as f64).collect();
        MassSpectrum::new(mz, intensity.to_vec()).unwrap()
    }

    fn run(intensity: &[f64], step: PreprocessStep) -> Vec<f64> {
        preprocess_spectrum(&spectrum(intensity), &[step])
            .unwrap()
            .intensity()
            .to_vec()
    }

    #[test]
    fn savitzky_golay_preserves_cubics_and_clips_negative_smoothed_values() {
        let cubic: Vec<f64> = (1..=30)
            .map(|x| {
                let x = f64::from(x);
                2.0 + 0.5 * x - 0.03 * x * x + 0.001 * x * x * x
            })
            .collect();
        let smoothed = run(
            &cubic,
            PreprocessStep::SavitzkyGolay {
                half_window: 4,
                polynomial_order: 3,
            },
        );
        for (index, (smoothed, original)) in smoothed.iter().zip(&cubic).enumerate() {
            assert!((smoothed - original).abs() <= 1e-10, "point {index}");
        }

        // The classic 5-point quadratic filter is (-3, 12, 17, 12, -3) / 35.
        let mut impulse = vec![0.0; 9];
        impulse[4] = 35.0;
        let smoothed = run(
            &impulse,
            PreprocessStep::SavitzkyGolay {
                half_window: 2,
                polynomial_order: 2,
            },
        );
        let expected = [0.0, 12.0, 17.0, 12.0, 0.0];
        for (smoothed, expected) in smoothed[2..7].iter().zip(expected) {
            assert!(
                (smoothed - expected).abs() <= 1e-12,
                "{smoothed} != {expected}"
            );
        }
    }

    #[test]
    fn snip_updates_each_window_from_the_previous_pass() {
        // Updating in place would give 2.5 at the third point instead of 5.
        let corrected = run(
            &[0.0, 10.0, 10.0, 0.0, 0.0],
            PreprocessStep::SnipBaseline {
                iterations: 1,
                decreasing: true,
            },
        );
        assert_eq!(corrected, [0.0, 5.0, 5.0, 0.0, 0.0]);
    }

    #[test]
    fn total_ion_current_divides_by_trapezoidal_area() {
        let spectrum = MassSpectrum::new(vec![1.0, 2.0, 4.0], vec![1.0, 3.0, 1.0]).unwrap();
        let calibrated =
            preprocess_spectrum(&spectrum, &[PreprocessStep::TotalIonCurrent]).unwrap();
        // Area = (1 + 3) / 2 * 1 + (3 + 1) / 2 * 2 = 6.
        assert_eq!(calibrated.intensity(), &[1.0 / 6.0, 0.5, 1.0 / 6.0]);

        let error = preprocess_spectrum(
            &MassSpectrum::new(vec![1.0, 2.0], vec![0.0, 0.0]).unwrap(),
            &[PreprocessStep::TotalIonCurrent],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("total ion current"), "{error}");
    }

    #[test]
    fn trim_keeps_an_inclusive_range_and_reports_the_failing_step() {
        let trimmed = preprocess_spectrum(
            &spectrum(&[1.0, 2.0, 3.0, 4.0]),
            &[PreprocessStep::Trim {
                min_mz: 2.0,
                max_mz: 3.0,
            }],
        )
        .unwrap();
        assert_eq!(trimmed.mz(), &[2.0, 3.0]);

        let error = preprocess_spectrum(
            &spectrum(&[1.0, 2.0]),
            &[
                PreprocessStep::Sqrt,
                PreprocessStep::Trim {
                    min_mz: 10.0,
                    max_mz: 20.0,
                },
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("step 1 (trim)"), "{error}");
    }

    #[test]
    fn binning_uses_half_open_bins_with_a_closed_last_bin() {
        let spectrum = MassSpectrum::new(
            vec![0.5, 1.0, 2.9, 3.0, 5.9, 7.0, 7.5],
            vec![100.0, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0],
        )
        .unwrap();
        let binned = bin_spectrum(
            &spectrum,
            &BinSpec {
                min_mz: 1.0,
                max_mz: 7.0,
                bins: 3,
            },
        )
        .unwrap();
        // Bins [1, 3), [3, 5), [5, 7]; 0.5 and 7.5 fall outside.
        assert_eq!(binned.intensity, [3.0, 4.0, 24.0]);
        assert_eq!(binned.points_outside, 2);

        // Finite inputs whose bin width or bin sum overflows are rejected.
        let overflowing_width = BinSpec {
            min_mz: -1e308,
            max_mz: 1e308,
            bins: 1,
        };
        assert!(bin_spectrum(&spectrum, &overflowing_width).is_err());
        let huge = MassSpectrum::new(vec![1.0, 2.0], vec![1e308, 1e308]).unwrap();
        let one_bin = BinSpec {
            min_mz: 1.0,
            max_mz: 2.0,
            bins: 1,
        };
        assert!(bin_spectrum(&huge, &one_bin).is_err());
    }

    /// 600 points every 30 Da from 2,000 Da: decaying background, three peaks and a ripple.
    fn synthetic_spectrum(scale: f64) -> MassSpectrum {
        let mz: Vec<f64> = (0..600).map(|index| 2000.0 + 30.0 * index as f64).collect();
        let intensity = mz
            .iter()
            .enumerate()
            .map(|(index, &mz)| {
                let background = 500.0 * (-(mz - 2000.0) / 6000.0).exp();
                let peaks: f64 = [(4365.0, 900.0), (7158.0, 600.0), (10300.0, 300.0)]
                    .iter()
                    .map(|&(centre, height)| height * (-((mz - centre) / 40.0).powi(2)).exp())
                    .sum();
                let ripple = 20.0 * (1.0 + (index as f64 * 1.7).sin());
                scale * (background + peaks + ripple)
            })
            .collect();
        MassSpectrum::new(mz, intensity).unwrap()
    }

    fn assert_close(actual: &MassSpectrum, expected: &MassSpectrum, relative: f64) {
        assert_eq!(actual.mz(), expected.mz());
        let largest = expected.intensity().iter().fold(0.0_f64, |a, &b| a.max(b));
        for (index, (a, b)) in actual
            .intensity()
            .iter()
            .zip(expected.intensity())
            .enumerate()
        {
            assert!(
                (a - b).abs() <= relative * largest,
                "point {index}: {a} != {b}"
            );
        }
    }

    #[test]
    fn full_pipeline_ignores_the_scale_of_raw_counts() {
        // sqrt, smoothing, SNIP and clipping all scale with the counts; the area division cancels it.
        let steps = [
            PreprocessStep::Sqrt,
            PreprocessStep::SavitzkyGolay {
                half_window: 10,
                polynomial_order: 3,
            },
            PreprocessStep::SnipBaseline {
                iterations: 20,
                decreasing: true,
            },
            PreprocessStep::TotalIonCurrent,
            PreprocessStep::Trim {
                min_mz: 2000.0,
                max_mz: 15000.0,
            },
        ];
        let reference = preprocess_spectrum(&synthetic_spectrum(1.0), &steps).unwrap();
        for scale in [1e-3, 7.3, 1e4] {
            let scaled = preprocess_spectrum(&synthetic_spectrum(scale), &steps).unwrap();
            assert_close(&scaled, &reference, 1e-12);
        }
    }

    #[test]
    fn snip_cancels_any_background_linear_in_point_index() {
        let spectrum = synthetic_spectrum(1.0);
        let tilted = MassSpectrum::new(
            spectrum.mz().to_vec(),
            spectrum
                .intensity()
                .iter()
                .enumerate()
                .map(|(index, value)| value + 250.0 + 0.8 * index as f64)
                .collect(),
        )
        .unwrap();
        for decreasing in [true, false] {
            let step = [PreprocessStep::SnipBaseline {
                iterations: 20,
                decreasing,
            }];
            assert_close(
                &preprocess_spectrum(&tilted, &step).unwrap(),
                &preprocess_spectrum(&spectrum, &step).unwrap(),
                1e-12,
            );
        }
    }

    #[test]
    fn binning_conserves_intensity_between_inclusive_edges() {
        let spectrum = synthetic_spectrum(1.0);
        // Both edges fall exactly on points (indices 10 and 400).
        let binned = bin_spectrum(
            &spectrum,
            &BinSpec {
                min_mz: 2300.0,
                max_mz: 14000.0,
                bins: 3600,
            },
        )
        .unwrap();
        let inside: f64 = spectrum.intensity()[10..=400].iter().sum();
        let binned_total: f64 = binned.intensity.iter().sum();
        assert_eq!(binned.points_outside, 600 - 391);
        assert!((binned_total - inside).abs() <= 1e-12 * inside);
    }

    #[test]
    fn configuration_rejects_unknown_steps_and_fields() {
        let steps: Vec<PreprocessStep> = serde_json::from_str(
            r#"[{"step":"sqrt"},{"step":"snip_baseline","iterations":20,"decreasing":true}]"#,
        )
        .unwrap();
        assert_eq!(steps.len(), 2);
        for text in [
            r#"[{"step":"normalize"}]"#,
            r#"[{"step":"trim","min_mz":1.0,"max_mz":2.0,"unit":"Da"}]"#,
            r#"[{"step":"savitzky_golay","half_window":10}]"#,
        ] {
            assert!(
                serde_json::from_str::<Vec<PreprocessStep>>(text).is_err(),
                "{text}"
            );
        }
    }
}
