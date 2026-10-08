#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn one_factor_coregionalization_recovers_positive_loading_and_joint_predictions() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(
        &input,
        "coordinate_id,x_um,output_a,output_b\n\
c-0,0,0.00,0.55\n\
c-1,1,0.48,1.41\n\
c-2,2,0.84,2.25\n\
c-3,3,1.00,2.45\n\
c-4,4,0.91,2.28\n\
c-5,5,0.60,1.65\n\
c-6,6,0.14,0.73\n\
c-7,7,-0.35,-0.15\n",
    )
    .expect("observations");
    let prediction = directory.path().join("prediction.csv");
    fs::write(
        &prediction,
        "prediction_id,x_um\n\
p-1,0.5\n\
p-2,3.5\n\
p-3,6.5\n",
    )
    .expect("predictions");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "multi-output-gp",
            "--input",
            input.to_str().expect("input path"),
            "--predict",
            prediction.to_str().expect("prediction path"),
            "--output-a-name",
            "marker-a",
            "--output-b-name",
            "marker-b",
            "--mean-prior-sd",
            "5",
            "--amplitude-prior-sd",
            "2",
            "--length-scale-prior-sd-um",
            "5",
            "--loading-b-prior-sd",
            "3",
            "--noise-a-sd",
            "0.10",
            "--noise-b-sd",
            "0.20",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "2000",
            "--draws",
            "2000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260828",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_multi_output_gp_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(
        result["model"]["family"],
        "one_factor_two_output_matern32_gp"
    );
    assert_eq!(result["model"]["loading_a"], 1.0);
    assert_eq!(result["model"]["loading_b_constraint"], "positive");
    let loading = result["posterior"]["loading_b"]["mean"]
        .as_f64()
        .expect("loading");
    assert!((loading - 2.0).abs() <= 0.7, "{loading}");

    let predictions = result["predictions"].as_array().expect("predictions");
    assert_eq!(predictions.len(), 3);
    for prediction in predictions {
        assert!(prediction["output_a_sd"].as_f64().unwrap() > 0.0);
        assert!(prediction["output_b_sd"].as_f64().unwrap() > 0.0);
    }
    assert!((predictions[1]["output_a_mean"].as_f64().unwrap() - 0.96).abs() <= 0.4);
    assert!((predictions[1]["output_b_mean"].as_f64().unwrap() - 2.42).abs() <= 0.6);
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
}
