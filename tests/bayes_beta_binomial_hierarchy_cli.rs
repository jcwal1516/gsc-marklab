#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn beta_binomial_hierarchy_recovers_population_and_patient_dispersion() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("fit.json");
    fs::write(
        &input,
        "patient_id,successes,trials\n\
p-1,12,100\n\
p-2,18,100\n\
p-3,22,100\n\
p-4,27,100\n\
p-5,31,100\n\
p-6,35,100\n\
p-7,40,100\n\
p-8,45,100\n",
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-hierarchy",
            "--input",
            input.to_str().unwrap(),
            "--population-alpha",
            "2",
            "--population-beta",
            "2",
            "--concentration-prior-sd",
            "20",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "2000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260827",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.bayesian_beta_binomial_hierarchy");
    assert_eq!(result["fit_state"], "complete", "{result}");
    let mean = result["posterior"]["population_probability"]["mean"]
        .as_f64()
        .unwrap();
    assert!((0.2..0.4).contains(&mean));
    assert!(
        result["posterior"]["concentration"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!((0.0..1.0).contains(&result["posterior"]["overdispersion_mean"].as_f64().unwrap()));
    assert_eq!(result["patients"].as_array().unwrap().len(), 8);
    assert!(result["patients"]
        .as_array()
        .unwrap()
        .iter()
        .all(|patient| {
            patient["posterior_probability"]["mean"].as_f64().unwrap() > 0.0
                && patient["posterior_probability"]["mean"].as_f64().unwrap() < 1.0
        }));
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_non_gaussian_hierarchy"
    );
}
