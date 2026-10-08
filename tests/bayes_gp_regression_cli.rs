#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn exact_gp_interpolates_smooth_micrometre_field_with_finite_uncertainty() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(
        &input,
        "observation_id,x_um,value\n\
o-0,0,1.00\n\
o-1,1,1.48\n\
o-2,2,1.84\n\
o-3,3,2.00\n\
o-4,4,1.91\n\
o-5,5,1.60\n\
o-6,6,1.14\n\
o-7,7,0.65\n",
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
    .expect("prediction coordinates");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "gp-regression",
            "--input",
            input.to_str().expect("input path"),
            "--predict",
            prediction.to_str().expect("prediction path"),
            "--mean-prior-mean",
            "0",
            "--mean-prior-sd",
            "5",
            "--amplitude-prior-sd",
            "2",
            "--length-scale-prior-sd-um",
            "5",
            "--noise-prior-sd",
            "0.5",
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
            "20260827",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_gp_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(result["claim_status"], "experimental");
    assert_eq!(result["model"]["family"], "exact_matern32_gp_regression");
    assert_eq!(result["model"]["coordinate_unit"], "micrometre");
    assert_eq!(result["input"]["observations"], 8);
    assert_eq!(result["input"]["predictions"], 3);

    for name in ["amplitude", "length_scale_um", "noise_sd"] {
        assert!(
            result["posterior"][name]["mean"]
                .as_f64()
                .expect("hyperparameter")
                > 0.0
        );
    }
    let predictions = result["predictions"].as_array().expect("predictions");
    assert_eq!(predictions.len(), 3);
    for prediction in predictions {
        assert!(prediction["sd"].as_f64().expect("predictive SD") > 0.0);
        assert!(
            prediction["interval_lower"].as_f64().unwrap()
                < prediction["interval_upper"].as_f64().unwrap()
        );
    }
    assert!((predictions[0]["mean"].as_f64().unwrap() - 1.24).abs() <= 0.45);
    assert!((predictions[1]["mean"].as_f64().unwrap() - 1.95).abs() <= 0.45);
    assert!((predictions[2]["mean"].as_f64().unwrap() - 0.90).abs() <= 0.45);
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
}
