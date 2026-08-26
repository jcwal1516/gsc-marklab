#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn nested_laplace_grid_normalizes_and_agrees_with_nuts() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("counts.csv");
    fs::write(
        &input,
        "region_id,count,exposure\na,2,5\nb,4,5\nc,8,5\nd,3,5\ne,10,5\n",
    )
    .expect("input");
    let output = directory.path().join("inla.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "poisson-lognormal-inla",
            "--input",
            input.to_str().unwrap(),
            "--latent-mean",
            "0",
            "--tau-shape",
            "2",
            "--tau-rate",
            "1",
            "--log-tau-min",
            "-4",
            "--log-tau-max",
            "4",
            "--grid-points",
            "81",
            "--endpoint-mass-limit",
            "0.05",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "12101",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("INLA JSON");
    assert_eq!(result["format"], "marklab.bayesian_poisson_lognormal_inla");
    assert_eq!(result["fit_state"], "approximate_only");
    assert_eq!(result["grid"].as_array().unwrap().len(), 81);
    let weight_sum = result["grid"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["normalized_weight"].as_f64().unwrap())
        .sum::<f64>();
    assert!((weight_sum - 1.0).abs() <= 1e-12);
    assert!(
        result["grid_diagnostics"]["left_endpoint_mass"]
            .as_f64()
            .unwrap()
            <= 0.05
    );
    assert!(
        result["grid_diagnostics"]["right_endpoint_mass"]
            .as_f64()
            .unwrap()
            <= 0.05
    );
    assert!(result["tau"]["mean"].as_f64().unwrap() > 0.0);
    assert_eq!(result["latent_marginals"].as_array().unwrap().len(), 5);
    assert!(
        result["hmc_comparison"]["latent_mean_rmse"]
            .as_f64()
            .unwrap()
            <= 0.15
    );
    assert_eq!(result["hmc_comparison"]["fit_state"], "complete");
    assert_eq!(result["hmc_comparison"]["divergences"], 0);
    assert_eq!(result["hmc_comparison"]["max_tree_depth_hits"], 0);
    assert_eq!(result["claim_status"], "experimental_approximate_only");
}
