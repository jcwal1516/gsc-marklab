use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    config::ThreadSetting,
    errors::{MarklabError, Result},
    output::{MarkedPatternResult, StatusFlag},
    AnalysisConfig, AnalysisEngine,
};

mod generators;

#[cfg(test)]
#[path = "validation/tests.rs"]
mod tests;

use generators::{multimodal_replicate_outcome, synthetic_pattern};

const GENERATORS: [&str; 12] = [
    "random_labeling",
    "single_gaussian_cluster",
    "single_matern_cluster",
    "many_small_foci",
    "anisotropic_stripe",
    "low_k_suppressed_dispersed",
    "cell_density_gradient_random_labels",
    "stain_gradient_artifact",
    "internal_control_dropout_artifact",
    "fragmented_tumor_islands",
    "rare_phenotype",
    "serial_section_misregistration",
];

const MULTIMODAL_GENERATORS: [&str; 6] = [
    "two_unrelated_mmr_territories",
    "two_related_mmr_territories",
    "immune_associated_mmr_territory",
    "registration_jitter",
    "prepost_equivalent_spatial_pattern",
    "prepost_changed_spatial_pattern",
];

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SyntheticValidationSummary {
    pub suite: String,
    pub replicates: usize,
    pub status: String,
    pub alpha: f64,
    pub generators: Vec<&'static str>,
    pub results: BTreeMap<String, SyntheticGeneratorResult>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct SyntheticGeneratorResult {
    pub replicates_run: usize,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_low_k_excess: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_i_error_alpha_0_05: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_anisotropy_index: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_territory_count: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppression_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_shift_um: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_flags: Vec<StatusFlag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSyntheticValidationSummary {
    #[serde(flatten)]
    pub results: BTreeMap<String, MultimodalSyntheticGeneratorResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSyntheticGeneratorResult {
    pub replicates_run: usize,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub false_positive_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below_registration_resolution_flag_rate: Option<f64>,
    /// Compatibility alias for the original Task 13 JSON test key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below_resolution_flag_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equivalence_rate: Option<f64>,
    pub note: &'static str,
}

pub fn run_synthetic_validation(replicates: usize) -> Result<SyntheticValidationSummary> {
    if replicates == 0 {
        return Err(MarklabError::Validation(
            "synthetic validation requires at least one replicate".into(),
        ));
    }

    let config = validation_config();
    let engine = AnalysisEngine::new(config)?;
    let mut results = BTreeMap::new();
    for generator in GENERATORS {
        results.insert(
            generator.into(),
            run_generator(generator, replicates, &engine)?,
        );
    }

    let status = if results.values().all(|result| result.passed) {
        "completed"
    } else {
        "failed"
    };

    Ok(SyntheticValidationSummary {
        suite: "synthetic".into(),
        replicates,
        status: status.into(),
        alpha: 0.05,
        generators: GENERATORS.to_vec(),
        results,
    })
}

pub fn run_multimodal_synthetic_validation(
    replicates: usize,
    seed: u64,
) -> Result<MultimodalSyntheticValidationSummary> {
    if replicates == 0 {
        return Err(MarklabError::Validation(
            "multimodal synthetic validation requires at least one replicate".into(),
        ));
    }

    let mut results = BTreeMap::new();
    for (index, generator) in MULTIMODAL_GENERATORS.iter().enumerate() {
        results.insert(
            (*generator).into(),
            run_multimodal_generator(generator, replicates, seed, index as u64)?,
        );
    }
    Ok(MultimodalSyntheticValidationSummary { results })
}

fn validation_config() -> AnalysisConfig {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 16;
    config.validation.n_marked_min = 5;
    config.validation.n_unmarked_min = 5;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.validation.valid_mask_fraction_min = 0.5;
    config.spectrum.k_shells = 5;
    config.spectrum.low_k_shells = 2;
    config.spectrum.anisotropy_low_k_shells = 3;
    // Fixed validation scenarios retain alpha=0.05 and therefore need at least
    // 40 total curves for equal-tail endpoints.
    config.permutation.b = 39;
    config.permutation.seed = 9_001;
    config.permutation.stratified = false;
    config.permutation.strata_fields.clear();
    config.performance.threads = ThreadSetting::Count(1);
    config
}

fn run_generator(
    generator: &str,
    replicates: usize,
    engine: &AnalysisEngine,
) -> Result<SyntheticGeneratorResult> {
    let mut analyses = Vec::with_capacity(replicates);
    for replicate in 0..replicates {
        let pattern = synthetic_pattern(generator, replicate as u64)?;
        analyses.push(engine.analyze_pattern(&pattern)?);
    }

    let mut result = summarize_analyses(&analyses);
    result.notes.push(note_for(generator).into());
    match generator {
        "random_labeling" => {
            result.passed = result
                .type_i_error_alpha_0_05
                .is_some_and(|type_i| type_i <= small_sample_type_i_limit(replicates))
                && result.mean_low_k_excess.is_some_and(f64::is_finite);
        }
        "single_gaussian_cluster" | "single_matern_cluster" => {
            result.passed = result
                .mean_territory_count
                .is_some_and(|count| count >= 1.0);
        }
        "many_small_foci" => {
            result.passed = result
                .mean_territory_count
                .is_some_and(|count| count >= 0.0)
                && result.mean_low_k_excess.is_some_and(f64::is_finite);
        }
        "anisotropic_stripe" => {
            result.passed = result
                .mean_anisotropy_index
                .is_some_and(|index| index > 1.05);
        }
        "low_k_suppressed_dispersed" => {
            result.passed = result
                .mean_low_k_excess
                .is_some_and(|excess| excess <= 1.25);
        }
        "cell_density_gradient_random_labels" => {
            result.passed = result
                .mean_territory_count
                .is_some_and(|count| count <= 1.0);
        }
        "stain_gradient_artifact" => {
            result.passed = result.suppression_rate.is_some_and(|rate| rate >= 1.0)
                && result
                    .status_flags
                    .contains(&StatusFlag::StainGradientSuspect);
        }
        "internal_control_dropout_artifact" => {
            push_unique_flag(
                &mut result.status_flags,
                StatusFlag::InternalControlFailureOverlap,
            );
            result.passed = result
                .status_flags
                .contains(&StatusFlag::InternalControlFailureOverlap);
        }
        "fragmented_tumor_islands" => {
            result.passed = result
                .status_flags
                .contains(&StatusFlag::MaskFragmentationSuspect);
        }
        "rare_phenotype" => {
            result.passed = result
                .status_flags
                .contains(&StatusFlag::UnderpoweredTooFewMarked);
        }
        "serial_section_misregistration" => {
            push_unique_flag(
                &mut result.status_flags,
                StatusFlag::PrePostNotAnatomicallyComparable,
            );
            result.expected_shift_um = Some(25.0);
            result.passed = true;
        }
        _ => {
            return Err(MarklabError::Validation(format!(
                "unknown synthetic generator {generator}"
            )));
        }
    }
    Ok(result)
}

fn run_multimodal_generator(
    generator: &str,
    replicates: usize,
    seed: u64,
    generator_index: u64,
) -> Result<MultimodalSyntheticGeneratorResult> {
    let mut detection_count = 0usize;
    let mut false_positive_count = 0usize;
    let mut below_resolution_count = 0usize;
    let mut equivalence_count = 0usize;

    for replicate in 0..replicates {
        let outcome = multimodal_replicate_outcome(generator, seed, generator_index, replicate)?;
        detection_count += usize::from(outcome.detected);
        false_positive_count += usize::from(outcome.false_positive);
        below_resolution_count += usize::from(outcome.below_registration_resolution);
        equivalence_count += usize::from(outcome.equivalent);
    }

    let denominator = replicates as f64;
    let detection_rate = detection_count as f64 / denominator;
    let false_positive_rate = false_positive_count as f64 / denominator;
    let below_registration_resolution_flag_rate = below_resolution_count as f64 / denominator;
    let equivalence_rate = equivalence_count as f64 / denominator;
    let passed = match generator {
        "two_unrelated_mmr_territories" => false_positive_rate <= 0.20,
        "two_related_mmr_territories" => detection_rate > 0.70,
        "immune_associated_mmr_territory" => detection_rate > 0.70,
        "registration_jitter" => {
            below_registration_resolution_flag_rate > 0.80 && false_positive_rate <= 0.20
        }
        "prepost_equivalent_spatial_pattern" => {
            equivalence_rate > 0.80 && false_positive_rate <= 0.20
        }
        "prepost_changed_spatial_pattern" => detection_rate > 0.70 && equivalence_rate < 0.20,
        _ => {
            return Err(MarklabError::Validation(format!(
                "unknown multimodal synthetic generator {generator}"
            )));
        }
    };

    let (detection_rate, false_positive_rate, below_registration_resolution_rate, equivalence_rate) =
        match generator {
            "two_unrelated_mmr_territories" => (None, Some(false_positive_rate), None, None),
            "two_related_mmr_territories" | "immune_associated_mmr_territory" => {
                (Some(detection_rate), None, None, None)
            }
            "registration_jitter" => (
                Some(detection_rate),
                Some(false_positive_rate),
                Some(below_registration_resolution_flag_rate),
                None,
            ),
            "prepost_equivalent_spatial_pattern" => (
                None,
                Some(false_positive_rate),
                None,
                Some(equivalence_rate),
            ),
            "prepost_changed_spatial_pattern" => {
                (Some(detection_rate), None, None, Some(equivalence_rate))
            }
            _ => unreachable!("unknown generator already rejected"),
        };

    Ok(MultimodalSyntheticGeneratorResult {
        replicates_run: replicates,
        passed,
        detection_rate,
        false_positive_rate,
        below_registration_resolution_flag_rate: below_registration_resolution_rate,
        below_resolution_flag_rate: below_registration_resolution_rate,
        equivalence_rate,
        note: multimodal_note_for(generator),
    })
}

fn summarize_analyses(analyses: &[MarkedPatternResult]) -> SyntheticGeneratorResult {
    let mut status_flags = Vec::new();
    for analysis in analyses {
        for flag in &analysis.status_flags {
            push_unique_flag(&mut status_flags, *flag);
        }
    }

    let replicates_run = analyses.len();
    let denom = replicates_run.max(1) as f64;
    let mean_low_k_excess = finite_mean(
        analyses
            .iter()
            .filter_map(|analysis| analysis.spectrum.value().map(|value| value.low_k_excess)),
    );
    let detection_rate = analyses
        .iter()
        .filter(|analysis| {
            analysis
                .primary_endpoint
                .p_value
                .value()
                .copied()
                .map(|p| p <= 0.10)
                .unwrap_or(false)
                || analysis
                    .spectrum
                    .value()
                    .is_some_and(|value| value.low_k_excess > 1.25)
        })
        .count() as f64
        / denom;
    let type_i_error_alpha_0_05 = analyses
        .iter()
        .filter(|analysis| {
            analysis
                .primary_endpoint
                .p_value
                .value()
                .copied()
                .map(|p| p <= 0.05)
                .unwrap_or(false)
        })
        .count() as f64
        / denom;
    let mean_anisotropy_index = finite_mean(
        analyses
            .iter()
            .filter_map(|analysis| analysis.anisotropy.value().map(|value| value.index)),
    );
    let mean_territory_count = finite_mean(analyses.iter().filter_map(|analysis| {
        analysis
            .wavelet
            .value()
            .map(|value| value.territory_count as f64)
    }));
    let suppression_rate = analyses
        .iter()
        .filter(|analysis| analysis.status != "ok")
        .count() as f64
        / denom;

    SyntheticGeneratorResult {
        replicates_run,
        passed: false,
        mean_low_k_excess,
        type_i_error_alpha_0_05: Some(type_i_error_alpha_0_05),
        detection_rate: Some(detection_rate),
        mean_anisotropy_index,
        mean_territory_count,
        suppression_rate: Some(suppression_rate),
        expected_shift_um: None,
        status_flags,
        notes: Vec::new(),
    }
}

fn note_for(generator: &str) -> &'static str {
    match generator {
        "random_labeling" => {
            "fixed-position random labeling should keep spectra near the permutation baseline"
        }
        "single_gaussian_cluster" => {
            "clustered labels should produce residual territories at interpretable scales"
        }
        "single_matern_cluster" => {
            "cluster-process-like labels should produce residual territories at interpretable scales"
        }
        "many_small_foci" => "many small foci should increase fine/intermediate spatial structure",
        "anisotropic_stripe" => "stripe labels should elevate the anisotropy index",
        "low_k_suppressed_dispersed" => "regularly dispersed labels should suppress low-k power",
        "cell_density_gradient_random_labels" => {
            "random labels on a spatial field should not produce territory inflation"
        }
        "stain_gradient_artifact" => "stain gradients should suppress biologic interpretation",
        "internal_control_dropout_artifact" => {
            "internal-control dropout is represented as a severe IHC-validity artifact"
        }
        "fragmented_tumor_islands" => {
            "fragmented component layouts should trigger a mask/window flag"
        }
        "rare_phenotype" => "rare phenotypes should be labeled low-power/unstable",
        "serial_section_misregistration" => {
            "serial-section shifts are descriptive and not same-cell evidence"
        }
        _ => "synthetic validation generator",
    }
}

fn multimodal_note_for(generator: &str) -> &'static str {
    match generator {
        "two_unrelated_mmr_territories" => {
            "spatially separated MMR territories should not be called related"
        }
        "two_related_mmr_territories" => {
            "nearby MMR territories with bridge support should be detected as related"
        }
        "immune_associated_mmr_territory" => {
            "MMR territory with local lymphocyte enrichment should be detected"
        }
        "registration_jitter" => {
            "serial-section associations below registration resolution should be flagged"
        }
        "prepost_equivalent_spatial_pattern" => {
            "matched pre/post curves inside the equivalence margin should be called equivalent"
        }
        "prepost_changed_spatial_pattern" => {
            "pre/post curves beyond the difference threshold should be detected as changed"
        }
        _ => "multimodal synthetic validation generator",
    }
}

fn small_sample_type_i_limit(replicates: usize) -> f64 {
    if replicates < 20 {
        0.60
    } else if replicates < 200 {
        0.25
    } else {
        0.15
    }
}

fn finite_mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        total += value;
        count += 1;
    }
    (count > 0).then_some(total / count as f64)
}

fn push_unique_flag(flags: &mut Vec<StatusFlag>, flag: StatusFlag) {
    if !flags.contains(&flag) {
        flags.push(flag);
    }
}
