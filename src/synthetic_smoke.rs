use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    common::stats::mean_all_finite,
    config::{NeighborhoodNullModel, RegistrationTransform, ThreadSetting},
    errors::{MarklabError, Result},
    multimodal::{MultimodalAnalysisRun, MultimodalEngine},
    output::{
        CurveComparisonMethod, EnrichmentStatisticUnavailableReason, MarkedPatternResult,
        MultimodalResult, NeighborhoodEnrichmentResult, StatusFlag,
    },
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

const MULTIMODAL_GENERATORS: [&str; 22] = [
    "random_labels_no_association",
    "two_unrelated_mmr_territories",
    "immune_independent_mmr_territory",
    "registration_jitter_no_association",
    "two_related_mmr_territories",
    "immune_associated_mmr_territory",
    "cross_interaction_enrichment",
    "registration_jitter",
    "prepost_within_margin_spatial_pattern",
    "prepost_changed_spatial_pattern",
    "registration_residual_above_threshold",
    "too_few_landmarks",
    "degenerate_landmarks",
    "empty_he_cells",
    "empty_ihc_cells",
    "no_abnormal_cells",
    "sparse_graph",
    "zero_expected_edge_count",
    "multiple_cell_classes",
    "multiple_null_models",
    "rigid_rotation",
    "affine_deformation",
];

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SyntheticSmokeSummary {
    pub suite: String,
    pub suite_kind: &'static str,
    pub seed: u64,
    pub engine_version: &'static str,
    pub configuration: MarkedSmokeConfiguration,
    pub replicates: usize,
    pub status: String,
    pub alpha: f64,
    pub generators: Vec<&'static str>,
    pub results: BTreeMap<String, SyntheticSmokeResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MarkedSmokeConfiguration {
    pub permutations: usize,
    pub permutation_seed: u64,
    pub threads: usize,
    pub family_wise_alpha: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SyntheticSmokeResult {
    pub replicates_attempted: usize,
    pub replicates_completed: usize,
    pub replicates_failed: usize,
    pub failure_reasons: Vec<String>,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_low_k_excess: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_i_error_alpha_0_05: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_i_error_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_anisotropy_index: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_territory_count: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppression_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppression_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepost_incomparable_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepost_incomparable_confidence_interval: Option<BinomialConfidenceInterval>,
    pub acceptance_criterion: &'static str,
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
    pub permutation_seed_base: u64,
    pub permutation_seed_policy: &'static str,
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
    pub scenario_configuration: MultimodalScenarioConfiguration,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criterion_met_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criterion_met_confidence_interval: Option<BinomialConfidenceInterval>,
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

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalScenarioConfiguration {
    pub transform: &'static str,
    pub registration_max_rmse_um: f64,
    pub registration_min_landmarks: usize,
    pub radius_um: f64,
    pub label_pairs: Vec<[String; 2]>,
    pub null_models: Vec<String>,
    pub permutations: usize,
    pub cross_interaction_margin: Option<f64>,
    pub n_he_cells: usize,
    pub n_ihc_cells: usize,
    pub n_landmarks: usize,
    pub has_post: bool,
}

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
            permutation_seed_base: config.permutation.seed,
            permutation_seed_policy: "domain-derived from suite seed, scenario, and replicate",
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
    let mut failure_reasons = Vec::new();
    for replicate in 0..replicates {
        let analysis = synthetic_pattern(generator, replicate as u64)
            .and_then(|pattern| engine.analyze_pattern(&pattern));
        match analysis {
            Ok(analysis) => analyses.push(analysis),
            Err(error) => failure_reasons.push(format!("replicate {replicate}: {error}")),
        }
    }

    let mut result = summarize_analyses(&analyses, replicates, failure_reasons);
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
    result.acceptance_criterion = marked_acceptance_criterion(generator);
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
            let comparison = compare_prepost(&pre_result, &post_result);
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

    let mut result = summarize_analyses(&post_analyses, replicates, failure_reasons);
    for flag in comparison_flags {
        push_unique_flag(&mut result.status_flags, flag);
    }
    let incomparable_rate = observed_rate(incomparable_count, result.replicates_completed);
    result.prepost_incomparable_rate = incomparable_rate;
    result.prepost_incomparable_confidence_interval = incomparable_rate
        .and_then(|_| wilson_interval(incomparable_count, result.replicates_completed));
    result.passed = result.replicates_failed == 0 && incomparable_rate == Some(1.0);
    result.acceptance_criterion = marked_acceptance_criterion("prepost_metadata_mismatch");
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
    let report_scenario = multimodal_replicate_scenario(generator, seed, generator_index, 0)?;
    let scenario_configuration = multimodal_scenario_configuration(&report_scenario);
    let outcomes = (0..replicates).map(|replicate| {
        run_multimodal_replicate(generator, seed, generator_index, replicate)
            .map_err(|error| (replicate, error))
    });
    summarize_multimodal_outcomes(generator, replicates, scenario_configuration, outcomes)
}

fn summarize_multimodal_outcomes(
    generator: &str,
    replicates: usize,
    scenario_configuration: MultimodalScenarioConfiguration,
    outcomes: impl IntoIterator<
        Item = std::result::Result<ObservedMultimodalOutcome, (usize, MarklabError)>,
    >,
) -> Result<MultimodalSyntheticSmokeResult> {
    let mut detection_count = 0usize;
    let mut criterion_met_count = 0usize;
    let mut false_positive_count = 0usize;
    let mut below_resolution_count = 0usize;
    let mut within_margin_count = 0usize;
    let mut failure_reasons = Vec::new();

    for outcome in outcomes {
        match outcome {
            Ok(outcome) => {
                criterion_met_count += usize::from(outcome.criterion_met);
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
    let criterion_met_rate = observed_rate(criterion_met_count, replicates_completed);
    let false_positive_rate = observed_rate(false_positive_count, replicates_completed);
    let below_registration_resolution_flag_rate =
        observed_rate(below_resolution_count, replicates_completed);
    let within_margin_rate = observed_rate(within_margin_count, replicates_completed);
    let no_failed_replicates = replicates_failed == 0;
    let passed = no_failed_replicates
        && criterion_met_rate.is_some_and(|rate| rate >= multimodal_min_criterion_rate(generator));

    let (
        detection_rate,
        false_positive_rate,
        below_registration_resolution_rate,
        within_margin_rate,
    ) = match generator {
        "two_unrelated_mmr_territories" => (None, false_positive_rate, None, None),
        "random_labels_no_association"
        | "immune_independent_mmr_territory"
        | "registration_jitter_no_association" => (None, false_positive_rate, None, None),
        "two_related_mmr_territories"
        | "immune_associated_mmr_territory"
        | "cross_interaction_enrichment" => (detection_rate, None, None, None),
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
        "registration_residual_above_threshold"
        | "too_few_landmarks"
        | "degenerate_landmarks"
        | "empty_he_cells"
        | "empty_ihc_cells"
        | "no_abnormal_cells"
        | "sparse_graph"
        | "zero_expected_edge_count"
        | "multiple_cell_classes"
        | "multiple_null_models"
        | "rigid_rotation"
        | "affine_deformation" => (None, None, None, None),
        _ => unreachable!("unknown generator already rejected"),
    };

    let acceptance_criterion = multimodal_acceptance_criterion(generator);

    Ok(MultimodalSyntheticSmokeResult {
        replicates_attempted: replicates,
        replicates_completed,
        replicates_failed,
        failure_reasons,
        scenario_configuration,
        passed,
        criterion_met_rate,
        criterion_met_confidence_interval: criterion_met_rate
            .and_then(|_| wilson_interval(criterion_met_count, replicates_completed)),
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

fn multimodal_scenario_configuration(
    scenario: &generators::MultimodalScenario,
) -> MultimodalScenarioConfiguration {
    MultimodalScenarioConfiguration {
        transform: match scenario.config.registration.transform {
            RegistrationTransform::Rigid => "rigid",
            RegistrationTransform::Affine => "affine",
        },
        registration_max_rmse_um: scenario.config.registration.max_rmse_um,
        registration_min_landmarks: scenario.config.registration.min_landmarks,
        radius_um: scenario.config.neighborhood.radius_um,
        label_pairs: scenario.config.neighborhood.label_pairs.clone(),
        null_models: scenario
            .config
            .neighborhood
            .null_models
            .iter()
            .map(|null_model| multimodal_null_model_name(*null_model).to_owned())
            .collect(),
        permutations: scenario.config.permutation.b,
        cross_interaction_margin: scenario.config.comparison.margins.cross_interaction,
        n_he_cells: scenario.pre.he_cells.len(),
        n_ihc_cells: scenario.pre.ihc_cells.len(),
        n_landmarks: scenario.pre.landmarks.len(),
        has_post: scenario.post.is_some(),
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ObservedMultimodalOutcome {
    criterion_met: bool,
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
    let pre = match generator {
        "registration_residual_above_threshold" => {
            return expected_engine_error_outcome(
                engine.analyze_run(&scenario.pre),
                "exceeds configured max_rmse_um",
            );
        }
        "too_few_landmarks" => {
            return expected_engine_error_outcome(
                engine.analyze_run(&scenario.pre),
                "registration requires at least",
            );
        }
        "degenerate_landmarks" => {
            return expected_engine_error_outcome(
                engine.analyze_run(&scenario.pre),
                "must span nonzero distance",
            );
        }
        _ => engine.analyze_run(&scenario.pre)?,
    };

    match generator {
        "random_labels_no_association"
        | "immune_independent_mmr_territory"
        | "registration_jitter_no_association" => {
            let detected = immune_enrichment_detected(&pre.result)?;
            Ok(ObservedMultimodalOutcome {
                criterion_met: !detected,
                detected,
                false_positive: detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "two_unrelated_mmr_territories" => {
            let false_positive = territory_count(&pre.result)? == 1;
            Ok(ObservedMultimodalOutcome {
                criterion_met: !false_positive,
                false_positive,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "two_related_mmr_territories" => {
            let detected = territory_count(&pre.result)? == 1;
            Ok(ObservedMultimodalOutcome {
                criterion_met: detected,
                detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "immune_associated_mmr_territory" => {
            let detected = immune_enrichment_detected(&pre.result)?;
            Ok(ObservedMultimodalOutcome {
                criterion_met: detected,
                detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "cross_interaction_enrichment" => {
            let detected = cross_interaction_detected(&pre.result)?;
            Ok(ObservedMultimodalOutcome {
                criterion_met: detected,
                detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "registration_jitter" => {
            let detected = immune_enrichment_detected(&pre.result)?;
            let below_registration_resolution = pre
                .graph
                .edges
                .iter()
                .any(|edge| edge.below_registration_resolution);
            let false_positive = detected && !below_registration_resolution;
            Ok(ObservedMultimodalOutcome {
                criterion_met: below_registration_resolution && !false_positive,
                detected,
                false_positive,
                below_registration_resolution,
                within_margin: false,
            })
        }
        "prepost_within_margin_spatial_pattern" | "prepost_changed_spatial_pattern" => {
            prepost_outcome(generator, &scenario, &engine, &pre)
        }
        "empty_he_cells" => {
            let summary = fused_cell_summary(&pre.result)?;
            Ok(criterion_outcome(
                summary.n_he_cells == 0 && summary.n_ihc_cells > 0,
            ))
        }
        "empty_ihc_cells" => {
            let summary = fused_cell_summary(&pre.result)?;
            Ok(criterion_outcome(
                summary.n_ihc_cells == 0 && summary.n_he_cells > 0,
            ))
        }
        "no_abnormal_cells" => Ok(criterion_outcome(territory_count(&pre.result)? == 0)),
        "sparse_graph" => Ok(criterion_outcome(pre.graph.edges.is_empty())),
        "zero_expected_edge_count" => {
            let row = lymphocyte_enrichment(&pre.result)?;
            Ok(criterion_outcome(
                row.expected_edges == 0.0
                    && row.enrichment_ratio.is_none()
                    && row.enrichment_ratio_unavailable_reason
                        == Some(EnrichmentStatisticUnavailableReason::ZeroExpectedEdges),
            ))
        }
        "multiple_cell_classes" => Ok(criterion_outcome(
            pre.result
                .neighborhood_enrichment
                .value()
                .is_some_and(|rows| rows.len() == 3),
        )),
        "multiple_null_models" => Ok(criterion_outcome(pre.null_model_sensitivity.len() == 4)),
        "rigid_rotation" => Ok(criterion_outcome(
            approximately(pre.transform.m00, 0.0)
                && approximately(pre.transform.m01, -1.0)
                && approximately(pre.transform.m10, 1.0)
                && approximately(pre.transform.m11, 0.0)
                && approximately(pre.transform.m02, 10.0)
                && approximately(pre.transform.m12, -5.0),
        )),
        "affine_deformation" => Ok(criterion_outcome(
            approximately(pre.transform.m00, 1.1)
                && approximately(pre.transform.m01, 0.2)
                && approximately(pre.transform.m10, -0.1)
                && approximately(pre.transform.m11, 0.9)
                && approximately(pre.transform.m02, 3.0)
                && approximately(pre.transform.m12, -4.0),
        )),
        _ => Err(MarklabError::Validation(format!(
            "unknown multimodal synthetic generator {generator}"
        ))),
    }
}

fn expected_engine_error_outcome(
    result: Result<MultimodalAnalysisRun>,
    expected_message: &str,
) -> Result<ObservedMultimodalOutcome> {
    match result {
        Err(error) if error.to_string().contains(expected_message) => Ok(criterion_outcome(true)),
        Err(error) => Err(MarklabError::Validation(format!(
            "production engine returned an unexpected error: {error}"
        ))),
        Ok(_) => Ok(criterion_outcome(false)),
    }
}

fn criterion_outcome(criterion_met: bool) -> ObservedMultimodalOutcome {
    ObservedMultimodalOutcome {
        criterion_met,
        ..ObservedMultimodalOutcome::default()
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
    let row = lymphocyte_enrichment(result)?;
    let p_value = row.q_value.or(row.p_value).ok_or_else(|| {
        MarklabError::Validation(
            "production lymphocyte/mmr_abnormal enrichment was not evaluable".into(),
        )
    })?;
    Ok(p_value <= 0.05)
}

fn lymphocyte_enrichment(result: &MultimodalResult) -> Result<&NeighborhoodEnrichmentResult> {
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
    Ok(row)
}

fn cross_interaction_detected(result: &MultimodalResult) -> Result<bool> {
    result
        .cross_interaction_curves
        .value()
        .and_then(|curves| {
            curves.iter().find(|curve| {
                curve.label_a == "lymphocyte" && curve.label_b == "mmr_abnormal"
                    || curve.label_a == "mmr_abnormal" && curve.label_b == "lymphocyte"
            })
        })
        .and_then(|curve| curve.p_global)
        .map(|p_value| p_value <= 0.05)
        .ok_or_else(|| {
            MarklabError::Validation(
                "production multimodal result omitted evaluable lymphocyte/mmr_abnormal cross interaction"
                    .into(),
            )
        })
}

fn fused_cell_summary(result: &MultimodalResult) -> Result<&crate::output::FusedCellSummary> {
    result.fused_cell_summary.value().ok_or_else(|| {
        MarklabError::Validation(
            "production multimodal result did not provide a fused-cell summary".into(),
        )
    })
}

fn approximately(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1.0e-10
}

fn prepost_outcome(
    generator: &str,
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
        criterion_met: if generator == "prepost_within_margin_spatial_pattern" {
            within_margin
        } else {
            !within_margin
        },
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
        "random_labels_no_association" => {
            "smoke only: production enrichment does not detect an association in at least 90% of replicates"
        }
        "two_unrelated_mmr_territories" => {
            "smoke only: production keeps unrelated territories separate in at least 80% of replicates"
        }
        "two_related_mmr_territories" => {
            "smoke only: production merges the related territory in at least 80% of replicates"
        }
        "immune_associated_mmr_territory" => {
            "smoke only: production q-value <= 0.05 in at least 80% of replicates"
        }
        "immune_independent_mmr_territory" => {
            "smoke only: production enrichment does not detect independent immune cells in at least 80% of replicates"
        }
        "registration_jitter_no_association" => {
            "smoke only: noisy registration without association remains negative in at least 80% of replicates"
        }
        "cross_interaction_enrichment" => {
            "smoke only: production cross-interaction global p-value detects enrichment in at least 80% of replicates"
        }
        "registration_jitter" => {
            "smoke only: production flags the association below registration resolution in at least 80% of replicates"
        }
        "prepost_within_margin_spatial_pattern" => {
            "smoke only: production keeps matched pre/post curves within the descriptive margin in at least 80% of replicates"
        }
        "prepost_changed_spatial_pattern" => {
            "smoke only: production puts changed pre/post curves outside the descriptive margin in at least 80% of replicates"
        }
        "registration_residual_above_threshold" => {
            "smoke only: production engine rejects registration RMSE above the configured threshold"
        }
        "too_few_landmarks" => {
            "smoke only: production engine rejects fewer than the configured landmark minimum"
        }
        "degenerate_landmarks" => {
            "smoke only: production registration rejects degenerate landmark geometry"
        }
        "empty_he_cells" => {
            "smoke only: production result truthfully reports an empty H&E section"
        }
        "empty_ihc_cells" => {
            "smoke only: production result truthfully reports an empty IHC section"
        }
        "no_abnormal_cells" => {
            "smoke only: production territory result is an available empty set when no abnormal cells exist"
        }
        "sparse_graph" => "smoke only: production graph is empty under a sub-spacing radius",
        "zero_expected_edge_count" => {
            "smoke only: production enrichment types zero expectation instead of emitting a non-finite ratio"
        }
        "multiple_cell_classes" => {
            "smoke only: production result contains every configured cell-class enrichment pair"
        }
        "multiple_null_models" => {
            "smoke only: production run contains every configured null-model sensitivity"
        }
        "rigid_rotation" => {
            "smoke only: production rigid fit recovers the known rotation and translation"
        }
        "affine_deformation" => {
            "smoke only: production affine fit recovers the known deformation"
        }
        _ => "unknown smoke acceptance criterion",
    }
}

fn multimodal_min_criterion_rate(generator: &str) -> f64 {
    match generator {
        "random_labels_no_association" => 0.90,
        _ => 0.80,
    }
}

fn summarize_analyses(
    analyses: &[MarkedPatternResult],
    replicates_attempted: usize,
    failure_reasons: Vec<String>,
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
        acceptance_criterion: "pending scenario evaluation",
        status_flags,
        notes: Vec::new(),
    }
}

fn marked_acceptance_criterion(generator: &str) -> &'static str {
    match generator {
        "random_labeling" => {
            "smoke only: type-I rate must remain below the replicate-count-dependent guard"
        }
        "single_gaussian_cluster" | "single_matern_cluster" => {
            "smoke only: mean production residual-territory count >= 1"
        }
        "many_small_foci" => {
            "smoke only: mean production residual-territory count >= 4 with finite low-k excess"
        }
        "anisotropic_stripe" => "smoke only: mean production anisotropy index > 1.05",
        "low_k_suppressed_dispersed" => "smoke only: mean production low-k excess <= 1.25",
        "cell_density_gradient_random_labels" => {
            "smoke only: mean production residual-territory count <= 1"
        }
        "stain_gradient_artifact" => {
            "smoke only: every replicate is suppressed and carries the production stain-gradient flag"
        }
        "internal_control_dropout_artifact" => {
            "smoke only: production status includes internal-control failure overlap"
        }
        "fragmented_tumor_islands" => {
            "smoke only: production status includes mask-fragmentation suspect"
        }
        "rare_phenotype" => {
            "smoke only: production status includes too-few-marked underpowering"
        }
        "prepost_metadata_mismatch" => {
            "smoke only: every production pre/post comparison reports anatomical incomparability"
        }
        _ => "unknown smoke acceptance criterion",
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
        "random_labels_no_association" => {
            "random H&E labels without designed association are a negative enrichment control"
        }
        "two_unrelated_mmr_territories" => {
            "spatially separated MMR territories should not be called related"
        }
        "two_related_mmr_territories" => {
            "nearby MMR territories with bridge support should be detected as related"
        }
        "immune_associated_mmr_territory" => {
            "MMR territory with local lymphocyte enrichment should be detected"
        }
        "immune_independent_mmr_territory" => {
            "immune cells spatially independent of the MMR territory should remain negative"
        }
        "registration_jitter_no_association" => {
            "registration noise must not manufacture an absent immune/MMR association"
        }
        "cross_interaction_enrichment" => {
            "a known local immune/MMR association should alter the production cross-interaction curve"
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
        "registration_residual_above_threshold" => {
            "registration residuals above the configured maximum must fail production analysis"
        }
        "too_few_landmarks" => "too few landmarks must fail production input validation",
        "degenerate_landmarks" => "degenerate landmarks must fail production registration",
        "empty_he_cells" => "an empty H&E section must remain explicit in production results",
        "empty_ihc_cells" => "an empty IHC section must remain explicit in production results",
        "no_abnormal_cells" => {
            "absence of abnormal IHC cells should produce no neighborhood territories"
        }
        "sparse_graph" => "a radius below all cell spacings should produce no graph edges",
        "zero_expected_edge_count" => {
            "zero expected edges must produce a typed unavailable enrichment ratio"
        }
        "multiple_cell_classes" => {
            "every configured cell-class pair should appear in production enrichment"
        }
        "multiple_null_models" => {
            "every configured null model should appear in production sensitivity results"
        }
        "rigid_rotation" => "the rigid production path should recover a known 90-degree rotation",
        "affine_deformation" => {
            "the affine production path should recover a known shear and anisotropic scale"
        }
        _ => "multimodal synthetic generator smoke check generator",
    }
}

fn small_sample_type_i_limit(replicates: usize) -> f64 {
    if replicates < 20 {
        0.25
    } else if replicates < 100 {
        0.20
    } else {
        0.15
    }
}

fn push_unique_flag(flags: &mut Vec<StatusFlag>, flag: StatusFlag) {
    if !flags.contains(&flag) {
        flags.push(flag);
    }
}
