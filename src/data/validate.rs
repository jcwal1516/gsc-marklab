use std::collections::BTreeSet;

use crate::{
    config::AnalysisConfig,
    data::Pattern,
    output::StatusFlag,
    spectra::kgrid::{resolvable_k_modes, KBand},
};

pub fn validation_flags(pattern: &Pattern, config: &AnalysisConfig) -> Vec<StatusFlag> {
    let mut flags = Vec::new();

    if pattern.len() < config.validation.n_min {
        flags.push(StatusFlag::UnderpoweredTooFewCells);
    }
    if pattern.n_marked() < config.validation.n_marked_min {
        flags.push(StatusFlag::UnderpoweredTooFewMarked);
    }
    if pattern.n_unmarked() < config.validation.n_unmarked_min {
        flags.push(StatusFlag::UnderpoweredTooFewUnmarked);
    }
    let p_hat = pattern.p_hat();
    if !pattern.is_empty() && (p_hat < config.validation.p_min || p_hat > config.validation.p_max) {
        flags.push(StatusFlag::SensitivityUnstable);
    }
    if pattern.window.area_um2.is_finite()
        && pattern.window.area_um2 > 0.0
        && pattern.window.area_um2 < config.validation.area_min_um2
    {
        flags.push(StatusFlag::UnderpoweredAreaTooSmall);
    }
    if let Some(band) = KBand::from_window(pattern.window.l_eff_um, pattern.window.d_nn_mean_um) {
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
