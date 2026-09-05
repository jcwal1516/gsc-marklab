#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn predictive_stacking_optimizes_patient_grouped_loo_densities() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("log_densities.csv");
    let mut fixture = "patient_id,held_out_unit,model_a,model_b\n".to_owned();
    for index in 0..8 {
        let (a, b) = if index < 4 {
            (0.8_f64, 0.2_f64)
        } else {
            (0.2, 0.8)
        };
        fixture.push_str(&format!("p{index},patient,{},{}\n", a.ln(), b.ln()));
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
            "predictive-stacking",
            "--input",
            input.to_str().expect("input path"),
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.predictive_stacking");
    assert_eq!(result["version"], 2);
    assert_eq!(result["backend"]["name"], "marklab-rust");
    assert_eq!(result["held_out_unit"], "patient");
    assert_eq!(result["patient_count"], 8);
    let weights = result["weights"].as_array().unwrap();
    assert_eq!(weights.len(), 2);
    assert!((weights[0]["weight"].as_f64().unwrap() - 0.5).abs() < 1e-8);
    assert!((weights[1]["weight"].as_f64().unwrap() - 0.5).abs() < 1e-8);
    assert!(
        (weights
            .iter()
            .map(|row| row["weight"].as_f64().unwrap())
            .sum::<f64>()
            - 1.0)
            .abs()
            < 1e-12
    );
    assert_eq!(
        result["grouped_mixture_log_predictive_density"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        result["leave_one_patient_out_sensitivity"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        result["weight_interpretation"],
        "predictive_optimization_weights_not_posterior_model_probabilities"
    );
}

#[test]
fn native_stacking_preserves_exact_csv_transport_and_atomic_publication() {
    let mut csv = "patient_id,held_out_unit,model_a,model_b\n".to_owned();
    for i in 0..8 {
        let a = if i < 4 {
            -0.16392574019031287_f64
        } else {
            -1.1639257401903128_f64
        };
        let b = if i < 4 {
            -1.1639257401903128_f64
        } else {
            -0.16392574019031287_f64
        };
        csv.push_str(&format!("p{i},patient,{a},{b}\n"));
    }
    let wire = marklab::predictive_stacking::native_request(csv.as_bytes().to_vec(), 30).unwrap();
    let response = Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "native-predictive-stacking"])
        .write_stdin(wire.clone())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let direct =
        serde_json::to_vec(&marklab::predictive_stacking::fit_csv(csv.as_bytes(), 30).unwrap())
            .unwrap();
    assert_eq!(response, direct);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.csv");
    let output = directory.path().join("result.json");
    fs::write(&input, &csv).unwrap();
    fs::write(&output, "existing result").unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "predictive-stacking",
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
            0 => bad["timeout_seconds"] = serde_json::json!(0),
            1 => bad["extra"] = serde_json::json!(true),
            _ => bad["csv"] = serde_json::json!(csv.replace(",patient,", ",slide,")),
        };
        Command::cargo_bin("marklab")
            .unwrap()
            .args(["backend", "native-predictive-stacking"])
            .write_stdin(serde_json::to_vec(&bad).unwrap())
            .assert()
            .failure();
    }
}

#[test]
fn stacking_csv_stops_at_declared_row_column_and_byte_bounds() {
    let csv = format!(
        "patient_id,held_out_unit,model_a,model_b\n{}extra,patient,invalid,0\n",
        "p,patient,0,0\n".repeat(500)
    );
    assert!(marklab::predictive_stacking::fit_csv(csv.as_bytes(), 30)
        .unwrap_err()
        .to_string()
        .contains("500"));
    let csv = format!(
        "patient_id,held_out_unit,{}\n",
        (0..17)
            .map(|i| format!("model_{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(marklab::predictive_stacking::fit_csv(csv.as_bytes(), 30)
        .unwrap_err()
        .to_string()
        .contains("header"));
    assert!(
        marklab::predictive_stacking::fit_csv(&vec![b'a'; 16 * 1024 * 1024 + 1], 30)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
}
