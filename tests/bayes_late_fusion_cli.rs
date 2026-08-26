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
