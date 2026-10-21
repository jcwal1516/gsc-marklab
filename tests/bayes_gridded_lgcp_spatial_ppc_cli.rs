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
    let patterns = result["prediction"]["patterns"]
        .as_array()
        .expect("patterns");
    assert_eq!(patterns.len(), 20);
    let replicated: Vec<[f64; 2]> = patterns
        .iter()
        .map(|pattern| {
            let cells = pattern["cell_counts"].as_array().expect("counts");
            assert_eq!(cells.len(), 4);
            let c: Vec<f64> = cells
                .iter()
                .map(|value| value.as_u64().unwrap() as f64)
                .collect();
            // For four cells, population variance is the sum of all six squared
            // pair differences divided by 16, independently of the mean-based calculation.
            let variance = ((c[0] - c[1]).powi(2)
                + (c[0] - c[2]).powi(2)
                + (c[0] - c[3]).powi(2)
                + (c[1] - c[2]).powi(2)
                + (c[1] - c[3]).powi(2)
                + (c[2] - c[3]).powi(2))
                / 16.0;
            let contrast = ((c[0] - c[1]).abs()
                + (c[0] - c[2]).abs()
                + (c[1] - c[3]).abs()
                + (c[2] - c[3]).abs())
                / 4.0;
            [variance, contrast]
        })
        .collect();
    for (index, (name, observed)) in [
        ("cell_count_variance", 7.5),
        ("adjacent_mean_absolute_difference", 3.5),
    ]
    .into_iter()
    .enumerate()
    {
        let summary = &result["summaries"][name];
        let mean = replicated.iter().map(|row| row[index]).sum::<f64>() / 20.0;
        let sd = (replicated
            .iter()
            .map(|row| (row[index] - mean).powi(2))
            .sum::<f64>()
            / 19.0)
            .sqrt();
        let probability = replicated
            .iter()
            .filter(|row| row[index] >= observed)
            .count() as f64
            / 20.0;
        for (field, expected) in [
            ("observed", observed),
            ("replicated_mean", mean),
            ("replicated_sd", sd),
            ("probability_replicated_at_least_observed", probability),
        ] {
            assert!(
                (summary[field].as_f64().unwrap() - expected).abs() < 1e-12,
                "{name}, {field}"
            );
        }
    }
    assert_eq!(
        result["claim_status"],
        "experimental_discretized_spatial_posterior_predictive"
    );
}
