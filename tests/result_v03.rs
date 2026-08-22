use std::collections::BTreeSet;

use marklab::{
    AnalysisResult, AnalysisSection, AnalysisStatus, FusedCellSummary, Interpretation,
    InterpretationClass, MultimodalResult, OutputWriter, PrePostResult, PrimaryEndpointKind,
    Provenance, RegistrationSummary, ResultDocument, ScaleEnergyBand, SpectrumNullModel,
    TransformKind, WindowSummary,
};

fn sample_result() -> MultimodalResult {
    MultimodalResult {
        case_id: "multimodal".into(),
        timepoint: "unknown".into(),
        protein: "unknown".into(),
        status: AnalysisStatus::Ok,
        registration: AnalysisSection::available(RegistrationSummary {
            transform_type: TransformKind::Affine,
            landmark_count: 3,
            rmse_um: 1.0,
            median_residual_um: 1.0,
            p95_residual_um: 1.0,
            max_residual_um: 1.0,
            usable_min_distance_um: 2.0,
        }),
        fused_cell_summary: AnalysisSection::available(FusedCellSummary {
            n_he_cells: 2,
            n_ihc_cells: 2,
            n_fused_cells: 2,
            registration_error_um: Some(1.0),
        }),
        fused_cells: Vec::new(),
        neighborhood_enrichment: AnalysisSection::available(Vec::new()),
        cross_interaction_curves: AnalysisSection::available(Vec::new()),
        neighborhood_territories: AnalysisSection::available(Vec::new()),
        territory_profiles: AnalysisSection::available(Vec::new()),
        territory_comparisons: AnalysisSection::InsufficientData {
            reason: "territory-profile comparison has not been computed".into(),
        },
        diagnostics: AnalysisSection::Disabled,
        timings: Vec::new(),
        interpretation: Interpretation {
            class: InterpretationClass::MultimodalSummary,
            text: "Multimodal registration, fusion, and neighborhood enrichment summary.".into(),
        },
    }
}

fn sample_prepost_result() -> PrePostResult {
    PrePostResult {
        status_flags: Vec::new(),
        curve_comparisons: Vec::new(),
        delta_xi_um: AnalysisSection::NotApplicable,
        delta_low_k_excess: AnalysisSection::available(0.25),
        delta_alpha: AnalysisSection::NotApplicable,
        delta_anisotropy_index: AnalysisSection::NotApplicable,
        delta_block_mean_variance_fraction: AnalysisSection::NotApplicable,
        delta_territory_count: AnalysisSection::available(1),
        territory_summary: AnalysisSection::NotApplicable,
        interpretation_text: "Descriptive comparison.".into(),
    }
}

#[test]
fn result_v03_roundtrip() {
    let document = ResultDocument::multimodal(sample_result());
    let value = serde_json::to_value(&document).expect("serialize result document");
    let keys = value
        .as_object()
        .expect("top-level object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    assert_eq!(
        keys,
        ["analysis", "format_version", "provenance"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(value["format_version"], "0.3");
    assert_eq!(value["analysis"]["kind"], "multimodal");
    assert!(value["analysis"]["result"].is_object());
    assert!(value["analysis"]["result"].get("format_version").is_none());
}

#[test]
fn window_uses_explicit_analysis_effective_length_name() {
    let window = WindowSummary {
        area_um2: 100.0,
        analysis_effective_length_um: 11.284,
        d_nn_mean_um: 2.0,
    };
    let value = serde_json::to_value(&window).expect("window value");

    assert_eq!(value["analysis_effective_length_um"], 11.284);
    assert!(value.get("l_eff_um").is_none());
    assert!(serde_json::from_value::<WindowSummary>(serde_json::json!({
        "area_um2": 100.0,
        "l_eff_um": 11.284,
        "d_nn_mean_um": 2.0
    }))
    .is_err());
}

#[test]
fn unknown_machine_status_and_interpretation_class_are_rejected() {
    let document = ResultDocument::multimodal(sample_result());
    let mut unknown_status = serde_json::to_value(&document).expect("document value");
    unknown_status["analysis"]["result"]["status"] = serde_json::json!("invented");
    assert!(serde_json::from_value::<ResultDocument>(unknown_status).is_err());

    let mut unknown_class = serde_json::to_value(&document).expect("document value");
    unknown_class["analysis"]["result"]["interpretation"]["class"] = serde_json::json!("invented");
    assert!(serde_json::from_value::<ResultDocument>(unknown_class).is_err());

    let mut unknown_transform = serde_json::to_value(&document).expect("document value");
    unknown_transform["analysis"]["result"]["registration"]["value"]["transform_type"] =
        serde_json::json!("invented");
    assert!(serde_json::from_value::<ResultDocument>(unknown_transform).is_err());

    for rejects_unknown in [
        serde_json::from_str::<PrimaryEndpointKind>("\"invented\"").is_err(),
        serde_json::from_str::<SpectrumNullModel>("\"invented\"").is_err(),
        serde_json::from_str::<ScaleEnergyBand>("\"invented\"").is_err(),
    ] {
        assert!(rejects_unknown);
    }
}

#[test]
fn unknown_fields_rejected() {
    let document = ResultDocument::multimodal(sample_result());
    let mut unknown_result_field = serde_json::to_value(&document).expect("document value");
    unknown_result_field["analysis"]["result"]["invented"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ResultDocument>(unknown_result_field).is_err());

    let mut unknown_registration_field = serde_json::to_value(&document).expect("document value");
    unknown_registration_field["analysis"]["result"]["registration"]["value"]["invented"] =
        serde_json::json!(true);
    assert!(serde_json::from_value::<ResultDocument>(unknown_registration_field).is_err());
    assert!(
        serde_json::from_value::<AnalysisSection<u8>>(serde_json::json!({
            "status": "available",
            "value": 3,
            "invented": true
        }))
        .is_err()
    );
}

#[test]
fn registration_summary_has_no_redundant_success_status() {
    let document = ResultDocument::multimodal(sample_result());
    let value = serde_json::to_value(document).expect("document value");

    assert!(value["analysis"]["result"]["registration"]["value"]
        .get("status")
        .is_none());
}

#[test]
fn prepost_result_roundtrip() {
    for (expected_kind, document) in [
        (
            "marked_prepost",
            ResultDocument::marked_prepost(sample_prepost_result()),
        ),
        (
            "multimodal_prepost",
            ResultDocument::multimodal_prepost(sample_prepost_result()),
        ),
    ] {
        let json = serde_json::to_string(&document).expect("serialize pre/post document");
        let parsed = ResultDocument::from_json(&json).expect("parse pre/post document");
        let value = serde_json::to_value(&parsed).expect("pre/post value");

        assert_eq!(value["format_version"], "0.3");
        assert_eq!(value["analysis"]["kind"], expected_kind);
        assert_eq!(parsed, document);
    }
}

#[test]
fn analysis_sections_use_explicit_statuses() {
    assert_eq!(
        serde_json::to_value(AnalysisSection::Available { value: 3_u8 }).unwrap(),
        serde_json::json!({"status": "available", "value": 3})
    );
    assert_eq!(
        serde_json::to_value(AnalysisSection::<u8>::InsufficientData {
            reason: "too few eligible shells".into(),
        })
        .unwrap(),
        serde_json::json!({
            "status": "insufficient_data",
            "reason": "too few eligible shells"
        })
    );
}

#[test]
fn unknown_result_version_rejected() {
    let error =
        ResultDocument::from_json(r#"{"format_version":"9.9","provenance":{},"analysis":{}}"#)
            .expect_err("unknown versions are unsupported");

    assert!(matches!(
        error,
        marklab::MarklabError::UnsupportedFormatVersion { .. }
    ));
}

#[test]
fn output_writer_writes_a_v03_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let document = ResultDocument {
        format_version: "0.3".into(),
        provenance: Provenance {
            program: "marklab".into(),
            crate_version: env!("CARGO_PKG_VERSION").into(),
        },
        analysis: AnalysisResult::Multimodal(sample_result()),
    };
    let mut options = marklab::AnalysisConfig::default().output;
    options.write_parquet_curves = false;
    options.write_geojson_territories = false;
    options.write_figures = false;
    options.write_run_manifest = false;

    let manifest = OutputWriter::write(&document, dir.path(), &options).expect("write output");
    let written = std::fs::read_to_string(dir.path().join("result.json")).expect("result JSON");
    let parsed = ResultDocument::from_json(&written).expect("read written document");

    assert_eq!(
        parsed, document,
        "the writer must preserve the supplied document"
    );
    assert!(matches!(parsed.analysis, AnalysisResult::Multimodal(_)));
    assert!(matches!(
        manifest.result,
        marklab::ArtifactStatus::Written { .. }
    ));
}
