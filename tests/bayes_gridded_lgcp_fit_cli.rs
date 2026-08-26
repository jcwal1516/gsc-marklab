#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn dense_gridded_lgcp_recovers_positive_fixed_covariate_effect() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    let output = directory.path().join("lgcp-fit.json");
    let counts = [2, 5, 12];
    let mut events_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for iy in 0..3 {
        for (ix, &count) in counts.iter().enumerate() {
            let covariate = ix as f64 - 1.0;
            for event in 0..count {
                let x = ix as f64 + (event as f64 + 1.0) / (count as f64 + 1.0);
                let y = iy as f64 + ((event * 7 % count) as f64 + 1.0) / (count as f64 + 1.0);
                writeln!(
                    events_csv,
                    "r{iy}c{ix}e{event},{x:.17},{y:.17},{covariate},0"
                )
                .expect("event row");
            }
        }
    }
    fs::write(&events, events_csv).expect("events");
    let mut grid_csv = String::from("ix,iy,covariate,offset\n");
    for iy in 0..3 {
        for ix in 0..3 {
            writeln!(grid_csv, "{ix},{iy},{},0", ix as f64 - 1.0).expect("grid row");
        }
    }
    fs::write(&grid, grid_csv).expect("grid");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-gridded-lgcp",
            "--events",
            events.to_str().unwrap(),
            "--grid",
            grid.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "3",
            "--ymax-um",
            "3",
            "--grid-x",
            "3",
            "--grid-y",
            "3",
            "--intercept-prior-mean",
            "1.5",
            "--intercept-prior-sd",
            "1",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "1",
            "--field-amplitude",
            "0.4",
            "--field-length-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "16101",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("LGCP fit JSON");
    assert_eq!(result["format"], "marklab.bayesian_gridded_lgcp_fit");
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["observed_event_count"], 57);
    assert!(result["posterior"]["coefficient"]["mean"].as_f64().unwrap() > 0.3);
    let cells = result["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 9);
    assert!(cells
        .iter()
        .all(|cell| cell["intensity"]["mean"].as_f64().unwrap() > 0.0));
    let latent_means = cells
        .iter()
        .map(|cell| cell["latent_effect"]["mean"].as_f64().unwrap())
        .collect::<Vec<_>>();
    assert!(
        latent_means
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            - latent_means.iter().copied().fold(f64::INFINITY, f64::min)
            > 0.01
    );
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
