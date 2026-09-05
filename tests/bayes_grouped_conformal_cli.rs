#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn native_request() -> Vec<u8> {
    let spec: marklab_bayes::GroupedConformalSpec = serde_json::from_str(
        &fs::read_to_string(
            "crates/marklab-bayes/tests/fixtures/grouped_conformal/small.spec.json",
        )
        .unwrap(),
    )
    .unwrap();
    let mut csv = "patient_id,split,site,subgroup,label,feature_1,feature_2\n".to_owned();
    for p in &spec.patients {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            p.patient_id, p.split, p.site, p.subgroup, p.label, p.features[0], p.features[1]
        ));
    }
    serde_json::to_vec(&serde_json::json!({"csv":csv,"alpha_bits":spec.alpha.to_bits(),"l2_penalty_bits":spec.l2_penalty.to_bits(),"timeout_seconds":spec.timeout_seconds})).unwrap()
}

#[test]
fn csv_admission_stops_at_the_row_budget_before_decoding_an_extra_feature() {
    let mut csv = String::from("patient_id,split,site,subgroup,label,feature_1,feature_2\n");
    for i in 0..100_000 {
        csv.push_str(&format!("p{i},train,s,g,{},1,2\n", i % 2));
    }
    csv.push_str("extra,train,s,g,1,invalid,2\n");
    let error = marklab::grouped_conformal::fit_csv(csv.as_bytes(), 0.2, 0.1, 30).unwrap_err();
    assert!(error.to_string().contains("100000"), "{error}");
}

#[test]
fn native_worker_executes_without_python_assets_or_interpreter() {
    let output = Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "native-grouped-conformal"])
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .env("MARKLAB_PYTHON", "/nonexistent/python")
        .env("MARKLAB_RUNTIME_ROOT", "/nonexistent/runtime")
        .write_stdin(native_request())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(result["backend"]["name"], "marklab-rust");
    assert_eq!(result["version"], 2);
    assert_eq!(result["predictions"].as_array().unwrap().len(), 8);
}

#[test]
fn native_transport_preserves_exact_csv_and_control_identity() {
    let mut request: serde_json::Value = serde_json::from_slice(&native_request()).unwrap();
    let alpha = f64::from_bits(0.2_f64.to_bits() + 1);
    let penalty = f64::from_bits(0.1_f64.to_bits() + 1);
    request["alpha_bits"] = alpha.to_bits().into();
    request["l2_penalty_bits"] = penalty.to_bits().into();
    let csv = request["csv"].as_str().unwrap();
    let expected = serde_json::to_vec(
        &marklab::grouped_conformal::fit_csv(csv.as_bytes(), alpha, penalty, 120).unwrap(),
    )
    .unwrap();
    let output = Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "native-grouped-conformal"])
        .write_stdin(serde_json::to_vec(&request).unwrap())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(output, expected);
}

#[test]
fn native_transport_rejects_nonfinite_controls_and_unknown_fields() {
    let mut request: serde_json::Value = serde_json::from_slice(&native_request()).unwrap();
    request["alpha_bits"] = f64::NAN.to_bits().into();
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "native-grouped-conformal"])
        .write_stdin(serde_json::to_vec(&request).unwrap())
        .assert()
        .failure()
        .stdout("");
    request["alpha_bits"] = 0.2_f64.to_bits().into();
    request["unexpected"] = true.into();
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "native-grouped-conformal"])
        .write_stdin(serde_json::to_vec(&request).unwrap())
        .assert()
        .failure()
        .stdout("");
}

#[test]
fn relocated_native_cli_runs_without_assets_and_preserves_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let bundle = directory.path().join("native bundle with spaces");
    fs::create_dir(&bundle).unwrap();
    let executable = bundle.join(format!("marklab{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &executable).unwrap();
    let request: serde_json::Value = serde_json::from_slice(&native_request()).unwrap();
    let input = bundle.join("input with spaces.csv");
    fs::write(&input, request["csv"].as_str().unwrap()).unwrap();
    let output = bundle.join("result.json");
    let run = || {
        let mut command = Command::new(&executable);
        command
            .current_dir(&bundle)
            .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
            .env("MARKLAB_PYTHON", "/nonexistent/python")
            .env("MARKLAB_RUNTIME_ROOT", "/nonexistent/runtime")
            .args([
                "bayes",
                "grouped-conformal",
                "--alpha",
                "0.2",
                "--l2-penalty",
                "0.1",
                "--timeout-seconds",
                "30",
                "--input",
            ])
            .arg(&input)
            .arg("--out")
            .arg(&output);
        command
    };
    run().assert().success();
    let original = fs::read(&output).unwrap();
    let expected =
        marklab::grouped_conformal::fit_csv(&fs::read(&input).unwrap(), 0.2, 0.1, 30).unwrap();
    let mut expected = serde_json::to_vec(&expected).unwrap();
    expected.push(b'\n');
    assert_eq!(original, expected);
    run().assert().failure();
    assert_eq!(fs::read(&output).unwrap(), original);
}

#[test]
fn grouped_conformal_fits_train_calibrates_then_scores_test() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let mut fixture = "patient_id,split,site,subgroup,label,feature_1,feature_2\n".to_owned();
    let rows = [
        ("p01", "train", "a", "g1", 0, -2.0, -0.2),
        ("p02", "train", "a", "g2", 0, -1.8, 0.3),
        ("p03", "train", "b", "g1", 0, -1.4, -0.5),
        ("p04", "train", "b", "g2", 1, -1.0, 0.7),
        ("p05", "train", "a", "g1", 0, -0.6, -0.1),
        ("p06", "train", "b", "g2", 0, -0.2, 0.2),
        ("p07", "train", "a", "g1", 1, 0.2, -0.3),
        ("p08", "train", "b", "g2", 1, 0.6, 0.4),
        ("p09", "train", "a", "g1", 0, 1.0, -0.7),
        ("p10", "train", "b", "g2", 1, 1.4, 0.1),
        ("p11", "train", "a", "g1", 1, 1.8, 0.5),
        ("p12", "train", "b", "g2", 1, 2.0, -0.2),
        ("c01", "calibration", "a", "g1", 0, -1.7, 0.1),
        ("c02", "calibration", "b", "g2", 0, -1.3, -0.4),
        ("c03", "calibration", "a", "g1", 1, -0.9, 0.6),
        ("c04", "calibration", "b", "g2", 0, -0.5, 0.0),
        ("c05", "calibration", "a", "g1", 0, -0.1, -0.2),
        ("c06", "calibration", "b", "g2", 1, 0.3, 0.2),
        ("c07", "calibration", "a", "g1", 1, 0.7, -0.4),
        ("c08", "calibration", "b", "g2", 0, 1.1, 0.8),
        ("c09", "calibration", "a", "g1", 1, 1.5, 0.3),
        ("c10", "calibration", "b", "g2", 1, 1.9, -0.1),
        ("q01", "test", "c", "g1", 0, -1.6, 0.0),
        ("q02", "test", "c", "g2", 0, -1.2, -0.2),
        ("q03", "test", "d", "g1", 0, -0.4, 0.1),
        ("q04", "test", "d", "g2", 1, 0.0, 0.5),
        ("q05", "test", "c", "g1", 1, 0.4, -0.1),
        ("q06", "test", "c", "g2", 1, 0.8, 0.2),
        ("q07", "test", "d", "g1", 0, 1.2, -0.8),
        ("q08", "test", "d", "g2", 1, 1.6, 0.0),
    ];
    for row in rows {
        fixture.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            row.0, row.1, row.2, row.3, row.4, row.5, row.6
        ));
    }
    fs::write(&input, fixture).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .env("MARKLAB_PYTHON", "/nonexistent/python")
        .env("MARKLAB_RUNTIME_ROOT", "/nonexistent/runtime")
        .args([
            "bayes",
            "grouped-conformal",
            "--input",
            input.to_str().expect("input path"),
            "--alpha",
            "0.2",
            "--l2-penalty",
            "0.1",
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.grouped_conformal_prediction");
    assert_eq!(result["backend"]["name"], "marklab-rust");
    assert_eq!(result["backend"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["version"], 2);
    assert_eq!(result["fit_split"], "train");
    assert_eq!(result["quantile_split"], "calibration");
    assert_eq!(result["prediction_split"], "test");
    assert_eq!(result["calibration_count"], 10);
    assert_eq!(result["predictions"].as_array().unwrap().len(), 8);
    assert!(result["predictions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| !row["prediction_set"].as_array().unwrap().is_empty()));
    assert_eq!(result["coverage"]["overall"]["count"], 8);
    assert_eq!(result["coverage"]["by_site"].as_array().unwrap().len(), 2);
    assert_eq!(
        result["coverage"]["by_subgroup"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        result["claim_status"],
        "exchangeability_conditional_coverage_not_guaranteed_under_shift"
    );
}
