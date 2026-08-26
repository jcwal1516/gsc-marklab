#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn mixture_gate_learns_context_and_masks_unavailable_experts() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("experts.csv");
    let mut fixture =
        "patient_id,split,expert_prediction_source,label,context_signal,expert_a,expert_b\n"
            .to_owned();
    for index in 0..30 {
        let split = if index < 12 {
            "gate_train"
        } else if index < 22 {
            "calibration"
        } else {
            "test"
        };
        let label = u8::from(!(index as usize).is_multiple_of(2));
        let context = if index % 4 < 2 {
            -1.0 - 0.01 * index as f64
        } else {
            1.0 + 0.01 * index as f64
        };
        let accurate = if label == 1 { 0.85 } else { 0.15 };
        let weak = if label == 1 { 0.55 } else { 0.45 };
        let (mut a, mut b) = if context < 0.0 {
            (accurate.to_string(), weak.to_string())
        } else {
            (weak.to_string(), accurate.to_string())
        };
        if index == 24 {
            a.clear();
        }
        if index == 27 {
            b.clear();
        }
        fixture.push_str(&format!(
            "p{index:02},{split},patient_level_out_of_fold,{label},{context},{a},{b}\n"
        ));
    }
    fs::write(&input, fixture).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "mixture-of-experts-fusion",
            "--input",
            input.to_str().expect("input path"),
            "--l2-penalty",
            "0.1",
            "--entropy-regularization",
            "0.01",
            "--ood-validation-quantile",
            "0.9",
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.mixture_of_experts_fusion");
    assert_eq!(
        result["expert_prediction_source"],
        "patient_level_out_of_fold"
    );
    assert_eq!(
        result["gating_features"],
        serde_json::json!(["context_signal", "expert_a_available", "expert_b_available"])
    );
    let predictions = result["predictions"].as_array().unwrap();
    assert_eq!(predictions.len(), 8);
    let negative = predictions
        .iter()
        .find(|row| row["patient_id"] == "p25")
        .unwrap();
    assert!(
        negative["gating_weights"][0].as_f64().unwrap()
            > negative["gating_weights"][1].as_f64().unwrap()
    );
    let positive = predictions
        .iter()
        .find(|row| row["patient_id"] == "p26")
        .unwrap();
    assert!(
        positive["gating_weights"][1].as_f64().unwrap()
            > positive["gating_weights"][0].as_f64().unwrap()
    );
    let missing_a = predictions
        .iter()
        .find(|row| row["patient_id"] == "p24")
        .unwrap();
    assert_eq!(missing_a["gating_weights"], serde_json::json!([0.0, 1.0]));
    let missing_b = predictions
        .iter()
        .find(|row| row["patient_id"] == "p27")
        .unwrap();
    assert_eq!(missing_b["gating_weights"], serde_json::json!([1.0, 0.0]));
    assert!(result["ood_threshold"].as_f64().unwrap().is_finite());
    assert!(result["metrics"]["brier_score"].as_f64().unwrap() < 0.25);
}
