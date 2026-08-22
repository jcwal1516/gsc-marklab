use std::collections::BTreeMap;

use crate::{
    common::stats::mean_all_finite,
    config::ThreadSetting,
    errors::{MarklabError, Result},
    output::{MarkedPatternResult, StatusFlag},
    prepost::compare_marked_prepost,
    AnalysisConfig, AnalysisEngine,
};

use super::{
    generators::synthetic_pattern,
    model::{MarkedSmokeConfiguration, SyntheticSmokeResult, SyntheticSmokeSummary},
    policy::{marked_acceptance_criterion, note_for, small_sample_type_i_limit},
    statistics::{observed_rate, wilson_interval},
};

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
    "prepost_metadata_mismatch",
];

pub fn run_synthetic_smoke(replicates: usize) -> Result<SyntheticSmokeSummary> {
    if replicates == 0 {
        return Err(MarklabError::Validation(
            "synthetic generator smoke check requires at least one replicate".into(),
        ));
    }

    let config = smoke_config();
    let engine = AnalysisEngine::new(config.clone())?;
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

    Ok(SyntheticSmokeSummary {
        suite: "synthetic_generator_smoke".into(),
        suite_kind: "smoke",
        seed: config.permutation.seed,
        engine_version: env!("CARGO_PKG_VERSION"),
        configuration: MarkedSmokeConfiguration {
            permutations: config.permutation.b,
            permutation_seed: config.permutation.seed,
            threads: 1,
            family_wise_alpha: config.inference.family_wise_alpha,
        },
        replicates,
        status: status.into(),
        alpha: 0.05,
        generators: GENERATORS.to_vec(),
        results,
    })
}

pub(super) fn smoke_config() -> AnalysisConfig {
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
    // Fixed smoke scenarios retain alpha=0.05 and therefore need at least
    // 40 total curves for equal-tail endpoints.
    config.permutation.b = 39;
    config.permutation.seed = 9_001;
    config.permutation.stratified = false;
    config.permutation.strata_fields.clear();
    config.performance.threads = ThreadSetting::Count(1);
    config
}

pub(super) fn run_generator(
    generator: &str,
    replicates: usize,
    engine: &AnalysisEngine,
) -> Result<SyntheticSmokeResult> {
    if generator == "prepost_metadata_mismatch" {
        return run_marked_prepost_metadata_mismatch(replicates, engine);
    }

    let mut analyses = Vec::with_capacity(replicates);
    let mut failure_reasons = Vec::new();
    for replicate in 0..replicates {
        let analysis = synthetic_pattern(generator, replicate as u64)
            .and_then(|pattern| engine.analyze_pattern(&pattern));
        match analysis {
            Ok(analysis) => analyses.push(analysis),
            Err(error) => failure_reasons.push(format!("replicate {replicate}: {error}")),
        }
    }

    let mut result = summarize_analyses(
        &analyses,
        replicates,
        failure_reasons,
        marked_acceptance_criterion(generator),
    );
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
                .is_some_and(|count| count >= 4.0)
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
        _ => {
            return Err(MarklabError::Validation(format!(
                "unknown synthetic generator {generator}"
            )));
        }
    }
    result.passed &= result.replicates_failed == 0;
    Ok(result)
}

fn run_marked_prepost_metadata_mismatch(
    replicates: usize,
    engine: &AnalysisEngine,
) -> Result<SyntheticSmokeResult> {
    let mut post_analyses = Vec::with_capacity(replicates);
    let mut incomparable_count = 0usize;
    let mut comparison_flags = Vec::new();
    let mut failure_reasons = Vec::new();
    for replicate in 0..replicates {
        let outcome: Result<_> = (|| {
            let mut pre = synthetic_pattern("prepost_metadata_mismatch", replicate as u64)?;
            pre.meta.case_id = "synthetic_pre_section".into();
            pre.meta.timepoint = "pre".into();
            let mut post = synthetic_pattern("prepost_metadata_mismatch", replicate as u64)?;
            post.meta.case_id = "synthetic_post_section".into();
            post.meta.timepoint = "post".into();

            let pre_result = engine.analyze_pattern(&pre)?;
            let post_result = engine.analyze_pattern(&post)?;
            let comparison = compare_marked_prepost(&pre_result, &post_result);
            Ok((post_result, comparison))
        })();
        match outcome {
            Ok((post_result, comparison)) => {
                let incomparable = comparison
                    .status_flags
                    .contains(&StatusFlag::PrePostNotAnatomicallyComparable);
                incomparable_count += usize::from(incomparable);
                for flag in comparison.status_flags {
                    push_unique_flag(&mut comparison_flags, flag);
                }
                post_analyses.push(post_result);
            }
            Err(error) => failure_reasons.push(format!("replicate {replicate}: {error}")),
        }
    }

    let mut result = summarize_analyses(
        &post_analyses,
        replicates,
        failure_reasons,
        marked_acceptance_criterion("prepost_metadata_mismatch"),
    );
    for flag in comparison_flags {
        push_unique_flag(&mut result.status_flags, flag);
    }
    let incomparable_rate = observed_rate(incomparable_count, result.replicates_completed);
    result.prepost_incomparable_rate = incomparable_rate;
    result.prepost_incomparable_confidence_interval = incomparable_rate
        .and_then(|_| wilson_interval(incomparable_count, result.replicates_completed));
    result.passed = result.replicates_failed == 0 && incomparable_rate == Some(1.0);
    result
        .notes
        .push(note_for("prepost_metadata_mismatch").into());
    Ok(result)
}

pub(super) fn summarize_analyses(
    analyses: &[MarkedPatternResult],
    replicates_attempted: usize,
    failure_reasons: Vec<String>,
    acceptance_criterion: &'static str,
) -> SyntheticSmokeResult {
    let mut status_flags = Vec::new();
    for analysis in analyses {
        for flag in &analysis.status_flags {
            push_unique_flag(&mut status_flags, *flag);
        }
    }

    let replicates_completed = analyses.len();
    let replicates_failed = failure_reasons.len();
    let mean_low_k_excess = mean_all_finite(
        analyses
            .iter()
            .filter_map(|analysis| analysis.spectrum.value().map(|value| value.low_k_excess)),
    );
    let detection_count = analyses
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
        .count();
    let type_i_error_count = analyses
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
        .count();
    let mean_anisotropy_index = mean_all_finite(
        analyses
            .iter()
            .filter_map(|analysis| analysis.anisotropy.value().map(|value| value.index)),
    );
    let mean_territory_count = mean_all_finite(analyses.iter().filter_map(|analysis| {
        analysis
            .multiscale_residual
            .value()
            .map(|value| value.territory_count as f64)
    }));
    let suppression_count = analyses
        .iter()
        .filter(|analysis| analysis.status != crate::output::AnalysisStatus::Ok)
        .count();
    let detection_rate = observed_rate(detection_count, replicates_completed);
    let type_i_error_alpha_0_05 = observed_rate(type_i_error_count, replicates_completed);
    let suppression_rate = observed_rate(suppression_count, replicates_completed);

    SyntheticSmokeResult {
        replicates_attempted,
        replicates_completed,
        replicates_failed,
        failure_reasons,
        passed: false,
        mean_low_k_excess,
        type_i_error_alpha_0_05,
        type_i_error_confidence_interval: type_i_error_alpha_0_05
            .and_then(|_| wilson_interval(type_i_error_count, replicates_completed)),
        detection_rate,
        detection_confidence_interval: detection_rate
            .and_then(|_| wilson_interval(detection_count, replicates_completed)),
        mean_anisotropy_index,
        mean_territory_count,
        suppression_rate,
        suppression_confidence_interval: suppression_rate
            .and_then(|_| wilson_interval(suppression_count, replicates_completed)),
        prepost_incomparable_rate: None,
        prepost_incomparable_confidence_interval: None,
        acceptance_criterion,
        status_flags,
        notes: Vec::new(),
    }
}

fn push_unique_flag(flags: &mut Vec<StatusFlag>, flag: StatusFlag) {
    if !flags.contains(&flag) {
        flags.push(flag);
    }
}
