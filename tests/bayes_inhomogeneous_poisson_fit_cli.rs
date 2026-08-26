#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn fitted_rectangle_process_recovers_positive_covariate_effect() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let output = directory.path().join("fit.json");
    let covariates = [-1.0, -1.0 / 3.0, 1.0 / 3.0, 1.0];
    let counts = [4, 7, 14, 27];
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for (cell, (&covariate, &count)) in covariates.iter().zip(&counts).enumerate() {
        for event in 0..count {
            let x = cell as f64 + (event as f64 + 1.0) / (count as f64 + 1.0);
            let y = (event as f64 + 1.0) / (count as f64 + 1.0);
            writeln!(
                event_csv,
                "c{cell}e{event},{x:.17},{y:.17},{covariate:.17},0"
            )
            .expect("event row");
        }
    }
    fs::write(&events, event_csv).expect("events");
    let mut quadrature_csv = String::from("ix,iy,covariate,offset\n");
    for (ix, covariate) in covariates.iter().enumerate() {
        writeln!(quadrature_csv, "{ix},0,{covariate:.17},0").expect("quadrature row");
    }
    fs::write(&quadrature, quadrature_csv).expect("quadrature");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-inhomogeneous-poisson",
            "--events",
            events.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "4",
            "--ymax-um",
            "1",
            "--grid-x",
            "4",
            "--grid-y",
            "1",
            "--intercept-prior-mean",
            "2",
            "--intercept-prior-sd",
            "2",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "2",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "15101",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("IPP fit JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_inhomogeneous_poisson_fit"
    );
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["observed_event_count"], 52);
    assert!(result["posterior"]["coefficient"]["mean"].as_f64().unwrap() > 0.5);
    assert_eq!(result["cells"].as_array().unwrap().len(), 4);
    assert!(result["cells"].as_array().unwrap().iter().all(|cell| {
        cell["intensity"]["mean"].as_f64().unwrap() > 0.0
            && cell["expected_count"]["mean"].as_f64().unwrap() > 0.0
    }));
    assert!(
        result["posterior_predictive"]["replicated_total_mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(result["claim_status"], "experimental");
}
