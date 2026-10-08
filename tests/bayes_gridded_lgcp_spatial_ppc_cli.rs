#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn gridded_lgcp_reports_spatial_cell_dispersion_and_neighbor_contrast() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    let output = directory.path().join("spatial-ppc.json");
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
            "gridded-lgcp-spatial-ppc",
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
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260827",
            "--replicates",
            "20",
            "--prediction-seed",
            "1729",
            "--maximum-total-points",
            "10000",
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
        "marklab.bayesian_gridded_lgcp_spatial_ppc"
    );
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["replicate_count"], 20);
    assert_eq!(result["neighbor_pair_count"], 4);
    for name in ["cell_count_variance", "adjacent_mean_absolute_difference"] {
        let summary = &result["summaries"][name];
        assert!(summary["observed"].as_f64().expect("observed").is_finite());
        assert!(summary["replicated_mean"]
            .as_f64()
            .expect("replicated")
            .is_finite());
        assert!((0.0..=1.0).contains(
            &summary["probability_replicated_at_least_observed"]
                .as_f64()
                .expect("tail")
        ));
    }
    assert_eq!(
        result["prediction"]["patterns"]
            .as_array()
            .expect("patterns")
            .len(),
        20
    );
    assert_eq!(
        result["claim_status"],
        "experimental_discretized_spatial_posterior_predictive"
    );
}
