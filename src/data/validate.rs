use std::collections::BTreeSet;

use crate::{
    config::AnalysisConfig,
    data::Pattern,
    geom::length_scales::analysis_effective_length_um,
    output::StatusFlag,
    spectra::kgrid::{resolvable_k_modes, KBand},
};

pub fn validation_flags(pattern: &Pattern, config: &AnalysisConfig) -> Vec<StatusFlag> {
    let n_marked = pattern.mark.iter().filter(|mark| **mark == 1).count();
    let n_unmarked = pattern.len().saturating_sub(n_marked);
    let prevalence = if pattern.is_empty() {
        0.0
    } else {
        n_marked as f64 / pattern.len() as f64
    };
    validation_flags_with_counts(pattern, config, n_marked, n_unmarked, prevalence)
}

pub(crate) fn validation_flags_with_counts(
    pattern: &Pattern,
    config: &AnalysisConfig,
    n_marked: usize,
    n_unmarked: usize,
    prevalence: f64,
) -> Vec<StatusFlag> {
    let mut flags = Vec::new();

    if pattern.len() < config.validation.n_min {
        flags.push(StatusFlag::UnderpoweredTooFewCells);
    }
    if n_marked < config.validation.n_marked_min {
        flags.push(StatusFlag::UnderpoweredTooFewMarked);
    }
    if n_unmarked < config.validation.n_unmarked_min {
        flags.push(StatusFlag::UnderpoweredTooFewUnmarked);
    }
    if !pattern.is_empty()
        && (prevalence < config.validation.p_min || prevalence > config.validation.p_max)
    {
        flags.push(StatusFlag::SensitivityUnstable);
    }
    if pattern.window.area_um2.is_finite()
        && pattern.window.area_um2 > 0.0
        && pattern.window.area_um2 < config.validation.area_min_um2
    {
        flags.push(StatusFlag::UnderpoweredAreaTooSmall);
    }
    if let Some(band) = analysis_effective_length_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
    )
    .and_then(|length| KBand::from_window(length, pattern.window.d_nn_mean_um))
    {
        let shell_count = resolvable_k_modes(band, config.spectrum.k_shells)
            .into_iter()
            .map(|mode| mode.shell_index)
            .collect::<BTreeSet<_>>()
            .len();
        if shell_count < config.validation.k_shell_min {
            flags.push(StatusFlag::UnderpoweredTooFewKShells);
        }
    }
    if pattern.window.valid_mask_fraction < config.validation.valid_mask_fraction_min {
        flags.push(StatusFlag::InvalidIhcMask);
    }
    if pattern
        .internal_control_valid_fraction
        .map(|fraction| fraction < config.validation.valid_mask_fraction_min)
        .unwrap_or(false)
    {
        flags.push(StatusFlag::InternalControlFailureOverlap);
        flags.push(StatusFlag::SuppressedBiologicInterpretation);
    }

    flags
}
