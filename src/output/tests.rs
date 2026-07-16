use std::fs;

use crate::{
    data::PatternMeta, io::report::render_analysis_report, prepost::deltas::compare_prepost,
    AnalysisConfig, AnalysisEngine, CurveTestResult, OutputWriter, Pattern, ResultDocument,
    StatusFlag,
};
#[cfg(feature = "parquet")]
use crate::{
    multimodal::cell_table::{CellSection, FusedCell},
    AnalysisSection, CrossInteractionCurve, FusedCellSummary, Interpretation, MultimodalResult,
    NeighborhoodEnrichmentResult, PairCorrelationPoint, RegistrationSummary,
};
use serde_json::Value;

fn pattern(case_id: &str, timepoint: &str, marks: Vec<u8>) -> Pattern {
    let mut pattern = Pattern::from_arrays(
        (0..marks.len()).map(|value| value as f64).collect(),
        vec![0.0; marks.len()],
        marks,
        PatternMeta {
            case_id: case_id.into(),
            timepoint: timepoint.into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    pattern.qc_bin = Some(vec![0; pattern.len()].into_boxed_slice());
    pattern.component_id = Some(vec![0; pattern.len()].into_boxed_slice());
    pattern
}

#[test]
fn output_writer_emits_result_manifest_qc_and_timings_json() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.output.write_run_manifest = true;
    config.output.write_parquet_curves = cfg!(feature = "parquet");

    let engine = AnalysisEngine::new(config.clone()).expect("engine");
    let result = engine
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");
    let dir = tempfile::tempdir().expect("temp dir");

    OutputWriter::write(
        &ResultDocument::marked(result.clone()),
        dir.path(),
        &config.output,
    )
    .expect("write outputs");

    assert!(dir.path().join("result.json").exists());
    assert!(dir.path().join("run_manifest.json").exists());
    assert!(dir.path().join("qc.json").exists());
    assert_eq!(
        dir.path().join("spectra.parquet").exists(),
        cfg!(feature = "parquet") && !result.spectrum_curve.is_empty()
    );
    assert_eq!(
        dir.path().join("scalogram.parquet").exists(),
        cfg!(feature = "parquet") && !result.scalogram_curve.is_empty()
    );
    assert_eq!(
        dir.path().join("pair_correlation.parquet").exists(),
        cfg!(feature = "parquet") && !result.pair_correlation_curve.is_empty()
    );
    assert_eq!(
        dir.path().join("wavelet_territories.geojson").exists(),
        result
            .wavelet_territories
            .value()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(
        dir.path().join("figures").join("spectrum.svg").exists(),
        !result.spectrum_curve.is_empty()
    );
    assert!(dir.path().join("timings.json").exists());
    assert!(dir.path().join("report.md").exists());
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("run_manifest.json")).expect("manifest"),
    )
    .expect("manifest json");
    assert_eq!(manifest["program"], "mmrspace");
    assert_eq!(manifest["result"]["case_id"], "case_001");
    assert_eq!(manifest["result"]["timepoint"], "post");
    assert_eq!(manifest["result"]["n_cells"], 4);
    assert_eq!(manifest["result"]["n_marked"], 2);
    assert_eq!(manifest["output"]["write_run_manifest"], true);
    assert_eq!(
        manifest["output"]["write_parquet_curves"],
        cfg!(feature = "parquet")
    );
    assert!(
        manifest["timings_stage_count"]
            .as_u64()
            .expect("stage count")
            >= 1
    );

    assert_eq!(
        dir.path().join("figures").join("anisotropy.svg").exists(),
        result.anisotropy.value().is_some()
    );
    if result.anisotropy.value().is_some() {
        let anisotropy_svg = fs::read_to_string(dir.path().join("figures").join("anisotropy.svg"))
            .expect("anisotropy svg");
        assert!(anisotropy_svg.contains("anisotropy-index"));
    }
    if !result.spectrum_curve.is_empty() {
        let spectrum_svg = fs::read_to_string(dir.path().join("figures").join("spectrum.svg"))
            .expect("spectrum svg");
        assert!(spectrum_svg.contains("<polyline"));
        assert!(spectrum_svg.contains("low-k excess"));
    }
    if !result.scalogram_curve.is_empty() {
        let scalogram_svg = fs::read_to_string(dir.path().join("figures").join("scalogram.svg"))
            .expect("scalogram svg");
        assert!(scalogram_svg.contains("fine"));
        assert!(scalogram_svg.contains("coarse"));
    }

    let result_json = fs::read_to_string(dir.path().join("result.json")).expect("result");
    assert!(result_json.contains("\"program\": \"mmrspace\""));
    let report = fs::read_to_string(dir.path().join("report.md")).expect("report");
    let report_lower = report.to_lowercase();
    assert!(report_lower.contains("low-k excess"));
    assert!(report_lower.contains("section-level spatial organization"));
    assert!(!report_lower.contains("same cells"));
    assert!(!report_lower.contains("directional growth"));

    let timings: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("timings.json")).expect("timings"),
    )
    .expect("timings json");
    let stages = timings["stages"].as_array().expect("timing stages");
    assert!(!stages.is_empty());
    assert!(stages.iter().any(|stage| stage["stage_name"] == "validate"));
    assert!(stages
        .iter()
        .any(|stage| stage["stage_name"] == "write_outputs"));
    for stage in stages {
        assert!(stage["wall_ms"].as_f64().expect("wall_ms") >= 0.0);
        assert!(stage["cpu_threads"].as_u64().expect("cpu_threads") >= 1);
        assert_eq!(stage["n_cells"], 4);
        assert_eq!(stage["n_marked"], 2);
        assert_eq!(
            stage["n_k_modes"].as_u64().expect("n_k_modes"),
            result.spectrum.value().map_or(0, |value| value.n_k_modes) as u64
        );
        assert!(stage["n_permutations"].as_u64().expect("n_permutations") <= 999);
        assert!(
            stage["estimated_peak_memory_mib"]
                .as_f64()
                .expect("estimated memory")
                > 0.0
        );
    }
}

#[cfg(not(feature = "parquet"))]
#[test]
fn output_writer_errors_when_parquet_curves_requested_without_parquet_feature() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.permutation.b = 3;

    let engine = AnalysisEngine::new(config.clone()).expect("engine");
    let result = engine
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");
    let dir = tempfile::tempdir().expect("temp dir");

    let err = OutputWriter::write(&ResultDocument::marked(result), dir.path(), &config.output)
        .expect_err("requested parquet curves should require the parquet feature");

    assert!(err.to_string().contains("Parquet curve output"));
}

#[test]
fn output_writer_respects_optional_artifact_flags() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.output.write_parquet_curves = false;
    config.output.write_geojson_territories = false;
    config.output.write_figures = false;
    config.output.write_run_manifest = false;

    let engine = AnalysisEngine::new(config.clone()).expect("engine");
    let result = engine
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");
    let dir = tempfile::tempdir().expect("temp dir");

    OutputWriter::write(&ResultDocument::marked(result), dir.path(), &config.output)
        .expect("write outputs");

    assert!(dir.path().join("result.json").exists());
    assert!(dir.path().join("qc.json").exists());
    assert!(dir.path().join("timings.json").exists());
    assert!(dir.path().join("report.md").exists());
    assert!(!dir.path().join("run_manifest.json").exists());
    assert!(!dir.path().join("spectra.parquet").exists());
    assert!(!dir.path().join("scalogram.parquet").exists());
    assert!(!dir.path().join("pair_correlation.parquet").exists());
    assert!(!dir.path().join("territories.geojson").exists());
    assert!(!dir.path().join("figures").exists());
}

#[test]
fn output_writer_rejects_an_unserializable_document_before_writing_artifacts() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 4;
    config.spectrum.low_k_shells = 1;
    config.spectrum.anisotropy_low_k_shells = 1;
    config.permutation.b = 7;
    config.permutation.stratified = false;
    config.inference.family_wise_alpha = 0.25;
    config.periodogram.enabled = false;
    config.wavelet.enabled = false;
    config.output.write_parquet_curves = false;
    config.output.write_geojson_territories = false;
    config.output.write_figures = false;
    config.output.write_run_manifest = false;

    let mut result = AnalysisEngine::new(config.clone())
        .expect("engine")
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");
    result.p_hat = f64::NAN;
    let dir = tempfile::tempdir().expect("temp dir");

    OutputWriter::write(&ResultDocument::marked(result), dir.path(), &config.output)
        .expect_err("non-finite result values must be rejected");

    assert!(fs::read_dir(dir.path())
        .expect("output directory")
        .next()
        .is_none());
}

#[test]
fn territories_geojson_writes_polygon_features_with_required_properties() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 20;
    config.validation.n_marked_min = 4;
    config.validation.n_unmarked_min = 4;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 8;
    config.spectrum.low_k_shells = 2;
    config.permutation.b = 19;
    config.inference.family_wise_alpha = 0.25;
    config.output.write_parquet_curves = false;

    let mut clustered = pattern(
        "case_territory",
        "post",
        (0..40).map(|index| u8::from(index < 8)).collect(),
    );
    clustered.window.l_eff_um = 40.0;
    clustered.window.d_nn_mean_um = 1.0;
    clustered.window.area_um2 = 40.0;

    let result = AnalysisEngine::new(config.clone())
        .expect("engine")
        .analyze_pattern(&clustered)
        .expect("analysis");
    let territories = result
        .wavelet_territories
        .value()
        .expect("wavelet territories")
        .clone();
    assert!(!territories.is_empty());

    let dir = tempfile::tempdir().expect("temp dir");
    OutputWriter::write(&ResultDocument::marked(result), dir.path(), &config.output)
        .expect("write outputs");
    let geojson: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("wavelet_territories.geojson")).expect("territories"),
    )
    .expect("geojson");
    let feature = &geojson["features"][0];

    assert_eq!(feature["geometry"]["type"], "Polygon");
    assert_eq!(
        feature["properties"]["center_x_um"],
        territories[0].center_x_um
    );
    assert_eq!(
        feature["properties"]["center_y_um"],
        territories[0].center_y_um
    );
    assert_eq!(feature["properties"]["radius_um"], territories[0].radius_um);
    assert!(
        feature["geometry"]["coordinates"][0]
            .as_array()
            .expect("polygon ring")
            .len()
            >= 17
    );
}

#[test]
fn prepost_interpretation_uses_allowed_descriptive_language_only() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    let engine = AnalysisEngine::new(config).expect("engine");
    let pre = engine
        .analyze_pattern(&pattern("case_001", "pre", vec![1, 0, 0, 0]))
        .expect("pre");
    let post = engine
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("post");

    let delta = compare_prepost(&pre, &post);
    let text = delta.interpretation_text.to_lowercase();

    assert!(delta
        .curve_tests
        .iter()
        .any(|test| test.comparison_name == "spectrum"));
    assert!(delta.curve_tests.iter().any(|test| test
        .interpretation
        .contains("non-confirmatory without a prespecified margin")));
    let delta_json = serde_json::to_value(&delta).expect("delta json");
    assert!(delta_json["curve_tests"].is_array());

    assert!(text.contains("coarse-scale spatial organization"));
    assert!(!text.contains("same cells"));
    assert!(!text.contains("clone"));
    assert!(!text.contains("directional growth"));
    assert!(!text.contains("gain"));
    assert!(!text.contains("loss"));
}

#[test]
fn report_explains_difference_and_equivalence_tests_when_curve_tests_exist() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.output.write_parquet_curves = false;
    let engine = AnalysisEngine::new(config).expect("engine");
    let mut result = engine
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");

    result.prepost_curve_tests.push(CurveTestResult {
        comparison_name: "spectrum".into(),
        metric: "max_abs_standardized_difference".into(),
        statistic: 0.1,
        p_difference: Some(0.6),
        equivalence_margin: None,
        p_equivalence: None,
        equivalent: None,
        interpretation: "nonsignificant diagnostic".into(),
    });

    let report = render_analysis_report(&result);

    assert!(report.contains("Difference tests assess detectable change"));
    assert!(
        report.contains("Equivalence tests assess same-enough behavior within configured margins")
    );
    assert!(report.contains("nonsignificant difference test is not interpreted as sameness"));
}

#[cfg(feature = "parquet")]
#[test]
fn output_writer_emits_optional_multimodal_parquet_artifacts() {
    let mut result = MultimodalResult {
        case_id: "case_001".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
        status: "ok".into(),
        registration: AnalysisSection::available(RegistrationSummary {
            transform_type: "affine".into(),
            landmark_count: 4,
            rmse_um: 1.0,
            median_residual_um: 0.5,
            p95_residual_um: 1.5,
            max_residual_um: 2.0,
            usable_min_distance_um: 3.0,
            status: "ok".into(),
        }),
        fused_cell_summary: AnalysisSection::available(FusedCellSummary {
            n_he_cells: 1,
            n_ihc_cells: 1,
            n_fused_cells: 2,
            registration_error_um: Some(1.0),
        }),
        fused_cells: Vec::new(),
        neighborhood_enrichment: AnalysisSection::available(vec![NeighborhoodEnrichmentResult {
            label_a: "mmr_abnormal".into(),
            label_b: "lymphocyte".into(),
            observed_edges: 2,
            expected_edges: 1.0,
            enrichment_ratio: 2.0,
            z_score: 1.5,
            p_value: Some(0.05),
            q_value: Some(0.1),
        }]),
        cross_interaction_curves: AnalysisSection::available(Vec::new()),
        neighborhood_territories: AnalysisSection::available(Vec::new()),
        territory_profiles: AnalysisSection::available(Vec::new()),
        territory_comparisons: AnalysisSection::InsufficientData {
            reason: "territory-profile comparison has not been computed".into(),
        },
        diagnostics: AnalysisSection::Disabled,
        timings: Vec::new(),
        interpretation: Interpretation {
            class: "multimodal_summary".into(),
            text: "Multimodal registration, fusion, and neighborhood enrichment summary.".into(),
        },
    };
    result.fused_cells = vec![
        FusedCell {
            source_section: CellSection::He,
            source_cell_id: "h1".into(),
            x_um_registered: 0.0,
            y_um_registered: 0.0,
            mmr_mark: None,
            mmr_probability: None,
            cell_type: Some("lymphocyte".into()),
            cell_type_probability: Some(0.9),
            same_section: false,
            registration_error_um: Some(1.0),
            timepoint: "post".into(),
            case_id: "case_001".into(),
            protein: "MSH6".into(),
        },
        FusedCell {
            source_section: CellSection::Ihc,
            source_cell_id: "i1".into(),
            x_um_registered: 5.0,
            y_um_registered: 0.0,
            mmr_mark: Some(1),
            mmr_probability: Some(0.95),
            cell_type: None,
            cell_type_probability: None,
            same_section: false,
            registration_error_um: Some(1.0),
            timepoint: "post".into(),
            case_id: "case_001".into(),
            protein: "MSH6".into(),
        },
    ];
    result.cross_interaction_curves =
        crate::AnalysisSection::available(vec![CrossInteractionCurve {
            label_a: "mmr_abnormal".into(),
            label_b: "lymphocyte".into(),
            points: vec![PairCorrelationPoint {
                r_min_um: 0.0,
                r_max_um: 10.0,
                value: 1.2,
                inference_eligible: true,
                lower_global_envelope: Some(0.8),
                upper_global_envelope: Some(1.6),
                count: 2,
            }],
            p_global: Some(0.2),
        }]);
    let mut options = AnalysisConfig::default().output;
    options.write_parquet_curves = true;
    options.write_geojson_territories = false;
    options.write_figures = false;
    options.write_run_manifest = false;
    let dir = tempfile::tempdir().expect("temp dir");

    OutputWriter::write(&ResultDocument::multimodal(result), dir.path(), &options)
        .expect("write outputs");

    assert!(dir.path().join("fused_cells.parquet").exists());
    assert!(dir.path().join("neighborhood_enrichment.parquet").exists());
    assert!(dir.path().join("cross_interaction_curves.parquet").exists());
}

#[test]
fn prepost_flags_nonmatching_case_or_protein_as_not_anatomically_comparable() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    let engine = AnalysisEngine::new(config).expect("engine");
    let pre = engine
        .analyze_pattern(&pattern("case_001", "pre", vec![1, 0, 0, 0]))
        .expect("pre");
    let post = engine
        .analyze_pattern(&pattern("case_002", "post", vec![1, 0, 1, 0]))
        .expect("post");

    let delta = compare_prepost(&pre, &post);

    assert!(delta
        .status_flags
        .contains(&StatusFlag::PrePostNotAnatomicallyComparable));
    assert!(delta
        .interpretation_text
        .contains("not anatomically comparable"));
}
