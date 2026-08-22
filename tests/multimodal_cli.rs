use assert_cmd::Command;
use marklab::{
    AnalysisConfig, HeCell, IhcCell, LandmarkPair, MultimodalEngine, MultimodalInput,
    ResultDocument,
};
use serde_json::Value;

#[test]
fn multimodal_analyze_writes_registration_and_neighborhood_outputs() {
    let fixture = MultimodalFixture::new();

    fixture.run().assert().success();

    assert!(fixture.out.join("registration_qc.json").exists());
    assert!(fixture.out.join("report.md").exists());

    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("result.json")).expect("result"),
    )
    .expect("json");
    let result = &document["analysis"]["result"];
    assert_eq!(result["case_id"], "case_001");
    assert_eq!(result["timepoint"], "post");
    assert_eq!(result["protein"], "MSH6");
    assert_eq!(result["registration"]["value"]["landmark_count"], 4);
    assert_eq!(result["fused_cell_summary"]["value"]["n_fused_cells"], 4);
    assert!(result["neighborhood_enrichment"]["value"].is_array());
    assert!(result["cross_interaction_curves"]["value"]
        .as_array()
        .expect("cross curves")
        .iter()
        .any(|curve| curve["p_global"].is_number()));
    assert!(!result["territory_profiles"]["value"]
        .as_array()
        .expect("territory profiles")
        .is_empty());
    assert_eq!(
        result["territory_comparisons"]["status"],
        "insufficient_data"
    );
    assert!(fixture.out.join("territory_profiles.json").exists());
    assert!(!fixture.out.join("territory_comparisons.json").exists());
}

#[test]
fn library_and_cli_core_results_match() {
    let fixture = MultimodalFixture::new();
    fixture.run().assert().success();

    let cli_document: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("result.json")).expect("CLI result"),
    )
    .expect("CLI result JSON");
    let config = AnalysisConfig::from_toml_path(&fixture.config).expect("config");
    let library_result = MultimodalEngine::new(config)
        .expect("engine")
        .analyze(&MultimodalInput {
            he_cells: vec![
                HeCell {
                    cell_id: "h1".into(),
                    x_um: 0.0,
                    y_um: 0.0,
                    cell_type: Some("lymphocyte".into()),
                    cell_type_probability: Some(0.9),
                },
                HeCell {
                    cell_id: "h2".into(),
                    x_um: 50.0,
                    y_um: 0.0,
                    cell_type: Some("stroma".into()),
                    cell_type_probability: Some(0.8),
                },
            ],
            ihc_cells: vec![
                IhcCell {
                    cell_id: "m1".into(),
                    x_um: 0.0,
                    y_um: 0.0,
                    mmr_mark: Some(1),
                    mmr_probability: Some(0.99),
                },
                IhcCell {
                    cell_id: "m2".into(),
                    x_um: 50.0,
                    y_um: 0.0,
                    mmr_mark: Some(0),
                    mmr_probability: Some(0.01),
                },
            ],
            landmarks: vec![
                LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
                LandmarkPair::new(50.0, 0.0, 50.0, 0.0),
                LandmarkPair::new(0.0, 50.0, 0.0, 50.0),
                LandmarkPair::new(50.0, 50.0, 50.0, 50.0),
            ],
            case_id: "case_001".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
        })
        .expect("library analysis");
    let library_document =
        serde_json::to_value(ResultDocument::multimodal(library_result)).expect("library result");

    assert_eq!(
        cli_document["analysis"]["result"],
        library_document["analysis"]["result"]
    );
}

#[test]
fn multimodal_analyze_uses_true_rigid_rotation() {
    let fixture = MultimodalFixture::new();
    fixture.update_config("transform = \"affine\"", "transform = \"rigid\"");
    std::fs::write(
        &fixture.landmarks,
        "source_x_um,source_y_um,target_x_um,target_y_um\n\
         0,0,10,-4\n\
         50,0,10,46\n\
         0,50,-40,-4\n\
         50,50,-40,46\n",
    )
    .expect("rotated landmarks");

    fixture.run().assert().success();

    let result: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("result.json")).expect("result"),
    )
    .expect("result json");
    let sidecar: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("registration_transform.json"))
            .expect("transform"),
    )
    .expect("transform json");

    assert_eq!(
        result["analysis"]["result"]["registration"]["value"]["transform_type"],
        "rigid"
    );
    assert_eq!(sidecar["transform_type"], "rigid");
    assert!((sidecar["matrix"][0][0].as_f64().expect("m00")).abs() < 1.0e-9);
    assert!((sidecar["matrix"][0][1].as_f64().expect("m01") + 1.0).abs() < 1.0e-9);
    assert!((sidecar["matrix"][1][0].as_f64().expect("m10") - 1.0).abs() < 1.0e-9);
}

#[test]
fn multimodal_analyze_writes_graph_smoothing_diagnostic_when_enabled() {
    let fixture = MultimodalFixture::new();
    fixture.update_config(
        "[diagnostics]\nbeta_posterior_groups = false\ngraph_smoothing = false",
        "[diagnostics]\nbeta_posterior_groups = false\ngraph_smoothing = true",
    );

    fixture.run().assert().success();

    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("result.json")).expect("result"),
    )
    .expect("json");
    let graph_smoothing =
        &document["analysis"]["result"]["diagnostics"]["value"]["graph_smoothing"];
    assert_eq!(
        graph_smoothing["diagnostic_name"],
        "deterministic_graph_smoothing_v1"
    );
    assert_eq!(graph_smoothing["n_nodes"], 4);
    assert!(graph_smoothing["label_pair_scores"]
        .as_array()
        .expect("label pair scores")
        .iter()
        .any(|row| row["label_a"] == "mmr_abnormal"));

    let report = std::fs::read_to_string(fixture.out.join("report.md")).expect("report");
    assert!(report.contains("Optional diagnostics"));
    assert!(report.contains("Graph-smoothing summary"));
    assert!(!report.to_lowercase().contains("clonality"));
    assert!(!report.to_lowercase().contains("proof"));
}

#[test]
fn multimodal_analyze_rejects_beta_posterior_groups_diagnostic() {
    let fixture = MultimodalFixture::new();
    fixture.update_config(
        "[diagnostics]\nbeta_posterior_groups = false\ngraph_smoothing = false",
        "[diagnostics]\nbeta_posterior_groups = true\ngraph_smoothing = false",
    );

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains("beta_posterior_groups diagnostic requires marked-pattern input"));
}

#[test]
fn multimodal_analyze_writes_qc_csv_and_null_sensitivity_sidecars() {
    let fixture = MultimodalFixture::new();

    fixture.run().assert().success();

    for artifact in [
        "registration_residuals.csv",
        "registration_residuals.json",
        "registration_transform.json",
        "registration_extrapolation.json",
        "fused_cells.csv",
        "neighborhood_territories.csv",
        "territory_profiles.csv",
        "cross_interaction_curves.csv",
        "neighborhood_enrichment.csv",
        "null_model_sensitivity.json",
        "null_model_sensitivity.csv",
    ] {
        assert!(fixture.out.join(artifact).exists(), "{artifact} missing");
    }
    assert!(
        !fixture.out.join("territory_comparisons.csv").exists(),
        "empty territory comparison data must not create an artifact"
    );

    let transform: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("registration_transform.json"))
            .expect("transform"),
    )
    .expect("json");
    assert_eq!(transform["transform_type"], "affine");
    assert!(transform["matrix"].as_array().expect("matrix").len() == 2);

    let extrapolation: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("registration_extrapolation.json"))
            .expect("extrapolation"),
    )
    .expect("json");
    assert_eq!(extrapolation["n_cells"], 4);
    assert!(extrapolation["fraction_outside_landmark_hull"]
        .as_f64()
        .expect("fraction")
        .is_finite());

    let sensitivity: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("null_model_sensitivity.json"))
            .expect("sensitivity"),
    )
    .expect("json");
    assert!(sensitivity
        .as_array()
        .expect("sensitivity rows")
        .iter()
        .any(|row| row["null_model"] == "source_section_density"));

    let fused_csv =
        std::fs::read_to_string(fixture.out.join("fused_cells.csv")).expect("fused CSV");
    let header = fused_csv.lines().next().expect("fused CSV header");
    assert!(header.contains("case_id"));
    assert!(header.contains("timepoint"));
    assert!(header.contains("protein"));
    assert!(fused_csv
        .lines()
        .skip(1)
        .all(|row| row.contains("case_001") && row.contains("post") && row.contains("MSH6")));
}

#[test]
fn multimodal_extrapolation_does_not_classify_degenerate_hull_as_inside() {
    let fixture = MultimodalFixture::new();
    fixture.update_config("transform = \"affine\"", "transform = \"rigid\"");
    std::fs::write(
        &fixture.landmarks,
        "source_x_um,source_y_um,target_x_um,target_y_um\n\
         0,0,0,0\n\
         25,0,25,0\n\
         50,0,50,0\n",
    )
    .expect("collinear landmarks");

    fixture.run().assert().success();

    let extrapolation: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("registration_extrapolation.json"))
            .expect("extrapolation"),
    )
    .expect("extrapolation JSON");
    assert_eq!(
        extrapolation["availability"],
        "degenerate_collinear_landmarks"
    );
    assert!(extrapolation["n_outside_landmark_hull"].is_null());
    assert!(extrapolation["fraction_outside_landmark_hull"].is_null());
    assert!(extrapolation["cell_flags"]
        .as_array()
        .expect("cell flags")
        .iter()
        .all(|cell| cell["outside_landmark_hull"].is_null()));
}

#[test]
fn multimodal_report_uses_multimodal_wording_without_single_modality_placeholders() {
    let fixture = MultimodalFixture::new();

    fixture.run().assert().success();

    let report = std::fs::read_to_string(fixture.out.join("report.md")).expect("report");
    assert!(report.contains("serial-section cells placed in a shared coordinate frame"));
    assert!(report.contains("serial-section neighborhood associations"));
    assert!(report.contains("registration"));
    assert!(report.contains("P95 residual"));
    assert!(report.contains("below the registration uncertainty scale are diagnostic only"));
    assert!(report.contains("not same-cell matches"));
    assert!(!report.contains("same physical cells"));
    assert!(!report.contains("Spectrum:"));
    assert!(!report.contains("Multiscale residual:"));
    assert!(!report.contains("Primary endpoint: low-k excess"));
    assert!(!report.contains("marked, p_hat"));
    assert!(!report.to_lowercase().contains("clonality"));
    assert!(!report.to_lowercase().contains("proof"));
}

#[test]
fn multimodal_analyze_rejects_malformed_landmark_headers() {
    let fixture = MultimodalFixture::new();
    std::fs::write(
        &fixture.landmarks,
        "source_x_um,source_y_um,target_x_um,bad_y\n0,0,0,0\n",
    )
    .expect("landmarks");

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains(fixture.landmarks.to_str().unwrap()));
    assert!(stderr.contains("expected landmark CSV headers"));
}

#[test]
fn multimodal_analyze_rejects_non_finite_landmark_coordinates_with_row_context() {
    let fixture = MultimodalFixture::new();
    std::fs::write(
        &fixture.landmarks,
        "source_x_um,source_y_um,target_x_um,target_y_um\n0,0,0,0\nNaN,0,0,0\n0,50,0,50\n50,50,50,50\n",
    )
    .expect("landmarks");

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains(fixture.landmarks.to_str().unwrap()));
    assert!(stderr.contains("row 3"));
    assert!(stderr.contains("landmark coordinates must be finite"));
}

#[test]
fn multimodal_analyze_rejects_too_few_landmarks() {
    let fixture = MultimodalFixture::new();
    std::fs::write(
        &fixture.landmarks,
        "source_x_um,source_y_um,target_x_um,target_y_um\n0,0,0,0\n50,0,50,0\n",
    )
    .expect("landmarks");

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains("registration requires at least 3 landmarks, found 2"));
}

#[test]
fn multimodal_analyze_rejects_rmse_threshold_failure() {
    let fixture = MultimodalFixture::new();
    fixture.update_config("max_rmse_um = 25.0", "max_rmse_um = 0.001");
    std::fs::write(
        &fixture.landmarks,
        "source_x_um,source_y_um,target_x_um,target_y_um\n0,0,0,0\n50,0,50,0\n0,50,0,50\n50,50,100,50\n",
    )
    .expect("landmarks");

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains("registration RMSE"));
    assert!(stderr.contains("exceeds configured max_rmse_um"));
}

#[test]
fn multimodal_analyze_rejects_unsupported_transform() {
    let fixture = MultimodalFixture::new();
    fixture.update_config("transform = \"affine\"", "transform = \"projective\"");

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(
        stderr.contains("registration.transform") && stderr.contains("projective"),
        "{stderr}"
    );
}

#[test]
fn multimodal_analyze_rejects_disabled_registration_or_neighborhood() {
    for (needle, replacement, expected) in [
        (
            "enabled = true\ntransform = \"affine\"",
            "enabled = false\ntransform = \"affine\"",
            "multimodal analyze requires [registration].enabled = true",
        ),
        (
            "enabled = true\nradius_um = 60.0",
            "enabled = false\nradius_um = 60.0",
            "multimodal analyze requires [neighborhood].enabled = true",
        ),
    ] {
        let fixture = MultimodalFixture::new();
        fixture.update_config(needle, replacement);

        let output = fixture.run().assert().failure().get_output().stderr.clone();
        let stderr = String::from_utf8(output).expect("stderr");
        assert!(stderr.contains(expected));
    }
}

#[test]
fn multimodal_analyze_rejects_non_finite_max_rmse_config() {
    let fixture = MultimodalFixture::new();
    fixture.update_config("max_rmse_um = 25.0", "max_rmse_um = nan");

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains("registration.max_rmse_um must be finite and non-negative"));
}

#[test]
fn multimodal_analyze_uses_registration_uncertainty_scale_for_fused_cells() {
    let fixture = MultimodalFixture::new();
    fixture.update_config("max_rmse_um = 25.0", "max_rmse_um = 100.0");
    std::fs::write(
        &fixture.landmarks,
        "source_x_um,source_y_um,target_x_um,target_y_um\n0,0,0,0\n50,0,50,0\n0,50,0,50\n50,50,80,50\n",
    )
    .expect("landmarks");

    fixture.run().assert().success();

    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("result.json")).expect("result"),
    )
    .expect("json");
    let result = &document["analysis"]["result"];
    let usable_min_distance = result["registration"]["value"]["usable_min_distance_um"]
        .as_f64()
        .expect("usable distance");
    let fused_registration_error = result["fused_cell_summary"]["value"]["registration_error_um"]
        .as_f64()
        .expect("registration error");

    assert!(usable_min_distance > 0.0);
    assert!((2.0 * fused_registration_error - usable_min_distance).abs() < 1.0e-9);
}

#[test]
fn multimodal_analyze_uses_configured_domain_detection() {
    let fixture = MultimodalFixture::new();
    std::fs::write(
        &fixture.ihc,
        "cell_id,x_um,y_um,mmr_mark,mmr_probability\nm1,0,0,1,0.99\nm2,8,0,1,0.98\nm3,200,0,1,0.97\nm4,50,0,0,0.01\n",
    )
    .expect("ihc");
    fixture.update_config(
        "territory_eps_um = 60.0\nterritory_min_cells = 1\nterritory_min_radius_um = 1.0",
        "territory_eps_um = 12.0\nterritory_min_cells = 2\nterritory_min_radius_um = 1.0",
    );

    fixture.run().assert().success();

    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.out.join("result.json")).expect("result"),
    )
    .expect("json");
    let result = &document["analysis"]["result"];
    let territories = result["neighborhood_territories"]["value"]
        .as_array()
        .expect("territories");
    assert_eq!(territories.len(), 1);
    assert_eq!(territories[0]["supporting_abnormal_cells"], 2);
    assert_eq!(territories[0]["cluster_id"], 0);
    assert!(territories[0].get("z_or_power").is_none());
    assert!(territories[0].get("scale_um").is_none());
    assert!(territories[0].get("qc_overlap_fraction").is_none());
    assert!((territories[0]["center_x_um"].as_f64().unwrap() - 4.0).abs() < 1.0e-9);
}

#[test]
fn multimodal_analyze_accepts_cellvit_he_csv_with_class_normalization() {
    let fixture = MultimodalFixture::new();
    std::fs::write(
        &fixture.he,
        "cell_id,x_centroid_um,y_centroid_um,predicted_class,class_probability\nh1,0,0,T lymphocyte,0.91\nh2,50,0,Stromal cell,0.88\n",
    )
    .expect("he");

    fixture
        .run_with_extra_args([
            "--he-format",
            "cellvit-csv",
            "--cellvit-min-probability",
            "0.80",
        ])
        .assert()
        .success();

    let fused_csv = std::fs::read_to_string(fixture.out.join("fused_cells.csv")).expect("fused");
    assert!(fused_csv.contains("lymphocyte"));
    assert!(fused_csv.contains("stroma"));
}

#[test]
fn multimodal_analyze_rejects_low_confidence_cellvit_he_rows() {
    let fixture = MultimodalFixture::new();
    std::fs::write(
        &fixture.he,
        "cell_id,x_centroid_um,y_centroid_um,predicted_class,class_probability\nh1,0,0,T lymphocyte,0.20\nh2,50,0,Stromal cell,0.88\n",
    )
    .expect("he");

    let output = fixture
        .run_with_extra_args([
            "--he-format",
            "cellvit-csv",
            "--cellvit-min-probability",
            "0.80",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains("below configured CellViT minimum probability"));
}

#[test]
fn multimodal_prepost_cli_writes_cross_curve_delta_result() {
    let pre = MultimodalFixture::new();
    let post = MultimodalFixture::new();
    let delta_dir = tempfile::tempdir().expect("delta");
    let delta_out = delta_dir.path().join("delta");

    std::fs::write(
        &post.ihc,
        "cell_id,x_um,y_um,mmr_mark,mmr_probability\nm1,0,0,1,0.99\nm2,50,0,0,0.01\nm3,200,0,1,0.99\n",
    )
    .expect("post ihc");
    pre.run().assert().success();
    post.run().assert().success();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "prepost",
            "--pre",
            pre.out.to_str().unwrap(),
            "--post",
            post.out.to_str().unwrap(),
            "--out",
            delta_out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let delta: Value = serde_json::from_str(
        &std::fs::read_to_string(delta_out.join("prepost.json")).expect("delta"),
    )
    .expect("json");
    assert_eq!(delta["format_version"], "0.3");
    assert_eq!(delta["analysis"]["kind"], "multimodal_prepost");
    let result = &delta["analysis"]["result"];
    assert!(result["curve_comparisons"]
        .as_array()
        .expect("curve comparisons")
        .iter()
        .any(|test| test["comparison_name"]
            .as_str()
            .is_some_and(|name| name.starts_with("cross_interaction:"))));
    assert_eq!(result["delta_territory_count"]["value"], 1);
    assert_eq!(result["territory_summary"]["value"]["delta_count"], 1);
    assert!(
        result["territory_summary"]["value"]["new_domain_count"]
            .as_u64()
            .expect("new domains")
            >= 1
    );
}

#[test]
fn multimodal_batch_runs_analyze_and_prepost_manifest_rows() {
    let analyze = MultimodalFixture::new();
    let pre = MultimodalFixture::new();
    let post = MultimodalFixture::new();
    let batch_dir = tempfile::tempdir().expect("batch");
    let manifest = batch_dir.path().join("manifest.csv");
    let out = batch_dir.path().join("out");

    pre.run().assert().success();
    post.run().assert().success();

    std::fs::write(
        &manifest,
        format!(
            "id,he_cells,ihc_cells,landmarks,config,case_id,timepoint,protein,pre,post\n\
reanalyze,{},{},{},{},case_001,post,MSH6,,\n\
pair,,,,,,,,{},{}\n",
            analyze.he.display(),
            analyze.ihc.display(),
            analyze.landmarks.display(),
            analyze.config.display(),
            pre.out.join("result.json").display(),
            post.out.join("result.json").display(),
        ),
    )
    .expect("manifest");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "batch",
            "--manifest",
            manifest.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(out.join("reanalyze").join("result.json").exists());
    assert!(out.join("pair").join("prepost.json").exists());
}

#[cfg(not(feature = "parquet"))]
#[test]
fn multimodal_analyze_rejects_parquet_outputs_before_writing_partial_outputs() {
    let fixture = MultimodalFixture::new();
    fixture.update_config(
        "write_parquet_curves = false",
        "write_parquet_curves = true",
    );

    let output = fixture.run().assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8(output).expect("stderr");

    assert!(stderr.contains("Multimodal parquet output requires the parquet feature"));
    assert!(!fixture.out.join("result.json").exists());
    assert!(!fixture.out.join("registration_qc.json").exists());
    assert!(!fixture.out.join("report.md").exists());
}

struct MultimodalFixture {
    _dir: tempfile::TempDir,
    he: std::path::PathBuf,
    ihc: std::path::PathBuf,
    landmarks: std::path::PathBuf,
    config: std::path::PathBuf,
    out: std::path::PathBuf,
}

impl MultimodalFixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let he = dir.path().join("he.csv");
        let ihc = dir.path().join("ihc.csv");
        let landmarks = dir.path().join("landmarks.csv");
        let config = dir.path().join("config.toml");
        let out = dir.path().join("out");

        std::fs::write(&he, "cell_id,x_um,y_um,cell_type,cell_type_probability\nh1,0,0,lymphocyte,0.9\nh2,50,0,stroma,0.8\n").expect("he");
        std::fs::write(
            &ihc,
            "cell_id,x_um,y_um,mmr_mark,mmr_probability\nm1,0,0,1,0.99\nm2,50,0,0,0.01\n",
        )
        .expect("ihc");
        std::fs::write(&landmarks, "source_x_um,source_y_um,target_x_um,target_y_um\n0,0,0,0\n50,0,50,0\n0,50,0,50\n50,50,50,50\n").expect("landmarks");
        std::fs::write(&config, include_str!("../examples/multimodal_config.toml"))
            .expect("config");

        Self {
            _dir: dir,
            he,
            ihc,
            landmarks,
            config,
            out,
        }
    }

    fn run(&self) -> Command {
        self.run_with_extra_args(std::iter::empty::<&str>())
    }

    fn run_with_extra_args<I, S>(&self, extra_args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "multimodal",
            "analyze",
            "--he-cells",
            self.he.to_str().unwrap(),
            "--ihc-cells",
            self.ihc.to_str().unwrap(),
            "--landmarks",
            self.landmarks.to_str().unwrap(),
            "--config",
            self.config.to_str().unwrap(),
            "--out",
            self.out.to_str().unwrap(),
            "--case-id",
            "case_001",
            "--timepoint",
            "post",
            "--protein",
            "MSH6",
        ]);
        command.args(extra_args);
        command
    }

    fn update_config(&self, needle: &str, replacement: &str) {
        let config = std::fs::read_to_string(&self.config).expect("read config");
        let updated = config.replace(needle, replacement);
        assert_ne!(config, updated, "config replacement did not match");
        std::fs::write(&self.config, updated).expect("write config");
    }
}
