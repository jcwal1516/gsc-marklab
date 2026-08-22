use std::fs;

use crate::{
    data::{validate::validation_flags, PatternMeta},
    geom::mask::TumorMask,
    io::csv::load_pattern_csv_with_diagnostics,
    multimodal::{multimodal_analysis_call_count, reset_multimodal_analysis_call_count},
    validation::run_synthetic_validation,
    AnalysisConfig, AnalysisEngine, Pattern, StatusFlag,
};

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

[wavelet]
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
fn synthetic_validation_controls_random_labeling_and_detects_anisotropy() {
    let summary = run_synthetic_validation(100).expect("synthetic validation");
    let random = &summary.results["random_labeling"];
    let stripe = &summary.results["anisotropic_stripe"];

    assert!(
        random.passed,
        "random-labeling calibration failed: {random:?}"
    );
    assert!(
        stripe.passed,
        "anisotropic stripe detection failed: {stripe:?}"
    );
}

mod multimodal {
    use assert_cmd::Command;

    #[test]
    fn validation_detects_immune_associated_mmr_territory() {
        let summary =
            crate::validation::run_multimodal_synthetic_validation(100, 123).expect("validation");
        assert!(
            summary.results["immune_associated_mmr_territory"]
                .detection_rate
                .expect("rate")
                > 0.70
        );
    }

    #[test]
    fn validation_flags_below_registration_resolution_associations() {
        let summary =
            crate::validation::run_multimodal_synthetic_validation(25, 456).expect("validation");
        assert!(
            summary.results["registration_jitter"]
                .below_resolution_flag_rate
                .expect("rate")
                > 0.80
        );
    }

    #[test]
    fn validation_rates_reflect_seeded_replicate_variation() {
        let seed_123 = crate::validation::run_multimodal_synthetic_validation(100, 123)
            .expect("seed 123 validation");
        let seed_124 = crate::validation::run_multimodal_synthetic_validation(100, 124)
            .expect("seed 124 validation");

        let immune_detection = seed_123.results["immune_associated_mmr_territory"]
            .detection_rate
            .expect("immune detection rate");
        assert!(
            immune_detection > 0.70 && immune_detection < 1.0,
            "immune-associated detection should pass without being degenerate: {immune_detection}"
        );

        let jitter_below_resolution = seed_123.results["registration_jitter"]
            .below_resolution_flag_rate
            .expect("jitter below-resolution rate");
        assert!(
            jitter_below_resolution > 0.80 && jitter_below_resolution < 1.0,
            "registration jitter flag rate should pass without being degenerate: {jitter_below_resolution}"
        );

        let rate_accessors = [
            rate_pair("two_unrelated_mmr_territories", |result| {
                result.false_positive_rate
            }),
            rate_pair("two_related_mmr_territories", |result| {
                result.detection_rate
            }),
            rate_pair("immune_associated_mmr_territory", |result| {
                result.detection_rate
            }),
            rate_pair("registration_jitter", |result| {
                result.below_resolution_flag_rate
            }),
            rate_pair("prepost_equivalent_spatial_pattern", |result| {
                result.equivalence_rate
            }),
            rate_pair("prepost_changed_spatial_pattern", |result| {
                result.detection_rate
            }),
        ];
        let any_rate_changed = rate_accessors.iter().any(|accessor| {
            let left = (accessor.rate)(&seed_123.results[accessor.generator]).expect("left rate");
            let right = (accessor.rate)(&seed_124.results[accessor.generator]).expect("right rate");
            (left - right).abs() > f64::EPSILON
        });
        assert!(
            any_rate_changed,
            "rates should vary across deterministic seeds"
        );

        let seed_123_json = serde_json::to_value(&seed_123).expect("summary json");
        assert!(
            seed_123_json["immune_associated_mmr_territory"]["below_resolution_flag_rate"]
                .is_null()
        );
        assert_eq!(
            seed_123_json["registration_jitter"]["below_resolution_flag_rate"],
            seed_123_json["registration_jitter"]["below_registration_resolution_flag_rate"]
        );
    }

    struct RateAccessor {
        generator: &'static str,
        rate: fn(&crate::validation::MultimodalSyntheticGeneratorResult) -> Option<f64>,
    }

    fn rate_pair(
        generator: &'static str,
        rate: fn(&crate::validation::MultimodalSyntheticGeneratorResult) -> Option<f64>,
    ) -> RateAccessor {
        RateAccessor { generator, rate }
    }

    #[test]
    fn validate_cli_writes_multimodal_validation_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("validation-multimodal");

        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "validate",
                "--suite",
                "multimodal",
                "--replicates",
                "25",
                "--out",
                out.to_str().expect("out path"),
            ])
            .assert()
            .success();

        let summary: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(out.join("validation.json")).expect("validation json"),
        )
        .expect("json");
        assert_eq!(
            summary["immune_associated_mmr_territory"]["passed"].as_bool(),
            Some(true)
        );
        assert_eq!(
            summary["registration_jitter"]["passed"].as_bool(),
            Some(true)
        );
    }
}

#[test]
#[ignore = "Phase 0 reproduction: COR-01 production-pipeline validation is fixed in Phase 9"]
fn remediation_multimodal_validation_calls_the_public_engine() {
    reset_multimodal_analysis_call_count();

    crate::validation::run_multimodal_synthetic_validation(1, 123).expect("multimodal validation");

    assert!(
        multimodal_analysis_call_count() >= 6,
        "each scenario replicate must invoke MultimodalEngine; observed {} calls",
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
    pattern.window.l_eff_um = 4.0;
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

    assert_eq!(result.status, "suppressed");
    assert!(result
        .status_flags
        .contains(&StatusFlag::UnderpoweredTooFewCells));
    assert!(result
        .interpretation
        .text
        .contains("not a clonality result"));
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
    pattern.window.l_eff_um = 7.0;
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
    pattern.window.l_eff_um = 6.0;
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
        .contains("overlaps IHC/QC artifact structure"));
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
    pattern.window.l_eff_um = 4.0;
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
    pattern.window.l_eff_um = 20.0;
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
    assert_eq!(result.status, "suppressed");
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
    pattern.window.l_eff_um = 6.0;
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
    assert_eq!(result.status, "suppressed");
}
