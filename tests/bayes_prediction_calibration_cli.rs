#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn platt_calibration_is_fit_only_on_patient_oof_rows() {
    let directory = tempfile::tempdir().expect("tempdir");
    let first = directory.path().join("first.csv");
    let second = directory.path().join("second.csv");
    let training = "patient_id,split,score,label\n\
p01,training_oof,-2,0\n\
p02,training_oof,-1.5,0\n\
p03,training_oof,-1,1\n\
p04,training_oof,-0.5,0\n\
p05,training_oof,0.5,1\n\
p06,training_oof,1,0\n\
p07,training_oof,1.5,1\n\
p08,training_oof,2,1\n";
    let test_scores = [-1.75, -1.25, -0.75, -0.25, 0.25, 0.75, 1.25, 1.75];
    let write_fixture = |path: &std::path::Path, labels: [u8; 8]| {
        let mut fixture = training.to_owned();
        for (index, (score, label)) in test_scores.iter().zip(labels).enumerate() {
            fixture.push_str(&format!("q{},test,{score},{label}\n", index + 1));
        }
        fs::write(path, fixture).expect("fixture");
    };
    write_fixture(&first, [0, 1, 0, 0, 1, 1, 0, 1]);
    write_fixture(&second, [1, 0, 1, 1, 0, 0, 1, 0]);

    let run = |input: &std::path::Path, output: &std::path::Path| {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "bayes",
                "calibrate-predictions",
                "--input",
                input.to_str().expect("input path"),
                "--method",
                "platt-logistic",
                "--bins",
                "4",
                "--timeout-seconds",
                "30",
                "--out",
                output.to_str().expect("output path"),
            ])
            .assert()
            .success();
        serde_json::from_slice::<serde_json::Value>(&fs::read(output).expect("result"))
            .expect("JSON")
    };
    let first_result = run(&first, &directory.path().join("first.json"));
    let second_result = run(&second, &directory.path().join("second.json"));

    assert_eq!(first_result["format"], "marklab.prediction_calibration");
    assert_eq!(first_result["backend"]["name"], "scipy");
    assert_eq!(first_result["backend"]["version"], "1.18.1");
    assert_eq!(first_result["fit_split"], "training_oof");
    assert_eq!(first_result["evaluation_split"], "test");
    assert_eq!(first_result["calibrator"], second_result["calibrator"]);
    for (left, right) in first_result["predictions"]
        .as_array()
        .unwrap()
        .iter()
        .zip(second_result["predictions"].as_array().unwrap())
    {
        assert_eq!(left["patient_id"], right["patient_id"]);
        assert_eq!(left["probability"], right["probability"]);
    }
    let probabilities = first_result["predictions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["probability"].as_f64().unwrap())
        .collect::<Vec<_>>();
    assert!(probabilities.windows(2).all(|pair| pair[0] < pair[1]));
    let brier = first_result["metrics"]["brier_score"].as_f64().unwrap();
    let recomputed = first_result["predictions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let residual =
                row["probability"].as_f64().unwrap() - row["label"].as_u64().unwrap() as f64;
            residual * residual
        })
        .sum::<f64>()
        / 8.0;
    assert!((brier - recomputed).abs() < 1e-12);
    assert_eq!(
        first_result["metrics"]["reliability_bins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|bin| bin["count"].as_u64().unwrap())
            .sum::<u64>(),
        8
    );
}
