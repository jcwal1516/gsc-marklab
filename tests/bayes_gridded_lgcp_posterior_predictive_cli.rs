#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[test]
fn seeded_lgcp_posterior_patterns_are_exactly_cell_bounded_and_repeatable() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    let first = directory.path().join("patterns-first.json");
    let second = directory.path().join("patterns-second.json");
    write_fixture(&events, &grid);

    run(&events, &grid, &first);
    run(&events, &grid, &second);
    let first_bytes = fs::read(&first).expect("first predictive artifact");
    assert_eq!(first_bytes, fs::read(&second).expect("second artifact"));
    let result: serde_json::Value =
        serde_json::from_slice(&first_bytes).expect("LGCP predictive JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_gridded_lgcp_posterior_predictive"
    );
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(
        result["claim_status"],
        "experimental_discretized_prediction"
    );
    let patterns = result["patterns"].as_array().expect("patterns");
    assert_eq!(patterns.len(), 8);
    let mut totals = Vec::new();
    for (replicate, pattern) in patterns.iter().enumerate() {
        assert_eq!(pattern["replicate"].as_u64().unwrap(), replicate as u64);
        let counts = pattern["cell_counts"].as_array().expect("cell counts");
        assert_eq!(counts.len(), 9);
        let total = pattern["total_count"].as_u64().unwrap();
        assert_eq!(
            counts
                .iter()
                .map(|value| value.as_u64().unwrap())
                .sum::<u64>(),
            total
        );
        let points = pattern["points"].as_array().expect("points");
        assert_eq!(points.len() as u64, total);
        for point in points {
            let x = point["x_um"].as_f64().unwrap();
            let y = point["y_um"].as_f64().unwrap();
            let ix = point["ix"].as_u64().unwrap();
            let iy = point["iy"].as_u64().unwrap();
            assert!((0.0..3.0).contains(&x));
            assert!((0.0..3.0).contains(&y));
            assert_eq!(ix, x.floor() as u64);
            assert_eq!(iy, y.floor() as u64);
        }
        totals.push(total);
    }
    assert!(totals.iter().sum::<u64>() <= 100_000);
    assert!(totals.windows(2).any(|pair| pair[0] != pair[1]));
    assert_eq!(
        result["approximation"]["location_rule"],
        "uniform_within_exact_full_cell"
    );
    assert_eq!(
        result["approximation"]["continuous_intensity_bound"],
        "not_available"
    );
}

fn write_fixture(events: &Path, grid: &Path) {
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
    fs::write(events, events_csv).expect("events");
    let mut grid_csv = String::from("ix,iy,covariate,offset\n");
    for iy in 0..3 {
        for ix in 0..3 {
            writeln!(grid_csv, "{ix},{iy},{},0", ix as f64 - 1.0).expect("grid row");
        }
    }
    fs::write(grid, grid_csv).expect("grid");
}

fn run(events: &Path, grid: &Path, output: &Path) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "simulate-gridded-lgcp-posterior-predictive",
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
            "--replicates",
            "8",
            "--prediction-seed",
            "16102",
            "--maximum-total-points",
            "100000",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}
