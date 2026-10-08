#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn gridded_lgcp_reports_one_at_a_time_prior_and_kernel_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    let output = directory.path().join("sensitivity.json");
    let counts = [2, 6, 3, 9];
    let mut events_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for iy in 0..2 {
        for ix in 0..2 {
            let count = counts[iy * 2 + ix];
            for event in 0..count {
                let coordinate = (event as f64 + 1.0) / (count as f64 + 1.0);
                writeln!(
                    events_csv,
                    "r{iy}c{ix}e{event},{},{},{},0",
                    ix as f64 + coordinate,
                    iy as f64 + coordinate,
                    ix as f64 - 0.5
                )
                .expect("event row");
            }
        }
    }
    fs::write(&events, events_csv).expect("events");
    fs::write(
        &grid,
        "ix,iy,covariate,offset\n0,0,-0.5,0\n1,0,0.5,0\n0,1,-0.5,0\n1,1,0.5,0\n",
    )
    .expect("grid");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "gridded-lgcp-sensitivity",
            "--events",
            events.to_str().unwrap(),
            "--grid",
            grid.to_str().unwrap(),
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
            "--lower-scale-multiplier",
            "0.5",
            "--upper-scale-multiplier",
            "2",
            "--material-standardized-shift",
            "0.75",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260827",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_gridded_lgcp_sensitivity"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["scenarios"].as_array().unwrap().len(), 9);
    assert_eq!(result["scenarios"][0]["scenario_id"], "baseline");
    assert!(result["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .all(|scenario| {
            scenario["fit_state"] == "complete"
                && scenario["maximum_absolute_standardized_shift"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
        }));
    assert!(matches!(
        result["sensitivity_state"].as_str(),
        Some("stable_within_declared_grid" | "sensitive_within_declared_grid")
    ));
    assert_eq!(
        result["claim_status"],
        "experimental_prior_kernel_sensitivity"
    );
}
