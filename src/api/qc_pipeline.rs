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
    spectra::structure_factor::PermutationWhitenedSpectrum,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConfoundingConclusion {
    ConfoundedBySpatialStrata,
    BothSignificant,
    NoUnstratifiedSignal,
    DegenerateStratifiedNull,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SpectrumInference {
    pub(super) p_global: f64,
    pub(super) low_k_excess_p_value: Option<f64>,
}

impl SpectrumInference {
    fn from_spectrum(spectrum: &PermutationWhitenedSpectrum) -> Self {
        Self {
            p_global: spectrum.p_global,
            low_k_excess_p_value: spectrum.low_k_excess_p_value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SpectrumNullSensitivity {
    pub(super) unstratified: Option<SpectrumInference>,
    pub(super) stratified: Option<SpectrumInference>,
    pub(super) conclusion: ConfoundingConclusion,
}

pub(super) fn spectrum_null_sensitivity(
    unstratified: Option<&PermutationWhitenedSpectrum>,
    stratified: Option<&PermutationWhitenedSpectrum>,
    alpha: f64,
    stratified_degenerate: bool,
) -> SpectrumNullSensitivity {
    let unstratified = unstratified.map(SpectrumInference::from_spectrum);
    let stratified = stratified.map(SpectrumInference::from_spectrum);
    let conclusion = classify_confounding(
        unstratified.and_then(|value| value.low_k_excess_p_value),
        stratified.and_then(|value| value.low_k_excess_p_value),
        alpha,
        stratified_degenerate,
    );
    SpectrumNullSensitivity {
        unstratified,
        stratified,
        conclusion,
    }
}

fn classify_confounding(
    unstratified_p_value: Option<f64>,
    stratified_p_value: Option<f64>,
    alpha: f64,
    stratified_degenerate: bool,
) -> ConfoundingConclusion {
    if stratified_degenerate {
        return ConfoundingConclusion::DegenerateStratifiedNull;
    }
    let (Some(unstratified), Some(stratified)) = (unstratified_p_value, stratified_p_value) else {
        return ConfoundingConclusion::NotEvaluable;
    };
    let p_value_is_valid = |value: f64| value.is_finite() && (0.0..=1.0).contains(&value);
    let alpha_is_valid = alpha.is_finite() && 0.0 < alpha && alpha < 1.0;
    if !alpha_is_valid || !p_value_is_valid(unstratified) || !p_value_is_valid(stratified) {
        return ConfoundingConclusion::NotEvaluable;
    }
    match (unstratified < alpha, stratified < alpha) {
        (true, false) => ConfoundingConclusion::ConfoundedBySpatialStrata,
        (true, true) => ConfoundingConclusion::BothSignificant,
        (false, _) => ConfoundingConclusion::NoUnstratifiedSignal,
    }
}

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

#[cfg(test)]
mod tests {
    use super::{classify_confounding, ConfoundingConclusion};

    #[test]
    fn confounding_detected_when_unstratified_disappears_after_stratification() {
        assert_eq!(
            classify_confounding(Some(0.01), Some(0.20), 0.05, false),
            ConfoundingConclusion::ConfoundedBySpatialStrata
        );
    }

    #[test]
    fn confounding_not_detected_when_both_remain_significant() {
        assert_eq!(
            classify_confounding(Some(0.01), Some(0.02), 0.05, false),
            ConfoundingConclusion::BothSignificant
        );
    }

    #[test]
    fn confounding_not_detected_when_neither_is_significant() {
        assert_eq!(
            classify_confounding(Some(0.40), Some(0.20), 0.05, false),
            ConfoundingConclusion::NoUnstratifiedSignal
        );
    }

    #[test]
    fn homogeneous_strata_report_degenerate_null() {
        assert_eq!(
            classify_confounding(Some(0.01), Some(1.0), 0.05, true),
            ConfoundingConclusion::DegenerateStratifiedNull
        );
    }
}
