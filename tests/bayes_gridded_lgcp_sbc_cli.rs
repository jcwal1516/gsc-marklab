#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn numpyro_gridded_lgcp_sbc_has_bounded_field_ranks_and_complete_disposition() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    let output = directory.path().join("sbc.json");
    let mut events_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for iy in 0..2 {
        for ix in 0..2 {
            let count = 2 + ix + iy;
            for event in 0..count {
                let x = ix as f64 + (event as f64 + 1.0) / (count as f64 + 1.0);
                let y = iy as f64 + (event as f64 + 1.0) / (count as f64 + 1.0);
                writeln!(
                    events_csv,
                    "r{iy}c{ix}e{event},{x},{y},{},0",
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
            "gridded-lgcp-sbc",
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
            "0.5",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "0.5",
            "--field-amplitude",
            "0.25",
            "--field-length-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--replicates",
            "20",
            "--chains",
            "2",
            "--tune",
            "750",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260827",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.7",
            "--maximum-coverage-90",
            "1",
            "--timeout-seconds",
            "300",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_gridded_lgcp_sbc");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(
        result["replicates"].as_array().expect("replicates").len(),
        20
    );
    assert!(result["failures"].as_array().expect("failures").is_empty());
    for parameter in ["intercept", "coefficient", "latent_cell"] {
        let diagnostic = &result["diagnostics"][parameter];
        assert_eq!(
            diagnostic["rank_histogram"]
                .as_array()
                .expect("histogram")
                .iter()
                .map(|value| value.as_u64().expect("count"))
                .sum::<u64>(),
            20
        );
        assert!(
            diagnostic["rank_uniformity_p_value"]
                .as_f64()
                .expect("rank p")
                >= 0.001
        );
        assert!((0.7..=1.0).contains(&diagnostic["coverage_90"].as_f64().expect("coverage")));
    }
    assert_eq!(result["calibration"]["latent_cell_index"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_simulation_calibration"
    );
}
