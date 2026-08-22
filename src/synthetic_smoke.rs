use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    common::stats::mean_all_finite,
    config::{NeighborhoodNullModel, ThreadSetting},
    errors::{MarklabError, Result},
    multimodal::{MultimodalAnalysisRun, MultimodalEngine},
    output::{CurveComparisonMethod, MarkedPatternResult, MultimodalResult, StatusFlag},
    prepost::{compare_multimodal_prepost_with_margin, compare_prepost},
    AnalysisConfig, AnalysisEngine,
};

mod generators;

#[cfg(test)]
#[path = "synthetic_smoke/tests.rs"]
mod tests;

use generators::{multimodal_replicate_scenario, multimodal_smoke_config, synthetic_pattern};

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

const MULTIMODAL_GENERATORS: [&str; 6] = [
    "two_unrelated_mmr_territories",
    "two_related_mmr_territories",
    "immune_associated_mmr_territory",
    "registration_jitter",
    "prepost_within_margin_spatial_pattern",
    "prepost_changed_spatial_pattern",
];

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SyntheticSmokeSummary {
    pub suite: String,
    pub replicates: usize,
    pub status: String,
    pub alpha: f64,
    pub generators: Vec<&'static str>,
    pub results: BTreeMap<String, SyntheticSmokeResult>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct SyntheticSmokeResult {
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
    pub prepost_incomparable_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_flags: Vec<StatusFlag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSyntheticSmokeSummary {
    pub suite: String,
    pub suite_kind: &'static str,
    pub seed: u64,
    pub engine_version: &'static str,
    pub configuration: MultimodalSmokeConfiguration,
    #[serde(flatten)]
    pub results: BTreeMap<String, MultimodalSyntheticSmokeResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSmokeConfiguration {
    pub permutations: usize,
    pub permutation_seed: u64,
    pub radius_um: f64,
    pub null_models: Vec<String>,
    pub cross_interaction_margin: f64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct BinomialConfidenceInterval {
    pub confidence_level: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSyntheticSmokeResult {
    pub replicates_attempted: usize,
    pub replicates_completed: usize,
    pub replicates_failed: usize,
    pub failure_reasons: Vec<String>,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub false_positive_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub false_positive_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below_registration_resolution_flag_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below_registration_resolution_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within_margin_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within_margin_confidence_interval: Option<BinomialConfidenceInterval>,
    pub acceptance_criterion: &'static str,
    pub note: &'static str,
}

pub fn run_synthetic_smoke(replicates: usize) -> Result<SyntheticSmokeSummary> {
    if replicates == 0 {
        return Err(MarklabError::Validation(
            "synthetic generator smoke check requires at least one replicate".into(),
        ));
    }

    let config = smoke_config();
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

    Ok(SyntheticSmokeSummary {
        suite: "synthetic_generator_smoke".into(),
        replicates,
        status: status.into(),
        alpha: 0.05,
        generators: GENERATORS.to_vec(),
        results,
    })
}

pub fn run_multimodal_synthetic_smoke(
    replicates: usize,
    seed: u64,
) -> Result<MultimodalSyntheticSmokeSummary> {
    if replicates == 0 {
        return Err(MarklabError::Validation(
            "multimodal synthetic generator smoke check requires at least one replicate".into(),
        ));
    }
    let config = multimodal_smoke_config(seed);

    let mut results = BTreeMap::new();
    for (index, generator) in MULTIMODAL_GENERATORS.iter().enumerate() {
        results.insert(
            (*generator).into(),
            run_multimodal_generator(generator, replicates, seed, index as u64)?,
        );
    }
    Ok(MultimodalSyntheticSmokeSummary {
        suite: "multimodal_production_pipeline_smoke".into(),
        suite_kind: "smoke",
        seed,
        engine_version: env!("CARGO_PKG_VERSION"),
        configuration: MultimodalSmokeConfiguration {
            permutations: config.permutation.b,
            permutation_seed: config.permutation.seed,
            radius_um: config.neighborhood.radius_um,
            null_models: config
                .neighborhood
                .null_models
                .iter()
                .map(|null_model| multimodal_null_model_name(*null_model).to_owned())
                .collect(),
            cross_interaction_margin: config
                .comparison
                .margins
                .cross_interaction
                .expect("smoke config defines a cross-interaction margin"),
        },
        results,
    })
}

fn smoke_config() -> AnalysisConfig {
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

fn run_generator(
    generator: &str,
    replicates: usize,
    engine: &AnalysisEngine,
) -> Result<SyntheticSmokeResult> {
    if generator == "prepost_metadata_mismatch" {
        return run_marked_prepost_metadata_mismatch(replicates, engine);
    }

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
    Ok(result)
}

fn run_marked_prepost_metadata_mismatch(
    replicates: usize,
    engine: &AnalysisEngine,
) -> Result<SyntheticSmokeResult> {
    let mut post_analyses = Vec::with_capacity(replicates);
    let mut incomparable_count = 0usize;
    let mut comparison_flags = Vec::new();
    for replicate in 0..replicates {
        let mut pre = synthetic_pattern("prepost_metadata_mismatch", replicate as u64)?;
        pre.meta.case_id = "synthetic_pre_section".into();
        pre.meta.timepoint = "pre".into();
        let mut post = synthetic_pattern("prepost_metadata_mismatch", replicate as u64)?;
        post.meta.case_id = "synthetic_post_section".into();
        post.meta.timepoint = "post".into();

        let pre_result = engine.analyze_pattern(&pre)?;
        let post_result = engine.analyze_pattern(&post)?;
        let comparison = compare_prepost(&pre_result, &post_result);
        let incomparable = comparison
            .status_flags
            .contains(&StatusFlag::PrePostNotAnatomicallyComparable);
        incomparable_count += usize::from(incomparable);
        for flag in comparison.status_flags {
            push_unique_flag(&mut comparison_flags, flag);
        }
        post_analyses.push(post_result);
    }

    let mut result = summarize_analyses(&post_analyses);
    for flag in comparison_flags {
        push_unique_flag(&mut result.status_flags, flag);
    }
    let incomparable_rate = incomparable_count as f64 / replicates as f64;
    result.prepost_incomparable_rate = Some(incomparable_rate);
    result.passed = incomparable_rate == 1.0;
    result
        .notes
        .push(note_for("prepost_metadata_mismatch").into());
    Ok(result)
}

fn run_multimodal_generator(
    generator: &str,
    replicates: usize,
    seed: u64,
    generator_index: u64,
) -> Result<MultimodalSyntheticSmokeResult> {
    let outcomes = (0..replicates).map(|replicate| {
        run_multimodal_replicate(generator, seed, generator_index, replicate)
            .map_err(|error| (replicate, error))
    });
    summarize_multimodal_outcomes(generator, replicates, outcomes)
}

fn summarize_multimodal_outcomes(
    generator: &str,
    replicates: usize,
    outcomes: impl IntoIterator<
        Item = std::result::Result<ObservedMultimodalOutcome, (usize, MarklabError)>,
    >,
) -> Result<MultimodalSyntheticSmokeResult> {
    let mut detection_count = 0usize;
    let mut false_positive_count = 0usize;
    let mut below_resolution_count = 0usize;
    let mut within_margin_count = 0usize;
    let mut failure_reasons = Vec::new();

    for outcome in outcomes {
        match outcome {
            Ok(outcome) => {
                detection_count += usize::from(outcome.detected);
                false_positive_count += usize::from(outcome.false_positive);
                below_resolution_count += usize::from(outcome.below_registration_resolution);
                within_margin_count += usize::from(outcome.within_margin);
            }
            Err((replicate, error)) => {
                failure_reasons.push(format!("replicate {replicate}: {error}"));
            }
        }
    }

    let replicates_failed = failure_reasons.len();
    let replicates_completed = replicates.saturating_sub(replicates_failed);
    let detection_rate = observed_rate(detection_count, replicates_completed);
    let false_positive_rate = observed_rate(false_positive_count, replicates_completed);
    let below_registration_resolution_flag_rate =
        observed_rate(below_resolution_count, replicates_completed);
    let within_margin_rate = observed_rate(within_margin_count, replicates_completed);
    let no_failed_replicates = replicates_failed == 0;
    let passed = match generator {
        "two_unrelated_mmr_territories" => {
            no_failed_replicates && false_positive_rate.is_some_and(|rate| rate <= 0.20)
        }
        "two_related_mmr_territories" | "immune_associated_mmr_territory" => {
            no_failed_replicates && detection_rate.is_some_and(|rate| rate > 0.70)
        }
        "registration_jitter" => {
            no_failed_replicates
                && below_registration_resolution_flag_rate.is_some_and(|rate| rate > 0.80)
                && false_positive_rate.is_some_and(|rate| rate <= 0.20)
        }
        "prepost_within_margin_spatial_pattern" => {
            no_failed_replicates
                && within_margin_rate.is_some_and(|rate| rate > 0.80)
                && false_positive_rate.is_some_and(|rate| rate <= 0.20)
        }
        "prepost_changed_spatial_pattern" => {
            no_failed_replicates
                && detection_rate.is_some_and(|rate| rate > 0.70)
                && within_margin_rate.is_some_and(|rate| rate < 0.20)
        }
        _ => {
            return Err(MarklabError::Validation(format!(
                "unknown multimodal synthetic generator {generator}"
            )));
        }
    };

    let (
        detection_rate,
        false_positive_rate,
        below_registration_resolution_rate,
        within_margin_rate,
    ) = match generator {
        "two_unrelated_mmr_territories" => (None, false_positive_rate, None, None),
        "two_related_mmr_territories" | "immune_associated_mmr_territory" => {
            (detection_rate, None, None, None)
        }
        "registration_jitter" => (
            detection_rate,
            false_positive_rate,
            below_registration_resolution_flag_rate,
            None,
        ),
        "prepost_within_margin_spatial_pattern" => {
            (None, false_positive_rate, None, within_margin_rate)
        }
        "prepost_changed_spatial_pattern" => (detection_rate, None, None, within_margin_rate),
        _ => unreachable!("unknown generator already rejected"),
    };

    let acceptance_criterion = multimodal_acceptance_criterion(generator);

    Ok(MultimodalSyntheticSmokeResult {
        replicates_attempted: replicates,
        replicates_completed,
        replicates_failed,
        failure_reasons,
        passed,
        detection_rate,
        detection_confidence_interval: detection_rate
            .and_then(|_| wilson_interval(detection_count, replicates_completed)),
        false_positive_rate,
        false_positive_confidence_interval: false_positive_rate
            .and_then(|_| wilson_interval(false_positive_count, replicates_completed)),
        below_registration_resolution_flag_rate: below_registration_resolution_rate,
        below_registration_resolution_confidence_interval: below_registration_resolution_rate
            .and_then(|_| wilson_interval(below_resolution_count, replicates_completed)),
        within_margin_rate,
        within_margin_confidence_interval: within_margin_rate
            .and_then(|_| wilson_interval(within_margin_count, replicates_completed)),
        acceptance_criterion,
        note: multimodal_note_for(generator),
    })
}

#[derive(Clone, Copy, Debug, Default)]
struct ObservedMultimodalOutcome {
    detected: bool,
    false_positive: bool,
    below_registration_resolution: bool,
    within_margin: bool,
}

fn run_multimodal_replicate(
    generator: &str,
    seed: u64,
    generator_index: u64,
    replicate: usize,
) -> Result<ObservedMultimodalOutcome> {
    let scenario = multimodal_replicate_scenario(generator, seed, generator_index, replicate)?;
    let engine = MultimodalEngine::new(scenario.config.clone())?;
    let pre = engine.analyze_run(&scenario.pre)?;

    match generator {
        "two_unrelated_mmr_territories" => Ok(ObservedMultimodalOutcome {
            false_positive: territory_count(&pre.result)? == 1,
            ..ObservedMultimodalOutcome::default()
        }),
        "two_related_mmr_territories" => Ok(ObservedMultimodalOutcome {
            detected: territory_count(&pre.result)? == 1,
            ..ObservedMultimodalOutcome::default()
        }),
        "immune_associated_mmr_territory" => Ok(ObservedMultimodalOutcome {
            detected: immune_enrichment_detected(&pre.result)?,
            ..ObservedMultimodalOutcome::default()
        }),
        "registration_jitter" => {
            let detected = immune_enrichment_detected(&pre.result)?;
            let below_registration_resolution = pre
                .graph
                .edges
                .iter()
                .any(|edge| edge.below_registration_resolution);
            Ok(ObservedMultimodalOutcome {
                detected,
                false_positive: detected && !below_registration_resolution,
                below_registration_resolution,
                within_margin: false,
            })
        }
        "prepost_within_margin_spatial_pattern" | "prepost_changed_spatial_pattern" => {
            prepost_outcome(&scenario, &engine, &pre)
        }
        _ => Err(MarklabError::Validation(format!(
            "unknown multimodal synthetic generator {generator}"
        ))),
    }
}

fn territory_count(result: &MultimodalResult) -> Result<usize> {
    result
        .neighborhood_territories
        .value()
        .map(Vec::len)
        .ok_or_else(|| {
            MarklabError::Validation(
                "production multimodal result did not provide neighborhood territories".into(),
            )
        })
}

fn immune_enrichment_detected(result: &MultimodalResult) -> Result<bool> {
    let enrichment = result.neighborhood_enrichment.value().ok_or_else(|| {
        MarklabError::Validation(
            "production multimodal result did not provide neighborhood enrichment".into(),
        )
    })?;
    let row = enrichment
        .iter()
        .find(|row| row.label_a == "lymphocyte" && row.label_b == "mmr_abnormal")
        .ok_or_else(|| {
            MarklabError::Validation(
                "production multimodal result omitted lymphocyte/mmr_abnormal enrichment".into(),
            )
        })?;
    let p_value = row.q_value.or(row.p_value).ok_or_else(|| {
        MarklabError::Validation(
            "production lymphocyte/mmr_abnormal enrichment was not evaluable".into(),
        )
    })?;
    Ok(p_value <= 0.05)
}

fn prepost_outcome(
    scenario: &generators::MultimodalScenario,
    engine: &MultimodalEngine,
    pre: &MultimodalAnalysisRun,
) -> Result<ObservedMultimodalOutcome> {
    let post_input = scenario.post.as_ref().ok_or_else(|| {
        MarklabError::Validation("pre/post smoke scenario omitted the post input".into())
    })?;
    let post = engine.analyze_run(post_input)?;
    let comparison = compare_multimodal_prepost_with_margin(
        &pre.result,
        &post.result,
        scenario.config.comparison.margins.cross_interaction,
    );
    let within_margin = comparison
        .curve_comparisons
        .iter()
        .find(|comparison| comparison.method == CurveComparisonMethod::DescriptiveMargin)
        .and_then(|comparison| comparison.within_margin)
        .ok_or_else(|| {
            MarklabError::Validation(
                "production pre/post comparison did not provide a descriptive margin result".into(),
            )
        })?;

    Ok(ObservedMultimodalOutcome {
        detected: !within_margin,
        false_positive: !within_margin,
        below_registration_resolution: false,
        within_margin,
    })
}

fn observed_rate(successes: usize, completed: usize) -> Option<f64> {
    (completed > 0).then_some(successes as f64 / completed as f64)
}

fn wilson_interval(successes: usize, completed: usize) -> Option<BinomialConfidenceInterval> {
    if completed == 0 || successes > completed {
        return None;
    }
    const Z_95: f64 = 1.959_963_984_540_054;
    let n = completed as f64;
    let p = successes as f64 / n;
    let z2 = Z_95 * Z_95;
    let denominator = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denominator;
    let half_width = Z_95 * ((p * (1.0 - p) + z2 / (4.0 * n)) / n).sqrt() / denominator;
    Some(BinomialConfidenceInterval {
        confidence_level: 0.95,
        lower: if successes == 0 {
            0.0
        } else {
            (center - half_width).max(0.0)
        },
        upper: if successes == completed {
            1.0
        } else {
            (center + half_width).min(1.0)
        },
    })
}

const fn multimodal_null_model_name(null_model: NeighborhoodNullModel) -> &'static str {
    match null_model {
        NeighborhoodNullModel::SourceSection => "source_section",
        NeighborhoodNullModel::SourceSectionDensity => "source_section_density",
        NeighborhoodNullModel::SourceSectionCellClass => "source_section_cell_class",
        NeighborhoodNullModel::SourceSectionRegistrationQc => "source_section_registration_qc",
    }
}

fn multimodal_acceptance_criterion(generator: &str) -> &'static str {
    match generator {
        "two_unrelated_mmr_territories" => {
            "smoke only: production territory-merger false-positive rate <= 0.20"
        }
        "two_related_mmr_territories" => {
            "smoke only: production single-territory detection rate > 0.70"
        }
        "immune_associated_mmr_territory" => {
            "smoke only: production q-value <= 0.05 detection rate > 0.70"
        }
        "registration_jitter" => {
            "smoke only: production below-resolution edge rate > 0.80 and unflagged detection rate <= 0.20"
        }
        "prepost_within_margin_spatial_pattern" => {
            "smoke only: production descriptive-margin rate > 0.80 and outside-margin false-positive rate <= 0.20"
        }
        "prepost_changed_spatial_pattern" => {
            "smoke only: production outside-margin detection rate > 0.70 and descriptive-margin rate < 0.20"
        }
        _ => "unknown smoke acceptance criterion",
    }
}

fn summarize_analyses(analyses: &[MarkedPatternResult]) -> SyntheticSmokeResult {
    let mut status_flags = Vec::new();
    for analysis in analyses {
        for flag in &analysis.status_flags {
            push_unique_flag(&mut status_flags, *flag);
        }
    }

    let replicates_run = analyses.len();
    let denom = replicates_run.max(1) as f64;
    let mean_low_k_excess = mean_all_finite(
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
    let suppression_rate = analyses
        .iter()
        .filter(|analysis| analysis.status != "ok")
        .count() as f64
        / denom;

    SyntheticSmokeResult {
        replicates_run,
        passed: false,
        mean_low_k_excess,
        type_i_error_alpha_0_05: Some(type_i_error_alpha_0_05),
        detection_rate: Some(detection_rate),
        mean_anisotropy_index,
        mean_territory_count,
        suppression_rate: Some(suppression_rate),
        prepost_incomparable_rate: None,
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
        "many_small_foci" => "many small foci should increase local-difference or residual scale energy",
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
        "prepost_metadata_mismatch" => {
            "mismatched pre/post identifiers must be reported as not anatomically comparable"
        }
        _ => "synthetic generator smoke check generator",
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
        "prepost_within_margin_spatial_pattern" => {
            "matched pre/post curves should remain within the configured descriptive margin"
        }
        "prepost_changed_spatial_pattern" => {
            "pre/post curves beyond the difference threshold should be detected as changed"
        }
        _ => "multimodal synthetic generator smoke check generator",
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

fn push_unique_flag(flags: &mut Vec<StatusFlag>, flag: StatusFlag) {
    if !flags.contains(&flag) {
        flags.push(flag);
    }
}
