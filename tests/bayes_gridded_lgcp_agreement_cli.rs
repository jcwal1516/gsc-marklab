#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_the_same_typed_gridded_lgcp() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    let output = directory.path().join("agreement.json");
    let counts = [2, 5, 4, 9];
    let mut events_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for iy in 0..2 {
        for ix in 0..2 {
            let cell_index = iy * 2 + ix;
            let count = counts[cell_index];
            let covariate = ix as f64 - 0.5;
            for event in 0..count {
                let x = ix as f64 + (event as f64 + 1.0) / (count as f64 + 1.0);
                let y = iy as f64 + ((event * 3 % count) as f64 + 1.0) / (count as f64 + 1.0);
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
    for iy in 0..2 {
        for ix in 0..2 {
            writeln!(grid_csv, "{ix},{iy},{},0", ix as f64 - 0.5).expect("grid row");
        }
    }
    fs::write(&grid, grid_csv).expect("grid");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "gridded-lgcp-agreement",
            "--events",
            events.to_str().expect("events path"),
            "--grid",
            grid.to_str().expect("grid path"),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "2",
            "--ymax-um",
            "2",
            "--grid-x",
            "2",
            "--grid-y",
            "2",
            "--intercept-prior-mean",
            "1",
            "--intercept-prior-sd",
            "1",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "1",
            "--field-amplitude",
            "0.35",
            "--field-length-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "1500",
            "--target-accept",
            "0.95",
            "--seed",
            "20260827",
            "--maximum-standardized-difference",
            "5",
            "--minimum-parameter-tolerance",
            "0.08",
            "--minimum-field-tolerance",
            "0.12",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_gridded_lgcp_cross_backend_agreement"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    assert_eq!(result["numpyro"]["backend"]["version"], "0.21.0");
    assert_eq!(result["pymc"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["numpyro"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["comparison"]["intercept"]["passes"], true);
    assert_eq!(result["comparison"]["coefficient"]["passes"], true);
    assert_eq!(result["comparison"]["latent_effect"]["passes"], true);
    assert_eq!(result["comparison"]["expected_count"]["passes"], true);
    assert_eq!(result["comparison"]["cell_count"], 4);
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_validation"
    );
}
