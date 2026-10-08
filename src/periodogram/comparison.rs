//! Descriptive sensitivity to raster assignment, using matched tapered binary marks.
//! This is not a calibrated statistical test of agreement with the label-null spectrum.

use std::{collections::BTreeMap, f64::consts::TAU};

use super::{
    fft2::fft2_power_spectrum,
    raster::RasterAssignmentPlan,
    tapered::{apply_separable_hann_taper, hann_weight},
};
use crate::{data::Pattern, output::AnalysisSection};

// A diagnostic tolerance, not a significance level or a biological effect threshold.
const RELATIVE_POWER_TOLERANCE: f64 = 0.25;
const MAX_DIRECT_MODE_CELL_PAIRS: usize = 50_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridResolution {
    Adequate,
    Inadequate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MatchedBandPower {
    pub(crate) raster_low_k: f64,
    pub(crate) point_low_k: f64,
    pub(crate) min_frequency_per_um: f64,
    pub(crate) max_frequency_per_um: f64,
}

impl MatchedBandPower {
    pub(crate) fn relative_difference(self) -> f64 {
        let scale = self.raster_low_k.max(self.point_low_k);
        if scale == 0.0 {
            0.0
        } else {
            (self.raster_low_k - self.point_low_k).abs() / scale
        }
    }

    pub(crate) fn disagrees(self) -> bool {
        self.relative_difference() > RELATIVE_POWER_TOLERANCE
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PeriodogramDiagnostic {
    pub(crate) resolution: GridResolution,
    pub(crate) comparison: AnalysisSection<MatchedBandPower>,
}

impl PeriodogramDiagnostic {
    pub(crate) fn artifact_suspect(&self) -> bool {
        // An unavailable 2-D FFT (for example a one-row ROI) does not invalidate
        // the independently available point spectrum. Its resolution is still reported.
        self.comparison
            .value()
            .is_some_and(|value| self.resolution == GridResolution::Inadequate || value.disagrees())
    }

    pub(crate) fn description(&self) -> String {
        let grid = match self.resolution {
            GridResolution::Adequate => "adequate",
            GridResolution::Inadequate => "inadequate",
        };
        match &self.comparison {
            AnalysisSection::Available { value } => format!(
                " Raster resolution: {grid}. Matched tapered binary-mark gridding sensitivity: {:.1}% relative low-band difference over {:.6}–{:.6} cycles/um (25% descriptive tolerance; not a calibrated agreement test or a comparison of permutation p-values).",
                100.0 * value.relative_difference(), value.min_frequency_per_um, value.max_frequency_per_um),
            AnalysisSection::InsufficientData { reason } => format!(" Raster resolution: {grid}. Fourier comparison unavailable: {reason}; absence of a discrepancy flag does not establish agreement."),
            _ => String::new(),
        }
    }
}

pub(crate) fn matched_periodogram_diagnostic(
    pattern: &Pattern,
    cell_size_um: f64,
    low_k_shells: usize,
    max_scale_um: f64,
    memory_budget_bytes: usize,
) -> PeriodogramDiagnostic {
    let mut diagnostic = PeriodogramDiagnostic {
        resolution: GridResolution::Inadequate,
        comparison: AnalysisSection::InsufficientData {
            reason: "raster assignment is unavailable".into(),
        },
    };
    let Some(plan) = RasterAssignmentPlan::new(pattern, cell_size_um) else {
        return diagnostic;
    };
    let spec = plan.spec();
    diagnostic.resolution =
        if spec.width < 4 || spec.height < 4 || 2.0 * cell_size_um >= max_scale_um {
            GridResolution::Inadequate
        } else {
            GridResolution::Adequate
        };
    diagnostic.comparison = match compare(
        pattern,
        &plan,
        low_k_shells,
        max_scale_um,
        memory_budget_bytes,
    ) {
        Ok(value) => AnalysisSection::available(value),
        Err(reason) => AnalysisSection::InsufficientData {
            reason: reason.into(),
        },
    };
    diagnostic
}

fn compare(
    pattern: &Pattern,
    plan: &RasterAssignmentPlan,
    low_k_shells: usize,
    max_scale_um: f64,
    memory_budget_bytes: usize,
) -> Result<MatchedBandPower, &'static str> {
    let spec = plan.spec();
    if spec.width < 3 || spec.height < 3 {
        return Err("Hann taper needs at least three pixels along both axes");
    }
    if !max_scale_um.is_finite() || max_scale_um <= 0.0 {
        return Err("no eligible physical frequency range");
    }
    // Covers the raster, FFT buffers/scratch, power, and direct weighted-cell coordinates.
    let bytes = plan
        .pixel_count()
        .saturating_mul(96)
        .saturating_add(pattern.len().saturating_mul(40));
    if bytes > memory_budget_bytes {
        return Err("matched Fourier comparison exceeds its memory budget");
    }
    let mut raster = Vec::new();
    plan.fill_centered_binary_marks(&pattern.mark, &mut raster)
        .ok_or("centered binary raster is unavailable")?;
    apply_separable_hann_taper(&mut raster, spec.width, spec.height)
        .ok_or("no measurable tapered signal")?;
    let power =
        fft2_power_spectrum(&raster, spec.width, spec.height).ok_or("FFT is unavailable")?;
    let min_frequency = 1.0 / max_scale_um;
    if !min_frequency.is_finite() {
        return Err("no eligible physical frequency range");
    }
    let requested_shells = low_k_shells.max(1).saturating_add(1);
    let mut shells = BTreeMap::<usize, Vec<(usize, f64, f64)>>::new();
    for y in 0..spec.height {
        let fy = signed_frequency(y, spec.height, spec.cell_size_um);
        for x in 0..spec.width {
            let fx = signed_frequency(x, spec.width, spec.cell_size_um);
            let frequency = fx.hypot(fy);
            let roundoff = 4.0 * f64::EPSILON * frequency.max(min_frequency);
            if frequency + roundoff < min_frequency || frequency == 0.0 {
                continue;
            }
            // Compute annuli in dimensionless grid units; physical-unit
            // conversion must not move a mode across an integer shell boundary.
            let longest = spec.width.max(spec.height) as f64;
            let radius = (x.min(spec.width - x) as f64 * longest / spec.width as f64)
                .hypot(y.min(spec.height - y) as f64 * longest / spec.height as f64);
            let nearest = radius.round();
            let radius = if (radius - nearest).abs() <= 4.0 * f64::EPSILON * radius {
                nearest
            } else {
                radius
            };
            let shell = radius.floor().max(1.0) as usize;
            if shells.len() == requested_shells
                && shells
                    .last_key_value()
                    .is_some_and(|(last, _)| shell > *last)
            {
                continue;
            }
            shells
                .entry(shell)
                .or_default()
                .push((y * spec.width + x, fx, fy));
            if shells.len() > requested_shells {
                shells.pop_last();
            }
        }
    }
    if shells.len() <= low_k_shells.max(1) {
        return Err("eligible low-frequency shells lack a separate reference shell");
    }
    let mode_count = shells.values().map(Vec::len).sum::<usize>();
    if mode_count.saturating_mul(pattern.len()) > MAX_DIRECT_MODE_CELL_PAIRS {
        return Err("matched direct Fourier comparison exceeds its work budget");
    }
    let min_x = pattern
        .x_um
        .iter()
        .copied()
        .reduce(f64::min)
        .ok_or("empty pattern")?;
    let min_y = pattern
        .y_um
        .iter()
        .copied()
        .reduce(f64::min)
        .ok_or("empty pattern")?;
    let prevalence = pattern.n_marked() as f64 / pattern.len() as f64;
    // Identical cell weights in both estimators. Only original vs assigned coordinates differ.
    let weighted = pattern
        .x_um
        .iter()
        .zip(pattern.y_um.iter())
        .zip(pattern.mark.iter())
        .zip(plan.cell_bins())
        .map(|(((&x, &y), &mark), &bin)| {
            let x = x - min_x;
            let y = y - min_y;
            let taper = hann_weight(bin % spec.width, spec.width)
                * hann_weight(bin / spec.width, spec.height);
            (x, y, (f64::from(mark) - prevalence) * taper)
        })
        .collect::<Vec<_>>();
    let mut raster_means = Vec::with_capacity(shells.len());
    let mut point_means = Vec::with_capacity(shells.len());
    let mut min_selected = f64::INFINITY;
    let mut max_selected = 0.0_f64;
    for modes in shells.into_values() {
        let mut raster_sum = 0.0;
        let mut point_sum = 0.0;
        for &(index, fx, fy) in &modes {
            let frequency = fx.hypot(fy);
            min_selected = min_selected.min(frequency);
            max_selected = max_selected.max(frequency);
            raster_sum += power[index];
            let mut re = 0.0;
            let mut im = 0.0;
            for &(x, y, weight) in &weighted {
                let (sin, cos) = (TAU * (fx * x + fy * y)).sin_cos();
                re += weight * cos;
                im += weight * sin;
            }
            point_sum += re * re + im * im;
        }
        raster_means.push(raster_sum / modes.len() as f64);
        point_means.push(point_sum / modes.len() as f64);
    }
    // f32 FFT cancellation cannot resolve power beneath this conservative roundoff floor.
    // Use the same floor for both estimators so exact spectral zeros cannot become a mismatch.
    let amplitude_error = 8.0
        * f64::from(f32::EPSILON)
        * (1.0 + (spec.width as f64).log2() + (spec.height as f64).log2())
        * weighted
            .iter()
            .map(|(_, _, weight)| weight.abs())
            .sum::<f64>();
    let power_floor = amplitude_error * amplitude_error;
    let normalized = |means: &[f64]| {
        let total = means.iter().sum::<f64>();
        if !total.is_finite() || total <= power_floor * means.len() as f64 {
            return None;
        }
        let low_power = means[..low_k_shells.max(1)].iter().sum::<f64>();
        let value = if low_power <= power_floor * low_k_shells.max(1) as f64 {
            0.0
        } else {
            low_power / total
        };
        (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(value)
    };
    Ok(MatchedBandPower {
        raster_low_k: normalized(&raster_means)
            .ok_or("no measurable raster power in eligible frequency bands")?,
        point_low_k: normalized(&point_means)
            .ok_or("no measurable point power in eligible frequency bands")?,
        min_frequency_per_um: min_selected,
        max_frequency_per_um: max_selected,
    })
}

fn signed_frequency(index: usize, length: usize, spacing: f64) -> f64 {
    let mode = if index <= length / 2 {
        index as f64
    } else {
        index as f64 - length as f64
    };
    mode / (length as f64 * spacing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::PatternMeta;

    #[test]
    fn fft_summary_changes_disagreement_independently_of_grid_resolution() {
        for resolution in [GridResolution::Adequate, GridResolution::Inadequate] {
            let mut diagnostic = PeriodogramDiagnostic {
                resolution,
                comparison: AnalysisSection::available(MatchedBandPower {
                    point_low_k: 0.8,
                    raster_low_k: 0.8,
                    min_frequency_per_um: 0.1,
                    max_frequency_per_um: 0.2,
                }),
            };
            assert!(!diagnostic.comparison.value().unwrap().disagrees());
            if let AnalysisSection::Available { value } = &mut diagnostic.comparison {
                value.raster_low_k = 0.1;
            }
            assert!(diagnostic.comparison.value().unwrap().disagrees());
            assert!(diagnostic.artifact_suspect());
            diagnostic.comparison = AnalysisSection::InsufficientData {
                reason: "zero tapered energy".into(),
            };
            assert!(diagnostic.description().contains("unavailable"));
            assert!(!diagnostic.artifact_suspect());
            assert_eq!(diagnostic.resolution, resolution);
        }
    }

    fn pattern(jitter: bool, scale: f64, translate: f64, complement: bool) -> Pattern {
        Pattern::from_arrays(
            (0..96)
                .map(|i| {
                    scale
                        * ((i % 12) as f64
                            + if jitter && i % 12 != 0 {
                                0.2 * (i % 3) as f64
                            } else {
                                0.0
                            })
                        + translate
                })
                .collect(),
            (0..96)
                .map(|i| scale * (i / 12) as f64 + translate)
                .collect(),
            (0..96)
                .map(|i| u8::from((i % 12 < 4) ^ complement))
                .collect(),
            PatternMeta {
                case_id: "fft-audit".into(),
                timepoint: "post".into(),
                protein: "mark".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn matched_rectangular_fft_agrees_on_grid_and_preserves_units_and_marks() {
        let reference =
            matched_periodogram_diagnostic(&pattern(false, 1., 0., false), 1., 2, 8., usize::MAX);
        let value = reference.comparison.value().expect("matched bands");
        assert!(value.relative_difference() < 1e-6, "{reference:?}");
        assert!(value.min_frequency_per_um >= 1. / 8.);
        for (scale, translate) in [(0.1, 0.), (2.8, 0.), (0.15, 100.1)] {
            let actual = matched_periodogram_diagnostic(
                &pattern(false, scale, translate, false),
                scale,
                2,
                8. * scale,
                usize::MAX,
            );
            let actual = actual.comparison.value().expect("decimal-grid bands");
            assert!(
                actual.relative_difference() < 1e-6,
                "{scale}, {translate}: {actual:?}"
            );
            assert!((actual.raster_low_k - value.raster_low_k).abs() < 1e-6);
            assert!((actual.point_low_k - value.point_low_k).abs() < 1e-6);
        }
        let jittered =
            matched_periodogram_diagnostic(&pattern(true, 1., 0., false), 1., 2, 8., usize::MAX);
        for (scale, translate, complement) in [(1., 100., false), (10., 0., false), (1., 0., true)]
        {
            let actual = matched_periodogram_diagnostic(
                &pattern(true, scale, translate, complement),
                scale,
                2,
                8. * scale,
                usize::MAX,
            );
            let expected = jittered.comparison.value().unwrap();
            let actual = actual.comparison.value().unwrap();
            assert!((actual.point_low_k - expected.point_low_k).abs() < 1e-12);
            assert!((actual.raster_low_k - expected.raster_low_k).abs() < 1e-12);
        }
        let limited = matched_periodogram_diagnostic(&pattern(false, 1., 0., false), 1., 2, 8., 1);
        assert!(limited.description().contains("memory budget"));
    }

    #[test]
    fn matched_comparison_uses_first_nonempty_eligible_shells_and_a_reference() {
        // The cutoff lies above every mode in shell 2. Shells 3, 4 and 5
        // still provide two low-frequency shells and a reference shell.
        let input = pattern(false, 1., 0., false);
        let diagnostic = matched_periodogram_diagnostic(&input, 1., 2, 4.2, usize::MAX);
        let value = diagnostic
            .comparison
            .value()
            .expect("nonempty reference shell");
        assert!(value.relative_difference() < 1e-6);
        assert!((value.min_frequency_per_um - 0.25).abs() < 1e-12);
        assert!(value.max_frequency_per_um >= 5. / 12.);
        assert!(value.max_frequency_per_um < 0.5);

        let unavailable = matched_periodogram_diagnostic(&input, 1., 20, 4.2, usize::MAX);
        assert!(unavailable.description().contains("reference shell"));
        let unavailable =
            matched_periodogram_diagnostic(&input, 1., 2, f64::from_bits(1), usize::MAX);
        assert!(unavailable
            .description()
            .contains("no eligible physical frequency range"));
    }
}
