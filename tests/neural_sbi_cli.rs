#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pinned_sbi_npe_nle_nre_and_sequential_npe_match_truncated_gaussian_posterior() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("neural_sbi.json");
    let fixture = serde_json::json!({
        "prior":{"family":"uniform","lower":-3.0,"upper":3.0},
        "simulator":{"family":"gaussian_location","noise_standard_deviation":1.0},
        "observed_summary":1.0,
        "simulations_per_estimator":1024,
        "sequential_rounds":2,
        "simulations_per_round":512,
        "training":{"density_estimator":"mdn","ratio_classifier":"mlp","batch_size":128,"maximum_epochs":50,"stop_after_epochs":8},
        "posterior_grid_points":1201,
        "seed":83,
        "timeout_seconds":120
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .current_dir(directory.path())
        .args([
            "neural",
            "sbi",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        !directory.path().join("sbi-logs").exists(),
        "the static backend must not publish undeclared TensorBoard artifacts"
    );

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.neural_simulation_based_inference"
    );
    assert_eq!(result["backend"]["version"], "sbi-0.26.1");
    for method in ["npe", "nle", "nre"] {
        assert!(
            result["estimators"][method]["posterior_mean_absolute_error"]
                .as_f64()
                .unwrap()
                < 0.25
        );
        assert!(
            result["estimators"][method]["serialized_state_bytes"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    assert_eq!(result["sequential"]["rounds"].as_array().unwrap().len(), 2);
    assert_eq!(result["sequential"]["rounds"][0]["proposal"], "prior");
    assert_eq!(
        result["sequential"]["rounds"][1]["proposal"],
        "round_1_npe_posterior"
    );
    assert!(
        result["sequential"]["posterior_mean_absolute_error"]
            .as_f64()
            .unwrap()
            < 0.25
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_amortized_and_sequential_sbi"
    );
}
