#![cfg(feature = "cli")]

use std::{f64::consts::LN_2, fs};

use assert_cmd::Command;

#[test]
fn complete_rectangle_midpoint_grid_matches_constant_intensity_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let output = directory.path().join("likelihood.json");
    fs::write(
        &events,
        "event_id,x_um,y_um,covariate,offset\na,0.25,0.25,0,0\nb,1.75,0.75,0,0\n",
    )
    .expect("events");
    fs::write(&quadrature, "ix,iy,covariate,offset\n0,0,0,0\n1,0,0,0\n").expect("quadrature");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "inhomogeneous-poisson-likelihood",
            "--events",
            events.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "2",
            "--ymax-um",
            "1",
            "--grid-x",
            "2",
            "--grid-y",
            "1",
            "--intercept",
            "0.6931471805599453",
            "--coefficient",
            "0",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("likelihood JSON");
    assert_eq!(result["format"], "marklab.inhomogeneous_poisson_likelihood");
    assert_eq!(result["event_count"], 2);
    assert_eq!(result["quadrature_node_count"], 2);
    assert!((result["window_area_um2"].as_f64().unwrap() - 2.0).abs() <= 1e-12);
    assert!((result["event_term"].as_f64().unwrap() - 2.0 * LN_2).abs() <= 1e-12);
    assert!((result["integral_term"].as_f64().unwrap() - 4.0).abs() <= 1e-12);
    assert!((result["log_likelihood"].as_f64().unwrap() - (2.0 * LN_2 - 4.0)).abs() <= 1e-12);
    assert_eq!(result["claim_status"], "experimental_fixed_likelihood");
}
