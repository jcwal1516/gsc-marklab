#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

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
    assert_eq!(result["backend"]["version"], "1.18.1");
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
