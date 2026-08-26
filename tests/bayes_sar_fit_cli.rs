#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn fitted_lag_completes_and_error_mode_reports_diagnostics() {
    let directory = tempfile::tempdir().expect("tempdir");
    let regions = directory.path().join("regions.csv");
    let mut region_csv = String::from("region_id\n");
    for index in 0..8 {
        region_csv.push_str(&format!("r{index}\n"));
    }
    fs::write(&regions, region_csv).expect("regions");
    let edges = directory.path().join("edges.csv");
    let mut edge_csv = String::from("source_region,target_region,weight\n");
    for index in 0..8 {
        edge_csv.push_str(&format!("r{index},r{},1\n", (index + 7) % 8));
        edge_csv.push_str(&format!("r{index},r{},1\n", (index + 1) % 8));
    }
    fs::write(&edges, edge_csv).expect("edges");
    let data = directory.path().join("data.csv");
    fs::write(
        &data,
        "region_id,y,x\n\
r0,-0.09000877394856459,-1\n\
r1,-0.25599346988090443,-0.7\n\
r2,0.5167189747425349,-0.3\n\
r3,0.9007866348311374,0\n\
r4,1.4885252574650478,0.2\n\
r5,1.7560484149358488,0.5\n\
r6,2.351797508773948,0.8\n\
r7,2.1892683102238073,1\n",
    )
    .expect("data");
    let output = directory.path().join("fit.json");
    let error_output = directory.path().join("error-fit.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "sar-fit",
            "--regions",
            regions.to_str().unwrap(),
            "--edges",
            edges.to_str().unwrap(),
            "--data",
            data.to_str().unwrap(),
            "--model",
            "lag",
            "--interpretation",
            "descriptive",
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "3",
            "--coefficient-prior-sd",
            "2",
            "--rho-bound",
            "0.95",
            "--sigma-prior-sd",
            "1",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "7301",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "sar-fit",
            "--regions",
            regions.to_str().unwrap(),
            "--edges",
            edges.to_str().unwrap(),
            "--data",
            data.to_str().unwrap(),
            "--model",
            "error",
            "--interpretation",
            "descriptive",
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "3",
            "--coefficient-prior-sd",
            "2",
            "--rho-bound",
            "0.95",
            "--sigma-prior-sd",
            "1",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "7302",
            "--timeout-seconds",
            "180",
            "--out",
            error_output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("SAR fit JSON");
    assert_eq!(result["format"], "marklab.bayesian_sar_fit");
    assert_eq!(result["fit_state"], "complete");
    let slope = &result["posterior"]["coefficients"][0];
    assert_eq!(slope["predictor"], "x");
    assert!((0.5..=1.8).contains(&slope["mean"].as_f64().unwrap()));
    assert!((0.0..=0.75).contains(&result["posterior"]["rho"]["mean"].as_f64().unwrap()));
    assert_eq!(result["impacts"][0]["predictor"], "x");
    assert!(result["impacts"][0]["direct"]["mean"].as_f64().unwrap() > 0.0);
    assert!(result["impacts"][0]["total"]["mean"].as_f64().unwrap() > 0.0);
    assert!(
        (result["posterior_predictive"]["observed_mean"]
            .as_f64()
            .unwrap()
            - 1.107142857142857)
            .abs()
            <= 1e-12
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(result["claim_status"], "experimental");

    let error_result: serde_json::Value =
        serde_json::from_slice(&fs::read(error_output).unwrap()).expect("error SAR fit JSON");
    assert_eq!(error_result["fit_state"], "nonconverged");
    assert_eq!(error_result["model"]["model_type"], "error");
    assert_eq!(error_result["impacts"], serde_json::json!([]));
    assert_eq!(error_result["diagnostics"]["divergences"], 0);
    assert_eq!(error_result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(error_result["claim_status"], "diagnostic_only_nonconverged");
}
