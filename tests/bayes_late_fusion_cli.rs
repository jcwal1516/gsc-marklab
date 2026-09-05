#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn late_fusion_uses_oof_predictions_and_reports_missing_modalities() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("modalities.csv");
    let mut fixture =
        "patient_id,split,base_prediction_source,label,modality_a,modality_b\n".to_owned();
    for index in 0..30 {
        let split = if index < 10 {
            "meta_train"
        } else if index < 20 {
            "calibration"
        } else {
            "test"
        };
        let label = u8::from(!(index as usize).is_multiple_of(2));
        let a = if index >= 20 && index % 5 == 0 {
            String::new()
        } else if label == 1 {
            format!("{}", 0.7 + 0.01 * (index % 3) as f64)
        } else {
            format!("{}", 0.3 - 0.01 * (index % 3) as f64)
        };
        let b = if index >= 20 && index % 5 == 1 {
            String::new()
        } else if label == 1 {
            format!("{}", 0.65 + 0.01 * (index % 4) as f64)
        } else {
            format!("{}", 0.35 - 0.01 * (index % 4) as f64)
        };
        fixture.push_str(&format!(
            "p{index:02},{split},patient_level_out_of_fold,{label},{a},{b}\n"
        ));
    }
    fs::write(&input, fixture).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .env("MARKLAB_PYTHON", "/nonexistent/marklab-python")
        .env("MARKLAB_RUNTIME_ROOT", "/nonexistent/marklab-runtime")
        .args([
            "bayes",
            "late-fusion",
            "--input",
            input.to_str().expect("input path"),
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
    assert_eq!(result["format"], "marklab.late_fusion");
    assert_eq!(result["version"], 2);
    assert_eq!(result["backend"]["name"], "marklab-rust");
    assert!(result["backend"].get("python_version").is_none());
    assert_eq!(
        result["base_prediction_source"],
        "patient_level_out_of_fold"
    );
    assert_eq!(result["meta_fit_split"], "meta_train");
    assert_eq!(result["calibration_split"], "calibration");
    assert_eq!(result["evaluation_split"], "test");
    assert_eq!(
        result["modalities"],
        serde_json::json!(["modality_a", "modality_b"])
    );
    assert_eq!(result["predictions"].as_array().unwrap().len(), 10);
    assert!(result["predictions"].as_array().unwrap().iter().all(|row| {
        let value = row["probability"].as_f64().unwrap();
        (0.0..=1.0).contains(&value)
    }));
    assert_eq!(result["missing_scenarios"].as_array().unwrap().len(), 3);
    assert_eq!(result["modality_ablations"].as_array().unwrap().len(), 2);
    assert!(result["metrics"]["brier_score"].as_f64().unwrap() < 0.25);
}

#[test]
fn native_fusion_transport_preserves_penalty_and_publication_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.csv");
    let output = directory.path().join("result.json");
    let spec: serde_json::Value = serde_json::from_str(include_str!(
        "../crates/marklab-bayes/tests/fixtures/late_fusion/small.spec.json"
    ))
    .unwrap();
    let mut csv =
        "patient_id,split,base_prediction_source,label,modality_0,modality_1\n".to_owned();
    for p in spec["patients"].as_array().unwrap() {
        let probabilities = p["modality_probabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    v.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        csv.push_str(&format!(
            "{},{},patient_level_out_of_fold,{},{}\n",
            p["patient_id"].as_str().unwrap(),
            p["split"].as_str().unwrap(),
            p["label"],
            probabilities
        ));
    }
    let penalty = 0.16392574019031287_f64;
    let wire = marklab::late_fusion::native_request(csv.as_bytes().to_vec(), penalty, 30).unwrap();
    let response = Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "native-late-fusion"])
        .write_stdin(wire.clone())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let direct =
        serde_json::to_vec(&marklab::late_fusion::fit_csv(csv.as_bytes(), penalty, 30).unwrap())
            .unwrap();
    assert_eq!(response, direct);
    fs::write(&input, &csv).unwrap();
    fs::write(&output, "existing result").unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "late-fusion",
            "--l2-penalty",
            "0.1",
            "--timeout-seconds",
            "30",
            "--input",
        ])
        .arg(&input)
        .arg("--out")
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicates::str::contains("output already exists"));
    assert_eq!(fs::read_to_string(&output).unwrap(), "existing result");
    for case in 0..3 {
        let mut bad: serde_json::Value = serde_json::from_slice(&wire).unwrap();
        match case {
            0 => bad["l2_penalty_bits"] = serde_json::json!(f64::NAN.to_bits()),
            1 => bad["extra"] = serde_json::json!(true),
            _ => {
                bad["csv"] =
                    serde_json::json!(csv.replace("patient_level_out_of_fold", "in_sample"))
            }
        };
        Command::cargo_bin("marklab")
            .unwrap()
            .args(["backend", "native-late-fusion"])
            .write_stdin(serde_json::to_vec(&bad).unwrap())
            .assert()
            .failure();
    }
}

#[test]
fn fusion_csv_enforces_row_and_column_budgets_before_extra_values() {
    let mut csv =
        "patient_id,split,base_prediction_source,label,modality_0,modality_1\n".to_owned();
    csv.push_str(&"p,meta_train,patient_level_out_of_fold,0,0.5,0.5\n".repeat(100000));
    csv.push_str("extra,test,patient_level_out_of_fold,1,invalid,0.5\n");
    assert!(marklab::late_fusion::fit_csv(csv.as_bytes(), 0.1, 30)
        .unwrap_err()
        .to_string()
        .contains("100000"));
    let header = format!(
        "patient_id,split,base_prediction_source,label,{}\n",
        (0..17)
            .map(|i| format!("modality_{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(marklab::late_fusion::fit_csv(header.as_bytes(), 0.1, 30)
        .unwrap_err()
        .to_string()
        .contains("header"));
}
