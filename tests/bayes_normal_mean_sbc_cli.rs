#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn conjugate_normal_mean_sbc_has_uniform_ranks_and_deterministic_output() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("sbc.json");
    let repeated = directory.path().join("sbc-repeated.json");
    for path in [&output, &repeated] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "bayes",
                "normal-mean-sbc",
                "--prior-mean",
                "0",
                "--prior-sd",
                "1",
                "--known-sigma",
                "1",
                "--observations-per-replicate",
                "4",
                "--replicates",
                "500",
                "--posterior-draws",
                "200",
                "--seed",
                "14101",
                "--timeout-seconds",
                "60",
                "--out",
                path.to_str().unwrap(),
            ])
            .assert()
            .success();
    }
    assert_eq!(fs::read(&output).unwrap(), fs::read(&repeated).unwrap());

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("SBC JSON");
    assert_eq!(result["format"], "marklab.bayesian_normal_mean_sbc");
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 500);
    assert!(result["failures"].as_array().unwrap().is_empty());
    assert!(result["replicates"]
        .as_array()
        .unwrap()
        .iter()
        .all(|replicate| replicate["rank"].as_u64().unwrap() <= 200));
    assert_eq!(
        result["diagnostics"]["rank_histogram"]
            .as_array()
            .unwrap()
            .iter()
            .map(|count| count.as_u64().unwrap())
            .sum::<u64>(),
        500
    );
    assert!(
        result["diagnostics"]["rank_uniformity_p_value"]
            .as_f64()
            .unwrap()
            > 0.01
    );
    let coverage = result["diagnostics"]["coverage_95"].as_f64().unwrap();
    assert!((0.90..=0.99).contains(&coverage));
    assert!(
        result["diagnostics"]["z_score_mean"]
            .as_f64()
            .unwrap()
            .abs()
            <= 0.15
    );
    assert!((result["diagnostics"]["z_score_sd"].as_f64().unwrap() - 1.0).abs() <= 0.15);
    assert!((result["diagnostics"]["mean_shrinkage"].as_f64().unwrap() - 0.8).abs() <= 1e-12);
    assert_eq!(result["diagnostics"]["posterior_draws_exchangeable"], true);
    assert_eq!(result["claim_status"], "experimental_calibration_procedure");
}
