#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn random_effects_meta_analysis_recovers_regression_and_predicts_new_site() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("sites.csv");
    fs::write(
        &input,
        "site_id,effect,standard_error,covariate\n\
s-1,-0.7,0.15,-3.5\n\
s-2,0.2,0.15,-2.5\n\
s-3,0.3,0.15,-1.5\n\
s-4,1.1,0.15,-0.5\n\
s-5,1.0,0.15,0.5\n\
s-6,1.7,0.15,1.5\n\
s-7,1.75,0.15,2.5\n\
s-8,2.65,0.15,3.5\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "meta-analysis",
            "--input",
            input.to_str().expect("input path"),
            "--covariate-name",
            "site-score",
            "--new-site-covariate",
            "0",
            "--global-prior-mean",
            "0",
            "--global-prior-sd",
            "5",
            "--covariate-prior-sd",
            "2",
            "--heterogeneity-prior-sd",
            "1",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "2000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260826",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_meta_analysis_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(result["claim_status"], "experimental");
    assert_eq!(
        result["model"]["family"],
        "bayesian_random_effects_meta_regression"
    );
    assert_eq!(result["model"]["covariate_name"], "site-score");
    assert_eq!(result["input"]["sites"], 8);

    let global = result["posterior"]["global_effect"]["mean"]
        .as_f64()
        .expect("global");
    let slope = result["posterior"]["covariate_effect"]["mean"]
        .as_f64()
        .expect("slope");
    let heterogeneity = result["posterior"]["heterogeneity"]["mean"]
        .as_f64()
        .expect("heterogeneity");
    assert!((global - 1.0).abs() <= 0.3, "{global}");
    assert!((slope - 0.4).abs() <= 0.15, "{slope}");
    assert!((heterogeneity - 0.3).abs() <= 0.3, "{heterogeneity}");
    assert_eq!(result["site_effects"].as_array().expect("sites").len(), 8);
    assert_eq!(result["new_site_prediction"]["covariate"], 0.0);
    let predicted = result["new_site_prediction"]["mean"]
        .as_f64()
        .expect("prediction");
    assert!((predicted - 1.0).abs() <= 0.4, "{predicted}");
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert!(result["posterior_predictive"]["replicated_effect_mean"]
        .as_f64()
        .expect("predictive mean")
        .is_finite());
}
