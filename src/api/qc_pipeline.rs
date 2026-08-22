use std::collections::BTreeMap;

use crate::{
    common::stats::{mean_ignoring_nonfinite, safe_finite_ratio},
    config::{AnalysisConfig, PermutationStratum},
    data::{validate::validation_flags, Pattern},
    errors::{MarklabError, Result},
    geom::components::ComponentSummary,
    output::{QcSummary, StatusFlag},
    permutation::{labels::permute_fixed_count, stratified::permute_within_strata},
    qc::stain_gradient::gradient_suspect,
    spectra::structure_factor::{
        stratified_permutation_whitened_spectrum, SpectrumPermutationOptions,
    },
};

pub(super) fn validate_pattern(
    config: &AnalysisConfig,
    pattern: &Pattern,
) -> Result<(Vec<StatusFlag>, Option<Vec<u32>>)> {
    let mut status_flags = validation_flags(pattern, config);
    if pattern
        .local_dab_od
        .as_deref()
        .is_some_and(gradient_suspect)
        || pattern
            .local_hematoxylin_od
            .as_deref()
            .is_some_and(gradient_suspect)
    {
        status_flags.push(StatusFlag::StainGradientSuspect);
        status_flags.push(StatusFlag::SuppressedBiologicInterpretation);
    }
    if let Some(component_id) = pattern.component_id.as_deref() {
        let summary = ComponentSummary::from_component_ids(component_id);
        if summary.component_count >= 3 && summary.largest_fraction < 0.5 {
            status_flags.push(StatusFlag::MaskFragmentationSuspect);
        }
    }

    let configured_strata = if config.permutation.stratified {
        let strata = combined_strata_for(config, pattern)?.ok_or_else(|| {
            MarklabError::Validation(
                "stratified permutation requires at least one configured stratum".into(),
            )
        })?;
        if strata_are_mark_homogeneous(&pattern.mark, &strata) {
            status_flags.push(StatusFlag::ConfoundedBySpatialStrata);
        }
        Some(strata)
    } else {
        None
    };

    Ok((status_flags, configured_strata))
}

pub(super) fn qc_summary(pattern: &Pattern) -> QcSummary {
    QcSummary {
        valid_mask_fraction: pattern.window.valid_mask_fraction,
        internal_control_valid_fraction: pattern.internal_control_valid_fraction,
        artifact_excluded_fraction: pattern.artifact_excluded_fraction,
        nonviable_excluded_fraction: pattern.nonviable_excluded_fraction,
        mean_tumor_probability: pattern
            .tumor_probability
            .as_deref()
            .and_then(|values| mean_ignoring_nonfinite(values.iter().copied().map(f64::from))),
        mean_nucleus_area_um2: pattern
            .nucleus_area_um2
            .as_deref()
            .and_then(|values| mean_ignoring_nonfinite(values.iter().copied().map(f64::from))),
        tumor_cell_density_per_mm2: tumor_cell_density_per_mm2(pattern),
    }
}

pub(super) fn tumor_cell_density_per_mm2(pattern: &Pattern) -> Option<f64> {
    if pattern.window.area_um2.is_finite() && pattern.window.area_um2 > 0.0 {
        safe_finite_ratio(pattern.len() as f64, pattern.window.area_um2 / 1_000_000.0)
    } else {
        None
    }
}

pub(super) fn stratified_confounds(config: &AnalysisConfig, pattern: &Pattern) -> Result<bool> {
    let Some(strata) = combined_strata_for(config, pattern)? else {
        return Ok(false);
    };

    // A null that preserves a homogeneous mark count inside every stratum is
    // degenerate: it reproduces the observed labels exactly. In that case any
    // unstratified signal is, by construction, completely explained by the
    // configured strata.
    if strata_are_mark_homogeneous(&pattern.mark, &strata) {
        return Ok(true);
    }

    stratified_permutation_whitened_spectrum(
        pattern,
        &strata,
        SpectrumPermutationOptions {
            n_shells: config.spectrum.k_shells,
            low_k_modes: config.spectrum.low_k_shells,
            n_permutations: config.permutation.b,
            seed: config.permutation.seed,
            family_wise_alpha: config.inference.family_wise_alpha,
            max_scale_um: config.validation.largest_interpretable_scale_fraction
                * pattern.window.l_eff_um,
            k_shell_min: config.validation.k_shell_min,
        },
    )?
    .map(|stratified| stratified.p_global >= config.inference.family_wise_alpha)
    .ok_or_else(|| {
        MarklabError::Compute(
            "stratified spectrum could not be evaluated for the configured strata".into(),
        )
    })
}

pub(super) fn strata_are_mark_homogeneous(marks: &[u8], strata: &[u32]) -> bool {
    let mut values = BTreeMap::<u32, u8>::new();
    marks
        .iter()
        .copied()
        .zip(strata.iter().copied())
        .all(|(mark, stratum)| *values.entry(stratum).or_insert(mark) == mark)
}

pub(super) fn permutation_labels(
    config: &AnalysisConfig,
    pattern: &Pattern,
    permutation_index: usize,
    seed_salt: u64,
) -> Result<Vec<u8>> {
    let seed = config.permutation.seed ^ (permutation_index as u64).wrapping_mul(seed_salt);
    if config.permutation.stratified {
        let strata = combined_strata_for(config, pattern)?.ok_or_else(|| {
            MarklabError::Validation(
                "stratified permutation requires at least one configured stratum".into(),
            )
        })?;
        permute_within_strata(&pattern.mark, &strata, seed)
    } else {
        permute_fixed_count(pattern.len(), pattern.n_marked(), seed)
    }
}

pub(super) fn combined_strata_for(
    config: &AnalysisConfig,
    pattern: &Pattern,
) -> Result<Option<Vec<u32>>> {
    let mut columns = Vec::with_capacity(config.permutation.strata_fields.len());
    for field in &config.permutation.strata_fields {
        let column = stratum_column(field, pattern).ok_or_else(|| {
            MarklabError::Validation(format!(
                "configured permutation stratum {field:?} is absent or has the wrong length"
            ))
        })?;
        columns.push(column);
    }
    if columns.is_empty() {
        return Ok(None);
    }

    let mut ids = BTreeMap::<Vec<u64>, u32>::new();
    let mut combined = Vec::with_capacity(pattern.len());
    for index in 0..pattern.len() {
        let key = columns
            .iter()
            .map(|column| column[index])
            .collect::<Vec<_>>();
        let next_id = ids.len() as u32;
        let id = *ids.entry(key).or_insert(next_id);
        combined.push(id);
    }
    Ok(Some(combined))
}

pub(super) fn stratum_column(field: &PermutationStratum, pattern: &Pattern) -> Option<Vec<u64>> {
    match field {
        PermutationStratum::QcBin => pattern.qc_bin.as_deref().and_then(|values| {
            (values.len() == pattern.len())
                .then(|| values.iter().map(|value| u64::from(*value)).collect())
        }),
        PermutationStratum::ComponentId => pattern.component_id.as_deref().and_then(|values| {
            (values.len() == pattern.len())
                .then(|| values.iter().map(|value| u64::from(*value)).collect())
        }),
    }
}
