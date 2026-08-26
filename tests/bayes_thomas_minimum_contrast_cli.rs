#![cfg(feature = "cli")]

use std::{f64::consts::PI, fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn exact_thomas_k_curve_recovers_parent_intensity_scale_and_offspring_mean() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("thomas-k.csv");
    let output = directory.path().join("thomas-fit.json");
    let kappa = 0.002;
    let sigma = 10.0;
    let mut csv = String::from("radius_um,observed_k_um2,weight\n");
    for index in 1..=10 {
        let radius = 5.0 * index as f64;
        let k = PI * radius * radius
            + (1.0 / kappa) * (1.0 - (-radius * radius / (4.0 * sigma * sigma)).exp());
        writeln!(csv, "{radius:.17},{k:.17},{}", 1.0 / radius).expect("K row");
    }
    fs::write(&input, csv).expect("input K curve");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-thomas-minimum-contrast",
            "--input",
            input.to_str().unwrap(),
            "--observed-intensity-per-um2",
            "0.02",
            "--kappa-min-per-um2",
            "0.0005",
            "--kappa-max-per-um2",
            "0.01",
            "--sigma-min-um",
            "2",
            "--sigma-max-um",
            "30",
            "--maximum-iterations",
            "1000",
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("minimum-contrast JSON");
    assert_eq!(result["format"], "marklab.thomas_minimum_contrast_fit");
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["likelihood_comparison"]["state"], "unavailable");
    let primary = &result["primary_fit"];
    assert!((primary["kappa_parent_per_um2"].as_f64().unwrap() - kappa).abs() < 1e-7);
    assert!((primary["sigma_um"].as_f64().unwrap() - sigma).abs() < 1e-5);
    assert!((primary["mu_offspring"].as_f64().unwrap() - 10.0).abs() < 1e-3);
    assert!(primary["objective"].as_f64().unwrap() < 1e-16);
    for fit in result["sensitivity_fits"].as_array().unwrap() {
        assert!((fit["kappa_parent_per_um2"].as_f64().unwrap() - kappa).abs() < 1e-6);
        assert!((fit["sigma_um"].as_f64().unwrap() - sigma).abs() < 1e-4);
    }
    let curve = result["curve"].as_array().unwrap();
    assert_eq!(curve.len(), 10);
    assert!(curve.iter().all(|row| {
        (row["observed_k_um2"].as_f64().unwrap() - row["fitted_k_um2"].as_f64().unwrap()).abs()
            < 1e-6
    }));
    assert_eq!(result["claim_status"], "experimental_minimum_contrast");
}
