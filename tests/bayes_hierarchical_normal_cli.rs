#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn hierarchical_normal_recovers_population_and_partially_pools_patients() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(
        &input,
        "patient_id,observation\n\
p-1,0.2\n\
p-1,0.4\n\
p-1,0.6\n\
p-1,0.8\n\
p-2,0.8\n\
p-2,1.0\n\
p-2,1.2\n\
p-2,1.4\n\
p-3,1.4\n\
p-3,1.6\n\
p-3,1.8\n\
p-3,2.0\n\
p-4,2.0\n\
p-4,2.2\n\
p-4,2.4\n\
p-4,2.6\n\
p-5,2.6\n\
p-5,2.8\n\
p-5,3.0\n\
p-5,3.2\n\
p-6,3.2\n\
p-6,3.4\n\
p-6,3.6\n\
p-6,3.8\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "hierarchical-normal",
            "--input",
            input.to_str().expect("input path"),
            "--global-prior-mean",
            "0",
            "--global-prior-sd",
            "5",
            "--between-patient-sd-prior",
            "2",
            "--known-sigma",
            "1",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "2000",
            "--target-accept",
            "0.9",
            "--seed",
            "20260825",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_hierarchical_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(result["claim_status"], "experimental");
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(result["backend"]["version"], "6.3.0");
    assert_eq!(
        result["model"]["family"],
        "gaussian_patient_varying_intercept"
    );
    assert_eq!(result["model"]["hierarchy"], serde_json::json!(["patient"]));
    assert_eq!(result["input"]["patients"], 6);
    assert_eq!(result["input"]["observations"], 24);

    let global_mean = result["posterior"]["global_mean"]["mean"]
        .as_f64()
        .expect("global mean");
    let between_patient_sd = result["posterior"]["between_patient_sd"]["mean"]
        .as_f64()
        .expect("between-patient SD");
    assert!((global_mean - 2.0).abs() <= 0.4, "{global_mean}");
    assert!(
        (between_patient_sd - 1.1).abs() <= 0.6,
        "{between_patient_sd}"
    );

    let patients = result["partial_pooling"].as_array().expect("patients");
    assert_eq!(patients.len(), 6);
    assert_eq!(patients[0]["patient_id"], "p-1");
    assert_eq!(patients[5]["patient_id"], "p-6");
    let low_raw = patients[0]["raw_mean"].as_f64().expect("low raw");
    let low_posterior = patients[0]["posterior_mean"]
        .as_f64()
        .expect("low posterior");
    let high_raw = patients[5]["raw_mean"].as_f64().expect("high raw");
    let high_posterior = patients[5]["posterior_mean"]
        .as_f64()
        .expect("high posterior");
    assert!(low_posterior > low_raw);
    assert!(high_posterior < high_raw);
    assert!(patients
        .iter()
        .all(|patient| patient["warning"] == "shrinkage_is_model_dependent_not_a_quality_score"));

    assert!(result["posterior"]["variance_partition_mean"]
        .as_f64()
        .expect("variance partition")
        .is_finite());
    assert_eq!(result["diagnostics"]["prior_predictive_finite"], true);
    assert_eq!(result["diagnostics"]["posterior_finite"], true);
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert!(
        result["posterior_predictive"]["replicated_global_mean_mean"]
            .as_f64()
            .expect("predictive mean")
            .is_finite()
    );
}
