use std::fs;

use crate::{
    data::{validate::validation_flags, PatternMeta},
    geom::mask::TumorMask,
    io::csv::load_pattern_csv_with_diagnostics,
    multimodal::{multimodal_analysis_call_count, reset_multimodal_analysis_call_count},
    synthetic_smoke::{
        generators::{calibration_pattern, CalibrationLabels, CalibrationLayout},
        model::BinomialConfidenceInterval,
        run_synthetic_smoke,
        statistics::{observed_rate, wilson_bounds, wilson_interval},
    },
    AnalysisConfig, AnalysisEngine, AnalysisStatus, Pattern, StatusFlag,
};

const CALIBRATION_ALPHA: f64 = 0.05;

/// Calibration fails only on evidence that the false-positive rate is above alpha.
///
/// Requiring the upper bound to sit below alpha would reject an exactly sized test most of
/// the time, so the rule tests anti-conservatism instead.
fn significantly_exceeds_alpha(interval: &BinomialConfidenceInterval) -> bool {
    interval.lower > CALIBRATION_ALPHA
}

#[test]
fn calibration_rule_accepts_an_exact_test_and_rejects_an_inflated_one() {
    let exact = wilson_interval(50, 1_000).expect("interval");
    let inflated = wilson_interval(70, 1_000).expect("interval");

    assert!(!significantly_exceeds_alpha(&exact));
    assert!(significantly_exceeds_alpha(&inflated));
}

/// Minimum share of attempts that must yield a p-value in a calibration scenario.
const MINIMUM_AVAILABILITY: f64 = 0.95;
/// Minimum Wilson 95% lower bound on the rejection rate of a declared alternative.
const MINIMUM_POWER_LOWER_BOUND: f64 = 0.8;
const Z_95: f64 = 1.959_963_984_540_054;

#[derive(Clone, Default)]
struct CalibrationTally {
    attempted: usize,
    available: usize,
    rejections: usize,
    errors: Vec<String>,
}

impl CalibrationTally {
    fn record(&mut self, replicate: usize, outcome: std::result::Result<Option<f64>, String>) {
        self.attempted += 1;
        match outcome {
            Ok(Some(p_value)) => {
                self.available += 1;
                self.rejections += usize::from(p_value <= CALIBRATION_ALPHA);
            }
            Ok(None) => {}
            Err(error) => self.errors.push(format!("replicate {replicate}: {error}")),
        }
    }
}

/// One-sided quantile that keeps the family-wise false-alarm rate at 2.5% across `nulls`
/// anti-conservatism checks (Bonferroni), as in the pathology calibration.
fn bonferroni_z(nulls: usize) -> f64 {
    statrs::distribution::ContinuousCDF::inverse_cdf(
        &statrs::distribution::Normal::standard(),
        1.0 - 0.025 / nulls as f64,
    )
}

/// A new JSON-lines file in the directory named by `MARKLAB_SHUFFLE_CALIBRATION_OUTPUT`.
fn calibration_output(file_name: &str) -> fs::File {
    let directory = std::env::var("MARKLAB_SHUFFLE_CALIBRATION_OUTPUT")
        .expect("set MARKLAB_SHUFFLE_CALIBRATION_OUTPUT to a results directory");
    fs::create_dir_all(&directory).expect("calibration output directory");
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(std::path::Path::new(&directory).join(file_name))
        .expect("new calibration output file")
}

/// What a calibration scenario is evidence for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScenarioRole {
    /// Fails only on evidence that the rejection rate exceeds alpha.
    Null,
    /// Fails when power is not shown to reach 0.8.
    Power,
    /// Reported without a rejection-rate criterion, to document behaviour outside the
    /// statistic's declared scope.
    Descriptive,
}

/// Writes one scenario row and returns its failure, if any. Every role requires p-values
/// for at least 95% of attempts.
fn judge_scenario(
    output: &mut fs::File,
    scenario: serde_json::Value,
    role: ScenarioRole,
    tally: &CalibrationTally,
    null_z: f64,
) -> Option<String> {
    use std::io::Write;

    let z = if role == ScenarioRole::Null {
        null_z
    } else {
        Z_95
    };
    let bounds = wilson_bounds(tally.rejections, tally.available, z);
    let availability = tally.available as f64 / tally.attempted as f64;
    let row = serde_json::json!({
        "scenario": scenario,
        "role": match role {
            ScenarioRole::Null => "null",
            ScenarioRole::Power => "power",
            ScenarioRole::Descriptive => "descriptive",
        },
        "attempted": tally.attempted,
        "available": tally.available,
        "rejections": tally.rejections,
        "rate_among_available": observed_rate(tally.rejections, tally.available),
        "wilson_z": z,
        "wilson_bounds": bounds,
        "error_count": tally.errors.len(),
        "first_errors": &tally.errors[..tally.errors.len().min(5)],
    });
    writeln!(output, "{row}").expect("write calibration row");
    println!("{row}");
    if availability < MINIMUM_AVAILABILITY {
        return Some(format!(
            "{scenario}: p-value available for {}/{} attempts, below {MINIMUM_AVAILABILITY}",
            tally.available, tally.attempted
        ));
    }
    let (lower, upper) = bounds?;
    if role == ScenarioRole::Null && lower > CALIBRATION_ALPHA {
        Some(format!(
            "{scenario} exceeds nominal alpha: {}/{} rejections, Bonferroni Wilson interval \
             [{lower}, {upper}] (z = {z})",
            tally.rejections, tally.available
        ))
    } else if role == ScenarioRole::Power && lower < MINIMUM_POWER_LOWER_BOUND {
        Some(format!(
            "{scenario} lacks power: {}/{} rejections, Wilson 95% interval [{lower}, {upper}]",
            tally.rejections, tally.available
        ))
    } else {
        None
    }
}

fn meta() -> PatternMeta {
    PatternMeta {
        case_id: "case_001".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
        slide_id: None,
        section_id: None,
        stain_batch: None,
        block_id: None,
        region_id: None,
    }
}

#[test]
fn default_config_matches_spec_thresholds() {
    let config = AnalysisConfig::default();

    assert_eq!(config.validation.n_min, 200);
    assert_eq!(config.validation.n_marked_min, 25);
    assert_eq!(config.validation.n_unmarked_min, 25);
    assert_eq!(config.validation.p_min, 0.02);
    assert_eq!(config.validation.p_max, 0.98);
    assert_eq!(config.validation.area_min_um2, 100_000.0);
    assert_eq!(config.validation.k_shell_min, 5);
    assert_eq!(config.spectrum.low_k_shells, 3);
    assert_eq!(config.permutation.b, 999);
    assert_eq!(config.permutation.seed, 123_456_789);
    assert_eq!(config.inference.family_wise_alpha, 0.05);
}

#[test]
fn config_loads_complete_toml_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[analysis]
mark_label = "MMR loss"
use_probabilistic_marks = false
analyze_components = "auto"

[validation]
n_min = 300
n_marked_min = 30
n_unmarked_min = 40
p_min = 0.01
p_max = 0.99
area_min_um2 = 200000.0
k_shell_min = 6
largest_interpretable_scale_fraction = 0.25
valid_mask_fraction_min = 0.75

[spectrum]
k_shells = 32
low_k_shells = 4
fit_low_k_alpha = true
anisotropy_low_k_shells = 6

[periodogram]
enabled = true

[multiscale_residual]
enabled = true
territory_detection = true
min_territory_z = 3.0

[permutation]
b = 199
seed = 42
stratified = false
strata_fields = []

[inference]
family_wise_alpha = 0.05

[performance]
threads = 4
memory_budget_mib = 2048
k_chunk_modes = 128
strict_repro = true
save_intermediates = false

[output]
write_parquet_curves = true
write_geojson_territories = false
write_figures = false
write_run_manifest = true
"#,
    )
    .expect("write config");

    let config = AnalysisConfig::from_toml_path(&path).expect("load config");

    assert_eq!(config.validation.n_min, 300);
    assert_eq!(config.inference.family_wise_alpha, 0.05);
    assert_eq!(config.spectrum.k_shells, 32);
    assert_eq!(config.permutation.b, 199);
    assert_eq!(config.performance.memory_budget_mib, 2048);
    assert!(config.performance.strict_repro);
    assert!(!config.output.write_figures);
}

#[test]
fn synthetic_smoke_controls_random_labeling_and_detects_anisotropy() {
    let summary = run_synthetic_smoke(100).expect("synthetic generator smoke check");
    let random = &summary.results["random_labeling"];
    let stripe = &summary.results["anisotropic_stripe"];

    assert!(
        random.passed,
        "random-labeling smoke check failed: {random:?}"
    );
    assert!(
        stripe.passed,
        "anisotropic stripe detection failed: {stripe:?}"
    );
}

#[test]
fn marked_smoke_uses_production_qc_and_prepost_outcomes() {
    let summary = run_synthetic_smoke(1).expect("synthetic generator smoke check");
    let internal_control = &summary.results["internal_control_dropout_artifact"];
    let many_foci = &summary.results["many_small_foci"];
    let metadata_mismatch = &summary.results["prepost_metadata_mismatch"];

    assert!(internal_control
        .status_flags
        .contains(&StatusFlag::InternalControlFailureOverlap));
    assert!(!internal_control
        .status_flags
        .contains(&StatusFlag::InvalidIhcMask));
    assert!(many_foci
        .mean_territory_count
        .is_some_and(|count| count >= 4.0));
    assert!(metadata_mismatch
        .status_flags
        .contains(&StatusFlag::PrePostNotAnatomicallyComparable));
    assert_eq!(metadata_mismatch.prepost_incomparable_rate, Some(1.0));
    for result in summary.results.values() {
        assert_eq!(result.replicates_attempted, 1);
        assert_eq!(result.replicates_completed, 1);
        assert_eq!(result.replicates_failed, 0);
        assert!(result.failure_reasons.is_empty());
        assert!(!result.acceptance_criterion.is_empty());
    }
}

#[test]
fn unknown_smoke_generators_are_rejected_before_running() {
    let engine = AnalysisEngine::new(crate::synthetic_smoke::smoke_config()).expect("engine");
    let error = crate::synthetic_smoke::run_generator("not_a_generator", 2, &engine)
        .expect_err("unknown generator");
    assert!(error
        .to_string()
        .contains("unknown synthetic smoke generator not_a_generator"));
}

#[test]
fn marked_smoke_reports_failed_replicates_without_hiding_denominators() {
    let pattern = crate::synthetic_smoke::generators::synthetic_pattern("random_labeling", 0)
        .expect("pattern");
    let engine = AnalysisEngine::new(crate::synthetic_smoke::smoke_config()).expect("engine");
    let analysis = engine.analyze_pattern(&pattern).expect("analysis");

    let result = crate::synthetic_smoke::summarize_analyses(
        &[analysis],
        2,
        vec!["replicate 1: synthetic execution failure".into()],
        "test acceptance criterion",
    );

    assert_eq!(result.replicates_attempted, 2);
    assert_eq!(result.replicates_completed, 1);
    assert_eq!(result.replicates_failed, 1);
    assert_eq!(result.failure_reasons.len(), 1);
    assert!(result.type_i_error_confidence_interval.is_some());
    assert_eq!(result.acceptance_criterion, "test acceptance criterion");
}

#[test]
#[ignore = "scheduled calibration: 32,000 null and 5,000 alternative analyses; use release mode"]
fn marked_shuffle_calibration() {
    let mut output = calibration_output("marked.jsonl");
    // (layout, marked percent, labels, permutations, role)
    let mut scenarios = Vec::new();
    for layout in CalibrationLayout::ALL {
        for marked_percent in [10, 25, 50] {
            scenarios.push((
                layout,
                marked_percent,
                CalibrationLabels::Uniform,
                39,
                ScenarioRole::Null,
            ));
        }
    }
    scenarios.push((
        CalibrationLayout::Square,
        25,
        CalibrationLabels::Uniform,
        199,
        ScenarioRole::Null,
    ));
    for layout in CalibrationLayout::ALL {
        scenarios.push((
            layout,
            25,
            CalibrationLabels::SmallFoci,
            39,
            ScenarioRole::Power,
        ));
    }
    // One focus holding a quarter of the cells is comparable to or larger than the low-k
    // endpoint's largest interpretable scale (0.33 of the window diagonal). It is reported
    // without a power criterion to document that limit.
    for layout in CalibrationLayout::ALL {
        scenarios.push((
            layout,
            25,
            CalibrationLabels::SingleFocus,
            39,
            ScenarioRole::Descriptive,
        ));
    }
    let null_z = bonferroni_z(
        scenarios
            .iter()
            .filter(|scenario| scenario.4 == ScenarioRole::Null)
            .count(),
    );
    // Scenarios are independent and individually seeded, so they run concurrently; rows are
    // written afterwards in scenario order.
    let tallies = std::thread::scope(|scope| {
        let handles = scenarios
            .iter()
            .enumerate()
            .map(
                |(index, &(layout, marked_percent, labels, permutations, role))| {
                    scope.spawn(move || {
                        let mut tally = CalibrationTally::default();
                        let attempts = if role == ScenarioRole::Null {
                            2_000
                        } else {
                            500
                        };
                        for replicate in 0..attempts {
                            // Draw a fresh Monte Carlo reference set for each replicate. The
                            // permutation test is exact only on average over that draw; one seed
                            // shared by every replicate would measure the size of a single fixed
                            // reference set instead.
                            // The shipped configuration, including its 64-shell spectrum and
                            // validation minimums, with fewer permutations and one thread.
                            let mut config = AnalysisConfig::default();
                            config.permutation.b = permutations;
                            config.permutation.stratified = false;
                            config.permutation.strata_fields.clear();
                            config.performance.threads = crate::config::ThreadSetting::Count(1);
                            config.permutation.seed = crate::common::seeds::splitmix64(
                                ((index as u64) << 32) ^ replicate as u64 ^ 0x7065_726d_7574_6573,
                            );
                            let outcome = AnalysisEngine::new(config)
                                .and_then(|engine| {
                                    calibration_pattern(
                                        layout,
                                        marked_percent,
                                        labels,
                                        index as u64,
                                        replicate as u64,
                                    )
                                    .and_then(|pattern| engine.analyze_pattern(&pattern))
                                })
                                .map(|analysis| analysis.primary_endpoint.p_value.value().copied())
                                .map_err(|error| error.to_string());
                            tally.record(replicate, outcome);
                        }
                        tally
                    })
                },
            )
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("calibration scenario thread"))
            .collect::<Vec<_>>()
    });
    let mut failures = Vec::new();
    for (&(layout, marked_percent, labels, permutations, role), tally) in
        scenarios.iter().zip(&tallies)
    {
        let scenario = serde_json::json!({
            "workflow": "analyze",
            "layout": layout.name(),
            "marked_percent": marked_percent,
            "labels": labels.name(),
            "permutations": permutations,
        });
        failures.extend(judge_scenario(&mut output, scenario, role, tally, null_z));
    }
    // Report every scenario before failing so one regression does not hide another.
    assert!(
        failures.is_empty(),
        "shuffle calibration criteria failed; preserve output and investigate:\n{}",
        failures.join("\n")
    );
}

mod multimodal {
    #[test]
    fn positive_control_detects_signal() {
        let summary =
            crate::synthetic_smoke::run_multimodal_synthetic_smoke(100, 123).expect("smoke check");
        assert!(
            summary.results["immune_associated_mmr_territory"]
                .detection_rate
                .expect("rate")
                > 0.70
        );
    }

    #[test]
    fn smoke_flags_below_registration_resolution_associations() {
        let summary =
            crate::synthetic_smoke::run_multimodal_synthetic_smoke(25, 456).expect("smoke check");
        assert!(
            summary.results["registration_jitter"]
                .below_registration_resolution_flag_rate
                .expect("rate")
                > 0.80
        );
    }

    #[test]
    fn rotation_scenario_requires_real_rigid_transform() {
        let result = crate::synthetic_smoke::run_multimodal_generator("rigid_rotation", 1, 123, 20)
            .expect("rigid rotation scenario");

        assert_eq!(result.criterion_met_rate, Some(1.0));
    }

    #[test]
    fn registration_jitter_uses_actual_registration_output() {
        let result =
            crate::synthetic_smoke::run_multimodal_generator("registration_jitter", 1, 123, 7)
                .expect("registration jitter scenario");

        assert_eq!(result.below_registration_resolution_flag_rate, Some(1.0));
    }

    #[test]
    fn prepost_equivalence_uses_actual_comparison() {
        let result = crate::synthetic_smoke::run_multimodal_generator(
            "prepost_within_margin_spatial_pattern",
            1,
            123,
            8,
        )
        .expect("pre/post margin scenario");

        assert_eq!(result.within_margin_rate, Some(1.0));
        assert!(result.note.contains("descriptive margin"));
    }

    #[test]
    fn smoke_covers_required_negative_positive_and_edge_scenarios() {
        let summary =
            crate::synthetic_smoke::run_multimodal_synthetic_smoke(1, 123).expect("smoke check");

        for scenario in [
            "random_labels_no_association",
            "immune_independent_mmr_territory",
            "registration_jitter_no_association",
            "cross_interaction_enrichment",
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
        ] {
            let result = &summary.results[scenario];
            assert_eq!(result.replicates_completed, 1, "{scenario}: {result:?}");
            assert_eq!(result.replicates_failed, 0, "{scenario}: {result:?}");
            assert!(result.passed, "{scenario}: {result:?}");
        }
        assert_eq!(
            summary.results["multiple_null_models"]
                .scenario_configuration
                .null_models
                .len(),
            4
        );
    }

    #[test]
    #[ignore = "scheduled calibration: 8,000 null runs and 500 alternative runs; use release mode"]
    fn shuffle_calibration() {
        use crate::{
            multimodal::{MultimodalAnalysisRun, MultimodalEngine},
            synthetic_smoke::generators::{
                multimodal_calibration_scenario, multimodal_replicate_scenario,
                MultimodalCalibrationLayout, MultimodalScenario, CALIBRATION_NULL_MODELS,
            },
        };

        fn run(
            scenario: crate::errors::Result<MultimodalScenario>,
        ) -> Result<MultimodalAnalysisRun, String> {
            let scenario = scenario.map_err(|error| error.to_string())?;
            MultimodalEngine::new(scenario.config)
                .and_then(|engine| engine.analyze_run(&scenario.pre))
                .map_err(|error| error.to_string())
        }

        // The lymphocyte/mmr_abnormal p-value under one null model, as the smoke suite reads it.
        fn p_value(
            run: &Result<MultimodalAnalysisRun, String>,
            null_model: crate::config::NeighborhoodNullModel,
        ) -> Result<Option<f64>, String> {
            let run = run.as_ref().map_err(Clone::clone)?;
            Ok(run
                .null_model_sensitivity
                .iter()
                .find(|result| result.null_model == null_model)
                .and_then(|result| {
                    result
                        .results
                        .iter()
                        .find(|row| row.label_a == "lymphocyte" && row.label_b == "mmr_abnormal")
                })
                .and_then(|row| row.q_value.or(row.p_value)))
        }

        let mut output = super::calibration_output("multimodal.jsonl");
        // (layout, lymphocyte percent of H&E cells, abnormal percent of IHC cells)
        let variants = [
            (MultimodalCalibrationLayout::SparseGrid, 48, 50),
            (MultimodalCalibrationLayout::SparseGrid, 20, 25),
            (MultimodalCalibrationLayout::DenseJitteredGrid, 48, 50),
            (MultimodalCalibrationLayout::DenseJitteredGrid, 20, 25),
        ];
        let null_z = super::bonferroni_z(variants.len() * CALIBRATION_NULL_MODELS.len());
        let mut failures = Vec::new();
        for (index, &(layout, lymphocyte_percent, abnormal_percent)) in variants.iter().enumerate()
        {
            let mut tallies =
                vec![super::CalibrationTally::default(); CALIBRATION_NULL_MODELS.len()];
            for replicate in 0..2_000 {
                let outcome = run(multimodal_calibration_scenario(
                    layout,
                    lymphocyte_percent,
                    abnormal_percent,
                    index as u64,
                    replicate,
                ));
                for (tally, null_model) in tallies.iter_mut().zip(CALIBRATION_NULL_MODELS) {
                    tally.record(replicate, p_value(&outcome, null_model));
                }
            }
            for (tally, null_model) in tallies.iter().zip(CALIBRATION_NULL_MODELS) {
                let scenario = serde_json::json!({
                    "workflow": "multimodal",
                    "layout": layout.name(),
                    "lymphocyte_percent": lymphocyte_percent,
                    "abnormal_percent": abnormal_percent,
                    "null_model": null_model,
                    "labels": "uniform_fixed_count",
                    "permutations": 99,
                });
                failures.extend(super::judge_scenario(
                    &mut output,
                    scenario,
                    super::ScenarioRole::Null,
                    tally,
                    null_z,
                ));
            }
        }
        // The existing immune-association positive control, read under every null model.
        let mut tallies = vec![super::CalibrationTally::default(); CALIBRATION_NULL_MODELS.len()];
        for replicate in 0..500 {
            let outcome = run(multimodal_replicate_scenario(
                "immune_associated_mmr_territory",
                123,
                0,
                replicate,
            )
            .map(|mut scenario| {
                scenario.config.neighborhood.null_models = CALIBRATION_NULL_MODELS.to_vec();
                scenario
            }));
            for (tally, null_model) in tallies.iter_mut().zip(CALIBRATION_NULL_MODELS) {
                tally.record(replicate, p_value(&outcome, null_model));
            }
        }
        for (tally, null_model) in tallies.iter().zip(CALIBRATION_NULL_MODELS) {
            let scenario = serde_json::json!({
                "workflow": "multimodal",
                "layout": "immune_associated_mmr_territory",
                "null_model": null_model,
                "labels": "associated_lymphocyte_and_abnormal_clusters",
                "permutations": 99,
            });
            failures.extend(super::judge_scenario(
                &mut output,
                scenario,
                super::ScenarioRole::Power,
                tally,
                null_z,
            ));
        }
        assert!(
            failures.is_empty(),
            "multimodal shuffle calibration criteria failed; preserve output and investigate:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn smoke_is_deterministic_and_reports_denominators() {
        let first = crate::synthetic_smoke::run_multimodal_synthetic_smoke(3, 123)
            .expect("first smoke check");
        let repeated = crate::synthetic_smoke::run_multimodal_synthetic_smoke(3, 123)
            .expect("repeated smoke check");

        assert_eq!(first, repeated);
        assert_eq!(first.suite_kind, "smoke");
        assert_eq!(first.seed, 123);
        assert_eq!(first.engine_version, env!("CARGO_PKG_VERSION"));
        for result in first.results.values() {
            assert_eq!(result.replicates_attempted, 3);
            assert_eq!(result.replicates_completed, 3);
            assert_eq!(result.replicates_failed, 0);
            assert!(result.failure_reasons.is_empty());
            assert!(!result.acceptance_criterion.is_empty());
            assert_eq!(result.criterion_met_rate, Some(1.0));
            assert!(result.criterion_met_confidence_interval.is_some());
        }

        let first_json = serde_json::to_value(&first).expect("summary json");
        assert!(first_json["immune_associated_mmr_territory"]
            .get("below_resolution_flag_rate")
            .is_none());
        assert!(
            first_json["registration_jitter"]["below_registration_resolution_flag_rate"]
                .is_number()
        );
        assert!(first_json["registration_jitter"]
            ["below_registration_resolution_confidence_interval"]
            .is_object());
    }

    #[test]
    fn failed_replicates_are_reported_and_fail_the_smoke_scenario() {
        let scenario = crate::synthetic_smoke::generators::multimodal_replicate_scenario(
            "two_unrelated_mmr_territories",
            123,
            1,
            0,
        )
        .expect("scenario");
        let result = crate::synthetic_smoke::summarize_multimodal_outcomes(
            "two_unrelated_mmr_territories",
            2,
            crate::synthetic_smoke::multimodal_scenario_configuration(&scenario),
            [
                Ok(crate::synthetic_smoke::ObservedMultimodalOutcome::default()),
                Err((
                    1,
                    crate::MarklabError::Validation("synthetic execution failure".into()),
                )),
            ],
        )
        .expect("summarize outcomes");

        assert_eq!(result.replicates_attempted, 2);
        assert_eq!(result.replicates_completed, 1);
        assert_eq!(result.replicates_failed, 1);
        assert_eq!(result.failure_reasons.len(), 1);
        assert!(result.failure_reasons[0].contains("replicate 1"));
        assert!(!result.passed);
    }
}

#[test]
fn remediation_multimodal_validation_calls_the_public_engine() {
    reset_multimodal_analysis_call_count();

    crate::synthetic_smoke::run_multimodal_synthetic_smoke(1, 123)
        .expect("multimodal synthetic generator smoke check");

    assert!(
        multimodal_analysis_call_count() >= 24,
        "each scenario input must invoke MultimodalEngine; observed {} calls",
        multimodal_analysis_call_count()
    );
}

#[test]
fn pattern_rejects_nonfinite_coordinates_and_nonbinary_marks() {
    let nonfinite = Pattern::from_arrays(vec![0.0, f64::NAN], vec![0.0, 1.0], vec![1, 0], meta());
    assert!(nonfinite.is_err());

    let nonbinary = Pattern::from_arrays(vec![0.0, 1.0], vec![0.0, 1.0], vec![1, 2], meta());
    assert!(nonbinary.is_err());
}

#[test]
fn validation_flags_underpowered_and_invalid_mask_cases() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 5;
    config.validation.n_marked_min = 2;
    config.validation.n_unmarked_min = 2;
    config.validation.valid_mask_fraction_min = 0.9;

    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0],
        vec![0.0, 0.0, 0.0],
        vec![1, 0, 0],
        meta(),
    )
    .expect("pattern");
    pattern.window.valid_mask_fraction = 0.25;

    let flags = validation_flags(&pattern, &config);

    assert!(flags.contains(&StatusFlag::UnderpoweredTooFewCells));
    assert!(flags.contains(&StatusFlag::UnderpoweredTooFewMarked));
    assert!(!flags.contains(&StatusFlag::UnderpoweredTooFewUnmarked));
    assert!(flags.contains(&StatusFlag::InvalidIhcMask));
}

#[test]
fn validation_flags_area_and_resolvable_k_shell_failures() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 100.0;
    config.validation.k_shell_min = 5;
    config.spectrum.k_shells = 8;

    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1, 0, 1, 0],
        meta(),
    )
    .expect("pattern");
    pattern.window.area_um2 = 25.0;
    pattern.window.analysis_effective_length_um = 4.0;
    pattern.window.d_nn_mean_um = 4.0;

    let flags = validation_flags(&pattern, &config);

    assert!(flags.contains(&StatusFlag::UnderpoweredAreaTooSmall));
    assert!(flags.contains(&StatusFlag::UnderpoweredTooFewKShells));
}

#[test]
fn engine_suppresses_strong_interpretation_when_validation_flags_are_present() {
    let mut config = AnalysisConfig::default();
    config.permutation.stratified = false;
    config.validation.n_min = 5;
    config.validation.n_marked_min = 2;
    config.validation.n_unmarked_min = 2;

    let engine = AnalysisEngine::new(config).expect("engine");
    let pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0],
        vec![0.0, 0.0, 0.0],
        vec![1, 0, 0],
        meta(),
    )
    .expect("pattern");

    let result = engine.analyze_pattern(&pattern).expect("analysis");

    assert_eq!(result.status, AnalysisStatus::Suppressed);
    assert!(result
        .status_flags
        .contains(&StatusFlag::UnderpoweredTooFewCells));
    assert!(result
        .interpretation
        .text
        .contains("suppressed by validation status"));
}

#[test]
fn validation_flags_extreme_prevalence_even_when_counts_pass() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 10;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.p_min = 0.20;
    config.validation.p_max = 0.80;

    let low_prevalence = Pattern::from_arrays(
        (0..10).map(|value| value as f64).collect(),
        vec![0.0; 10],
        vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        meta(),
    )
    .expect("low prevalence pattern");

    let high_prevalence = Pattern::from_arrays(
        (0..10).map(|value| value as f64).collect(),
        vec![0.0; 10],
        vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 0],
        meta(),
    )
    .expect("high prevalence pattern");

    assert_eq!(
        validation_flags(&low_prevalence, &config),
        vec![StatusFlag::SensitivityUnstable]
    );
    assert_eq!(
        validation_flags(&high_prevalence, &config),
        vec![StatusFlag::SensitivityUnstable]
    );
}

#[test]
fn csv_loader_preserves_optional_qc_bin_for_stratified_permutations() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,qc_bin,mark_probability\n\
0.0,0.0,1,case_001,post,MSH6,true,true,10,0.90\n\
1.0,0.0,0,case_001,post,MSH6,true,true,10,0.20\n\
2.0,0.0,1,case_001,post,MSH6,true,true,20,0.75\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[3,-1],[3,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;

    assert_eq!(pattern.qc_bin.as_deref(), Some(&[10, 10, 20][..]));
    assert_eq!(pattern.mark_prob.as_deref(), Some(&[0.90, 0.20, 0.75][..]));
}

#[test]
fn csv_loader_encodes_allowed_string_strata_for_stratified_permutations() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,internal_control_local,block_id,slide_region,histologic_compartment,stain_batch\n\
0.0,0.0,1,case_001,post,MSH6,true,true,valid,block_a,region_left,tumor,batch_a\n\
1.0,0.0,0,case_001,post,MSH6,true,true,valid,block_a,region_left,tumor,batch_a\n\
2.0,0.0,1,case_001,post,MSH6,true,true,valid,block_b,region_right,stroma,batch_b\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[3,-1],[3,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;
    let strata = &pattern.categorical_strata;

    for field in [
        "internal_control_bin",
        "block_id",
        "slide_region",
        "histologic_compartment",
        "stain_batch",
    ] {
        assert!(strata.contains_key(field), "missing {field}");
        assert_eq!(strata[field].len(), pattern.len());
        assert_eq!(strata[field][0], strata[field][1]);
    }
    assert_ne!(strata["block_id"][0], strata["block_id"][2]);
    assert_ne!(strata["slide_region"][0], strata["slide_region"][2]);
    assert_ne!(
        strata["histologic_compartment"][0],
        strata["histologic_compartment"][2]
    );
    assert_ne!(strata["stain_batch"][0], strata["stain_batch"][2]);
}

#[test]
fn csv_loader_preserves_tumor_probability_and_nucleus_area_metrics() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,tumor_probability,nucleus_area_um2\n\
0.0,0.0,1,case_001,post,MSH6,true,true,0.95,42.0\n\
1.0,0.0,0,case_001,post,MSH6,true,true,0.80,38.5\n\
2.0,0.0,1,case_001,post,MSH6,true,true,0.70,44.5\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[3,-1],[3,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;

    assert_eq!(
        pattern.tumor_probability.as_deref(),
        Some(&[0.95, 0.80, 0.70][..])
    );
    assert_eq!(
        pattern.nucleus_area_um2.as_deref(),
        Some(&[42.0, 38.5, 44.5][..])
    );
}

#[test]
fn csv_loader_rejects_partially_populated_dense_optional_metrics() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("partial_metric.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,local_dab_od\n\
0.0,0.0,1,case_001,post,MSH6,true,true,0.25\n\
1.0,0.0,0,case_001,post,MSH6,true,true,\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[2,-1],[2,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let error = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect_err("partially populated metrics must not receive fabricated values");

    assert!(error.to_string().contains("local_dab_od"));
    assert!(error.to_string().contains("every retained row or none"));
}

#[test]
fn metadata_mismatch_rejected() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("metadata_mismatch.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_002,post,MSH6,true,true\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[2,-1],[2,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let error = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect_err("mixed case metadata must be rejected");

    assert!(error
        .to_string()
        .contains("one case_id/timepoint/protein group"));
}

#[test]
fn csv_loader_rejects_invalid_tumor_probability_and_nucleus_area_metrics() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[2,-1],[2,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let invalid_probability = dir.path().join("invalid_probability.csv");
    fs::write(
        &invalid_probability,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,tumor_probability,nucleus_area_um2\n\
0.0,0.0,1,case_001,post,MSH6,true,true,1.20,42.0\n",
    )
    .expect("write invalid probability cells");
    let probability_error = load_pattern_csv_with_diagnostics(&invalid_probability, &mask)
        .expect_err("invalid tumor_probability should be rejected");
    assert!(probability_error.to_string().contains("tumor_probability"));

    let invalid_nucleus_area = dir.path().join("invalid_nucleus_area.csv");
    fs::write(
        &invalid_nucleus_area,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,tumor_probability,nucleus_area_um2\n\
0.0,0.0,1,case_001,post,MSH6,true,true,0.80,-1.0\n",
    )
    .expect("write invalid nucleus area cells");
    let area_error = load_pattern_csv_with_diagnostics(&invalid_nucleus_area, &mask)
        .expect_err("invalid nucleus_area_um2 should be rejected");
    assert!(area_error.to_string().contains("nucleus_area_um2"));
}

#[test]
fn csv_loader_uses_internal_control_local_as_validity_mask() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,internal_control_local\n\
0.0,0.0,1,case_001,post,MSH6,true,true,valid\n\
1.0,0.0,0,case_001,post,MSH6,true,true,valid\n\
2.0,0.0,1,case_001,post,MSH6,true,true,absent\n\
3.0,0.0,0,case_001,post,MSH6,true,true,unknown\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;

    assert_eq!(pattern.len(), 2);
    assert_eq!(pattern.n_marked(), 1);
    assert_eq!(pattern.window.valid_mask_fraction, 0.5);
    assert_eq!(pattern.valid_tumor_fraction, Some(1.0));
    assert_eq!(pattern.valid_ihc_fraction, Some(1.0));
    assert_eq!(pattern.internal_control_valid_fraction, Some(0.5));
}

#[test]
fn remediation_internal_control_fraction_is_not_final_retained_fraction() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,internal_control_local,artifact\n\
0.0,0.0,1,case_001,post,MSH6,true,true,valid,false\n\
1.0,0.0,0,case_001,post,MSH6,true,true,valid,false\n\
2.0,0.0,1,case_001,post,MSH6,true,true,absent,false\n\
3.0,0.0,0,case_001,post,MSH6,true,true,valid,true\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;

    assert_eq!(pattern.len(), 2);
    assert!((pattern.window.valid_mask_fraction - 0.5).abs() < 1.0e-12);
    assert_eq!(pattern.internal_control_valid_fraction, Some(0.75));
}

#[test]
fn csv_loader_tracks_each_qc_fraction_against_all_in_mask_cells() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,internal_control_local,artifact,necrosis\n\
0.0,0.0,1,case_001,post,MSH6,true,true,valid,false,false\n\
1.0,0.0,0,case_001,post,MSH6,true,true,valid,false,false\n\
2.0,0.0,1,case_001,post,MSH6,false,true,valid,false,false\n\
3.0,0.0,0,case_001,post,MSH6,true,false,valid,false,false\n\
4.0,0.0,1,case_001,post,MSH6,true,true,absent,false,false\n\
5.0,0.0,0,case_001,post,MSH6,true,true,valid,true,false\n\
6.0,0.0,1,case_001,post,MSH6,true,true,valid,false,true\n\
7.0,0.0,0,case_001,post,MSH6,false,false,absent,true,true\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[8,-1],[8,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;

    assert_eq!(pattern.len(), 2);
    assert_eq!(pattern.valid_tumor_fraction, Some(6.0 / 8.0));
    assert_eq!(pattern.valid_ihc_fraction, Some(6.0 / 8.0));
    assert_eq!(pattern.internal_control_valid_fraction, Some(6.0 / 8.0));
    assert_eq!(pattern.artifact_excluded_fraction, Some(2.0 / 8.0));
    assert_eq!(pattern.nonviable_excluded_fraction, Some(2.0 / 8.0));
    assert_eq!(pattern.window.valid_mask_fraction, 2.0 / 8.0);
}

#[test]
fn csv_loader_rejects_an_empty_in_mask_qc_denominator() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
10.0,10.0,1,case_001,post,MSH6,true,true\n\
11.0,10.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[1,-1],[1,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let error = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect_err("a zero in-mask denominator must not produce numeric QC fractions");

    assert!(error
        .to_string()
        .contains("no cells fell inside the tumor mask"));
}

#[test]
fn csv_loader_excludes_artifact_and_nonviable_rows_from_analysis_window() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,artifact,edge_artifact,fold_artifact,necrosis,nonviable_therapy_effect\n\
0.0,0.0,1,case_001,post,MSH6,true,true,false,false,false,false,false\n\
1.0,0.0,0,case_001,post,MSH6,true,true,true,false,false,false,false\n\
2.0,0.0,1,case_001,post,MSH6,true,true,false,true,false,false,false\n\
3.0,0.0,0,case_001,post,MSH6,true,true,false,false,true,false,false\n\
4.0,0.0,1,case_001,post,MSH6,true,true,false,false,false,true,false\n\
5.0,0.0,0,case_001,post,MSH6,true,true,false,false,false,false,true\n\
6.0,0.0,0,case_001,post,MSH6,true,true,false,false,false,false,false\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[7,-1],[7,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let mut pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;

    assert_eq!(pattern.mark.as_ref(), &[1, 0]);
    assert!((pattern.window.valid_mask_fraction - (2.0 / 7.0)).abs() < 1e-12);
    assert_eq!(pattern.valid_tumor_fraction, Some(1.0));
    assert_eq!(pattern.valid_ihc_fraction, Some(1.0));
    assert_eq!(pattern.artifact_excluded_fraction, Some(3.0 / 7.0));
    assert_eq!(pattern.nonviable_excluded_fraction, Some(2.0 / 7.0));

    pattern.window.area_um2 = 7.0;
    pattern.window.analysis_effective_length_um = 7.0;
    pattern.window.d_nn_mean_um = 1.0;
    let mut config = AnalysisConfig::default();
    config.permutation.stratified = false;
    config.validation.n_min = 2;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.valid_mask_fraction_min = 0.1;
    config.spectrum.k_shells = 8;
    config.permutation.b = 9;
    config.inference.family_wise_alpha = 0.25;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert_eq!(result.qc.artifact_excluded_fraction, Some(3.0 / 7.0));
    assert_eq!(result.qc.nonviable_excluded_fraction, Some(2.0 / 7.0));
}

#[test]
fn engine_flags_internal_control_dropout_with_required_suppression_wording() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,internal_control_local\n\
0.0,0.0,1,case_001,post,MSH6,true,true,valid\n\
1.0,0.0,0,case_001,post,MSH6,true,true,weak\n\
2.0,0.0,1,case_001,post,MSH6,true,true,absent\n\
3.0,0.0,0,case_001,post,MSH6,true,true,unknown\n\
4.0,0.0,1,case_001,post,MSH6,true,true,valid\n\
5.0,0.0,0,case_001,post,MSH6,true,true,valid\n",
    )
    .expect("write cells");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[6,-1],[6,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");
    let mut pattern = load_pattern_csv_with_diagnostics(&cells, &mask)
        .expect("load pattern")
        .pattern;
    pattern.window.analysis_effective_length_um = 6.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 6.0;
    let mut config = AnalysisConfig::default();
    config.permutation.stratified = false;
    config.validation.n_min = 3;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.valid_mask_fraction_min = 0.9;
    config.spectrum.k_shells = 8;
    config.permutation.b = 9;
    config.inference.family_wise_alpha = 0.25;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert!(result
        .status_flags
        .contains(&StatusFlag::InternalControlFailureOverlap));
    assert!(result
        .status_flags
        .contains(&StatusFlag::SuppressedBiologicInterpretation));
    assert!(result
        .interpretation
        .text
        .contains("overlaps recorded input-QC artifact structure"));
}

#[test]
fn engine_reports_local_tumor_cellularity_metrics() {
    let mut config = AnalysisConfig::default();
    config.permutation.stratified = false;
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.spectrum.k_shells = 8;
    config.permutation.b = 9;
    config.inference.family_wise_alpha = 0.25;

    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1, 0, 1, 0],
        meta(),
    )
    .expect("pattern");
    pattern.window.area_um2 = 2_000_000.0;
    pattern.window.analysis_effective_length_um = 4.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.valid_mask_fraction = 0.75;
    pattern.valid_tumor_fraction = Some(0.90);
    pattern.valid_ihc_fraction = Some(0.85);
    pattern.internal_control_valid_fraction = Some(0.80);
    pattern.tumor_probability = Some(vec![0.90, 0.80, 0.70, 0.60].into_boxed_slice());
    pattern.nucleus_area_um2 = Some(vec![40.0, 42.0, 44.0, 46.0].into_boxed_slice());

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert_eq!(result.qc.valid_mask_fraction, 0.75);
    assert_eq!(result.qc.valid_tumor_fraction, Some(0.90));
    assert_eq!(result.qc.valid_ihc_fraction, Some(0.85));
    assert_eq!(result.qc.internal_control_valid_fraction, Some(0.80));
    assert_eq!(result.qc.mean_tumor_probability, Some(0.75));
    assert_eq!(result.qc.mean_nucleus_area_um2, Some(43.0));
    assert_eq!(result.qc.tumor_cell_density_per_mm2, Some(2.0));
}

#[test]
fn engine_suppresses_interpretation_when_stain_gradient_is_detected() {
    let mut config = AnalysisConfig::default();
    config.permutation.stratified = false;
    config.validation.n_min = 8;
    config.validation.n_marked_min = 2;
    config.validation.n_unmarked_min = 2;
    config.validation.area_min_um2 = 1.0;
    config.spectrum.k_shells = 8;
    config.permutation.b = 9;
    config.inference.family_wise_alpha = 0.25;

    let mut pattern = Pattern::from_arrays(
        (0..20).map(|value| value as f64).collect(),
        vec![0.0; 20],
        (0..20).map(|index| u8::from(index < 4)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.analysis_effective_length_um = 20.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 20.0;
    pattern.local_dab_od = Some(
        (0..20)
            .map(|index| index as f32 / 20.0)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert!(result
        .status_flags
        .contains(&StatusFlag::StainGradientSuspect));
    assert!(result
        .status_flags
        .contains(&StatusFlag::SuppressedBiologicInterpretation));
    assert_eq!(result.status, AnalysisStatus::Suppressed);
}

#[test]
fn engine_flags_fragmented_component_layouts() {
    let mut config = AnalysisConfig::default();
    config.permutation.stratified = false;
    config.validation.n_min = 20;
    config.validation.n_marked_min = 2;
    config.validation.n_unmarked_min = 2;
    config.validation.area_min_um2 = 1.0;

    let mut pattern = Pattern::from_arrays(
        (0..30).map(|value| value as f64).collect(),
        vec![0.0; 30],
        (0..30).map(|index| u8::from(index % 7 == 0)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.analysis_effective_length_um = 6.0;
    pattern.window.d_nn_mean_um = 2.0;
    pattern.window.area_um2 = 30.0;
    pattern.component_id = Some(
        (0..30)
            .map(|index| index as u32)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert!(result
        .status_flags
        .contains(&StatusFlag::MaskFragmentationSuspect));
    assert_eq!(result.status, AnalysisStatus::Suppressed);
}
