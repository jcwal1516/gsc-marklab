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
