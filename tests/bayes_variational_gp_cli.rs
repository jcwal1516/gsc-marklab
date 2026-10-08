#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn variational_inducing_gp_is_explicitly_approximate_and_stable_across_starts() {
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
    .expect("predictions");
    let output = directory.path().join("result.json");
    let exact_output = directory.path().join("exact.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "variational-gp",
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
            "--inducing-points",
            "4",
            "--starts",
            "2",
            "--iterations",
            "10000",
            "--learning-rate",
            "0.01",
            "--posterior-draws",
            "2000",
            "--seed",
            "20260829",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

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
            exact_output.to_str().expect("exact output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_variational_gp_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "approximate_only");
    assert_eq!(result["claim_status"], "experimental_approximate_only");
    assert_eq!(
        result["model"]["approximation"],
        "vfe_inducing_point_mean_field_advi"
    );
    assert_eq!(result["diagnostics"]["starts"].as_array().unwrap().len(), 2);
    assert!(result["diagnostics"]["all_finite"].as_bool().unwrap());
    assert!(
        result["diagnostics"]["cross_start_prediction_rmse"]
            .as_f64()
            .expect("stability")
            < 0.5
    );
    let inducing = result["inducing_locations_um"]
        .as_array()
        .expect("inducing");
    assert_eq!(inducing.len(), 4);
    assert!(inducing
        .windows(2)
        .all(|pair| pair[0].as_f64().unwrap() < pair[1].as_f64().unwrap()));
    let predictions = result["predictions"].as_array().expect("predictions");
    assert_eq!(predictions.len(), 3);
    assert!(predictions
        .iter()
        .all(|prediction| prediction["sd"].as_f64().unwrap() > 0.0));
    let exact: serde_json::Value =
        serde_json::from_slice(&fs::read(exact_output).expect("exact result")).expect("exact JSON");
    assert_eq!(exact["fit_state"], "complete");
    let exact_predictions = exact["predictions"].as_array().expect("exact predictions");
    let squared_error = predictions
        .iter()
        .zip(exact_predictions)
        .map(|(approximate, exact)| {
            let difference =
                approximate["mean"].as_f64().unwrap() - exact["mean"].as_f64().unwrap();
            difference * difference
        })
        .sum::<f64>();
    let rmse = (squared_error / predictions.len() as f64).sqrt();
    assert!(rmse <= 0.35, "variational/exact prediction RMSE {rmse}");
}
