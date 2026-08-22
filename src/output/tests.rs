use std::fs;

use crate::{
    data::PatternMeta, io::report::render_analysis_report, prepost::compare_prepost,
    AnalysisConfig, AnalysisEngine, CurveComparisonAvailability, CurveComparisonResult,
    OutputWriter, Pattern, ResultDocument, SpectrumPoint, StatusFlag,
};
#[cfg(feature = "parquet")]
use crate::{
    multimodal::cells::{CellSection, FusedCell},
    AnalysisSection, CrossInteractionCurve, CrossInteractionPoint,
    EnrichmentStatisticUnavailableReason, FusedCellSummary, Interpretation, MultimodalResult,
    NeighborhoodEnrichmentResult, RegistrationSummary,
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
fn result_uses_mark_pair_covariance_schema() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");

    let document = serde_json::to_value(ResultDocument::marked(result)).expect("result json");
    let marked = document["analysis"]["result"]
        .as_object()
        .expect("marked result object");

    assert!(marked.contains_key("mark_pair_covariance"));
    assert!(!marked.contains_key("pair_correlation"));
    assert!(!marked.contains_key("pair_correlation_curve"));
    let point = marked["mark_pair_covariance_curve"]
        .as_array()
        .and_then(|points| points.first())
        .and_then(Value::as_object)
        .expect("mark-pair covariance point");
    assert!(point.contains_key("covariance"));
    assert!(point.contains_key("pair_count"));
    assert!(!point.contains_key("value"));
    assert!(!point.contains_key("count"));
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
        dir.path().join("scale_energy.parquet").exists(),
        cfg!(feature = "parquet") && !result.scale_energy_curve.is_empty()
    );
    assert_eq!(
        dir.path().join("mark_pair_covariance.parquet").exists(),
        cfg!(feature = "parquet") && !result.mark_pair_covariance_curve.is_empty()
    );
    assert_eq!(
        dir.path().join("residual_territories.geojson").exists(),
        result
            .residual_territories
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
    assert_eq!(manifest["program"], "marklab");
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
    if !result.scale_energy_curve.is_empty() {
        let scale_energy_svg =
            fs::read_to_string(dir.path().join("figures").join("scale_energy.svg"))
                .expect("scale_energy svg");
        assert!(scale_energy_svg.contains("local_difference"));
        assert!(scale_energy_svg.contains("block_mean"));
    }

    let result_json = fs::read_to_string(dir.path().join("result.json")).expect("result");
    assert!(result_json.contains("\"program\": \"marklab\""));
    let report = fs::read_to_string(dir.path().join("report.md")).expect("report");
    let report_lower = report.to_lowercase();
    assert!(report_lower.contains("low-k excess"));
    assert!(report_lower.contains("section-level organization"));
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

#[test]
#[ignore = "Phase 0 reproduction: OUT-01 unified telemetry is fixed in Phase 5"]
fn remediation_result_and_timings_sidecar_use_the_same_telemetry() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.permutation.b = 39;
    let result = AnalysisEngine::new(config.clone())
        .expect("engine")
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");
    let dir = tempfile::tempdir().expect("temp dir");

    OutputWriter::write(&ResultDocument::marked(result), dir.path(), &config.output)
        .expect("write outputs");

    let result_document: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("result.json")).expect("result document"),
    )
    .expect("result JSON");
    let timing_sidecar: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("timings.json")).expect("timing sidecar"),
    )
    .expect("timing JSON");

    assert_eq!(
        result_document["analysis"]["result"]["timings"], timing_sidecar["stages"],
        "persisted timing artifacts must derive from one authoritative telemetry history"
    );
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
    assert!(!dir.path().join("scale_energy.parquet").exists());
    assert!(!dir.path().join("mark_pair_covariance.parquet").exists());
    assert!(!dir.path().join("territories.geojson").exists());
    assert!(!dir.path().join("figures").exists());
}

#[test]
fn all_result_floats_are_finite() {
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
    config.multiscale_residual.enabled = false;
    config.output.write_parquet_curves = false;
    config.output.write_geojson_territories = false;
    config.output.write_figures = false;
    config.output.write_run_manifest = false;

    let mut result = AnalysisEngine::new(config.clone())
        .expect("engine")
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");
    result.qc.mean_tumor_probability = Some(f64::INFINITY);
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
        .residual_territories
        .value()
        .expect("multiscale residual territories")
        .clone();
    assert!(!territories.is_empty());

    let dir = tempfile::tempdir().expect("temp dir");
    OutputWriter::write(&ResultDocument::marked(result), dir.path(), &config.output)
        .expect("write outputs");
    let geojson: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("residual_territories.geojson")).expect("territories"),
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
    assert_eq!(
        feature["properties"]["analysis_scale_um"],
        territories[0].analysis_scale_um
    );
    assert_eq!(
        feature["properties"]["residual_score"],
        territories[0].residual_score
    );
    assert_eq!(
        feature["properties"]["supporting_marked_cells"],
        territories[0].supporting_marked_cells
    );
    assert!(feature["properties"]["qc_overlap_fraction"].is_null());
    assert!(feature["properties"].get("z_or_power").is_none());
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
    let mut pre = engine
        .analyze_pattern(&pattern("case_001", "pre", vec![1, 0, 0, 0]))
        .expect("pre");
    let mut post = engine
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("post");

    pre.spectrum_curve = vec![SpectrumPoint {
        k: 1.0,
        observed_power: 1.0,
        median_permutation_power: 1.0,
        whitened_power: 1.0,
        inference_eligible: true,
        lower_global_envelope: Some(0.8),
        upper_global_envelope: Some(1.2),
    }];
    post.spectrum_curve = vec![SpectrumPoint {
        whitened_power: 1.01,
        ..pre.spectrum_curve[0].clone()
    }];

    let delta = compare_prepost(&pre, &post);
    let text = delta.interpretation_text.to_lowercase();

    assert!(delta
        .curve_comparisons
        .iter()
        .any(|test| test.comparison_name == "spectrum"));
    assert!(delta.curve_comparisons.iter().any(|test| test
        .interpretation
        .contains("unavailable without a prespecified descriptive margin")));
    let delta_json = serde_json::to_value(&delta).expect("delta json");
    assert!(delta_json["curve_comparisons"].is_array());

    assert!(text.contains("coarse-scale organization"));
    assert!(!text.contains("same cells"));
    assert!(!text.contains("clone"));
    assert!(!text.contains("directional growth"));
    assert!(!text.contains("gain"));
    assert!(!text.contains("loss"));
}

#[test]
fn report_distinguishes_difference_diagnostics_from_margin_assessments() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.output.write_parquet_curves = false;
    let engine = AnalysisEngine::new(config).expect("engine");
    let mut result = engine
        .analyze_pattern(&pattern("case_001", "post", vec![1, 0, 1, 0]))
        .expect("analysis");

    result
        .prepost_curve_comparisons
        .push(CurveComparisonResult {
            comparison_name: "spectrum".into(),
            method: crate::CurveComparisonMethod::PooledBinPermutation,
            metric: "max_abs_standardized_difference".into(),
            availability: CurveComparisonAvailability::Available,
            statistic: Some(0.1),
            unavailable_reason: None,
            pooled_bin_p_value: Some(0.6),
            margin: None,
            within_margin: None,
            interpretation: "nonsignificant diagnostic".into(),
        });

    let report = render_analysis_report(&result);

    assert!(report.contains("pooled-bin permutation diagnostics describe difference"));
    assert!(report.contains("descriptive margin assessments only report whether"));
    assert!(report.contains("nonsignificant difference diagnostic is not interpreted as sameness"));
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
            expected_edges: 0.0,
            enrichment_ratio: None,
            enrichment_ratio_unavailable_reason: Some(
                EnrichmentStatisticUnavailableReason::ZeroExpectedEdges,
            ),
            z_score: None,
            z_score_unavailable_reason: Some(
                EnrichmentStatisticUnavailableReason::ZeroNullVariance,
            ),
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
        },
    ];
    result.cross_interaction_curves =
        crate::AnalysisSection::available(vec![CrossInteractionCurve {
            label_a: "mmr_abnormal".into(),
            label_b: "lymphocyte".into(),
            points: vec![CrossInteractionPoint {
                r_min_um: 0.0,
                r_max_um: 10.0,
                value: Some(1.2),
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
