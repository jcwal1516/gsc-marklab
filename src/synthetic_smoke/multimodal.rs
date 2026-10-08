use std::collections::BTreeMap;

use crate::{
    config::RegistrationTransform,
    errors::{MarklabError, Result},
};

use super::{
    generators::{self, multimodal_replicate_scenario, multimodal_smoke_config},
    model::{
        MultimodalScenarioConfiguration, MultimodalSmokeConfiguration,
        MultimodalSyntheticSmokeResult, MultimodalSyntheticSmokeSummary,
    },
    multimodal_observation::{run_multimodal_replicate, ObservedMultimodalOutcome},
    policy::{
        multimodal_acceptance_criterion, multimodal_min_criterion_rate, multimodal_note_for,
        multimodal_null_model_name,
    },
    statistics::{observed_rate, wilson_interval},
};

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

pub(super) fn run_multimodal_generator(
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

pub(super) fn summarize_multimodal_outcomes(
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

pub(super) fn multimodal_scenario_configuration(
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
