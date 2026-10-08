#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn lag_and_error_likelihoods_match_two_region_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let regions = directory.path().join("regions.csv");
    fs::write(&regions, "region_id\na\nb\n").expect("regions");
    let edges = directory.path().join("edges.csv");
    fs::write(&edges, "source_region,target_region,weight\na,b,1\nb,a,1\n").expect("edges");
    let data = directory.path().join("data.csv");
    fs::write(&data, "region_id,y,x\na,1,0\nb,2,1\n").expect("data");
    let coefficients = directory.path().join("coefficients.csv");
    fs::write(
        &coefficients,
        "predictor,coefficient\nintercept,0.5\nx,0.75\n",
    )
    .expect("coefficients");
    let lag = directory.path().join("lag.json");
    let error = directory.path().join("error.json");

    for (model, output) in [("lag", &lag), ("error", &error)] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "bayes",
                "sar-likelihood",
                "--regions",
                regions.to_str().unwrap(),
                "--edges",
                edges.to_str().unwrap(),
                "--data",
                data.to_str().unwrap(),
                "--coefficients",
                coefficients.to_str().unwrap(),
                "--model",
                model,
                "--rho",
                "0.25",
                "--sigma",
                "1.2",
                "--interpretation",
                "descriptive",
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    let lag: serde_json::Value = serde_json::from_slice(&fs::read(lag).unwrap()).expect("lag JSON");
    let error: serde_json::Value =
        serde_json::from_slice(&fs::read(error).unwrap()).expect("error JSON");
    assert_eq!(lag["format"], "marklab.sar_likelihood");
    assert_eq!(lag["model_type"], "lag");
    assert!((lag["log_abs_determinant"].as_f64().unwrap() - 0.9375_f64.ln()).abs() <= 1e-12);
    assert!((lag["residual_sum_squares"].as_f64().unwrap() - 0.25).abs() <= 1e-12);
    assert!((lag["log_likelihood"].as_f64().unwrap() + 2.353864256690381).abs() <= 1e-12);
    assert_eq!(lag["impacts"][0]["predictor"], "x");
    assert!((lag["impacts"][0]["direct"].as_f64().unwrap() - 0.8).abs() <= 1e-12);
    assert!((lag["impacts"][0]["indirect"].as_f64().unwrap() - 0.2).abs() <= 1e-12);
    assert!((lag["impacts"][0]["total"].as_f64().unwrap() - 1.0).abs() <= 1e-12);
    assert_eq!(error["model_type"], "error");
    assert!((error["residual_sum_squares"].as_f64().unwrap() - 0.48828125).abs() <= 1e-12);
    assert!((error["log_likelihood"].as_f64().unwrap() + 2.4366008018292704).abs() <= 1e-12);
    assert_eq!(error["impacts"], serde_json::json!([]));
    assert_eq!(
        error["claim_status"],
        "experimental_fixed_parameter_likelihood"
    );
}
