#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn poisson_log_rate_laplace_matches_mode_and_hessian_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("counts.csv");
    fs::write(
        &input,
        "observation_id,count,exposure\na,0,1\nb,1,1\nc,2,1\nd,3,1\ne,4,1\n",
    )
    .expect("input");
    let output = directory.path().join("laplace.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "poisson-log-rate-laplace",
            "--input",
            input.to_str().unwrap(),
            "--prior-mean",
            "0",
            "--prior-sd",
            "1",
            "--initial-log-rate",
            "0",
            "--max-iterations",
            "1000",
            "--gradient-tolerance",
            "0.0000000001",
            "--seed",
            "11101",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("Laplace JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_poisson_log_rate_laplace"
    );
    assert_eq!(result["fit_state"], "approximate_only");
    assert!((result["mode"]["log_rate"].as_f64().unwrap() - 0.6282607821567117).abs() <= 1e-10);
    assert!(
        (result["hessian"]["negative_hessian"].as_f64().unwrap() - 10.371739217843288).abs()
            <= 1e-9
    );
    assert!((result["hessian"]["variance"].as_f64().unwrap() - 0.09641584492209603).abs() <= 1e-10);
    assert!(result["optimizer"]["gradient_norm"].as_f64().unwrap() <= 1e-10);
    assert!(
        (result["rate_approximation"]["mean"].as_f64().unwrap() - 1.966919679609662).abs() <= 1e-9
    );
    assert_eq!(result["claim_status"], "experimental_approximate_only");
}
