#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[test]
fn weighted_exact_window_lgcp_matches_the_existing_rectangular_field_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let membership = directory.path().join("membership.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let grid = directory.path().join("grid.csv");
    let window = directory.path().join("window.geojson");
    let exact_output = directory.path().join("exact.json");
    let rectangular_output = directory.path().join("rectangular.json");
    let counts = [2, 4, 8, 16];
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    let mut membership_csv = String::from("event_id,quadrature_node_id\n");
    for (index, count) in counts.into_iter().enumerate() {
        let ix = index % 2;
        let iy = index / 2;
        let covariate = ix as f64 * 2.0 - 1.0;
        for event in 0..count {
            let event_id = format!("c{index}-e{event:02}");
            let x = ix as f64 + (event as f64 + 1.0) / (count as f64 + 1.0);
            let y = iy as f64 + ((event * 3 % count) as f64 + 1.0) / (count as f64 + 1.0);
            writeln!(event_csv, "{event_id},{x:.17},{y:.17},{covariate},0").unwrap();
            writeln!(membership_csv, "{event_id},q-{index}").unwrap();
        }
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(&membership, membership_csv).unwrap();
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nq-0,0.5,0.5,1,-1,0\nq-1,1.5,0.5,1,1,0\nq-2,0.5,1.5,1,-1,0\nq-3,1.5,1.5,1,1,0\n",
    )
    .unwrap();
    fs::write(
        &grid,
        "ix,iy,covariate,offset\n0,0,-1,0\n1,0,1,0\n0,1,-1,0\n1,1,1,0\n",
    )
    .unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,2],[0,0]]]]}"#,
    )
    .unwrap();

    rectangular_command(&events, &grid, &rectangular_output)
        .assert()
        .success();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-arbitrary-window-lgcp",
            "--events",
            events.to_str().unwrap(),
            "--event-membership",
            membership.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--intercept-prior-mean",
            "1",
            "--intercept-prior-sd",
            "1",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "1",
            "--field-amplitude",
            "0.2",
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
            "--maximum-events",
            "30",
            "--maximum-quadrature-nodes",
            "4",
            "--maximum-draw-node-work",
            "8000",
            "--prediction-replicates",
            "8",
            "--prediction-seed",
            "16102",
            "--maximum-predictive-points",
            "10000",
            "--neighbor-radius-um",
            "1.1",
            "--maximum-neighbor-pairs",
            "4",
            "--timeout-seconds",
            "180",
            "--out",
            exact_output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let exact: serde_json::Value =
        serde_json::from_slice(&fs::read(exact_output).unwrap()).unwrap();
    let rectangular: serde_json::Value =
        serde_json::from_slice(&fs::read(rectangular_output).unwrap()).unwrap();
    assert_eq!(
        exact["format"],
        "marklab.bayesian_arbitrary_window_lgcp_fit"
    );
    assert_eq!(exact["fit_state"], "complete");
    assert_eq!(exact["observed_event_count"], 30);
    assert_eq!(exact["quadrature_node_count"], 4);
    assert_eq!(exact["backend"]["name"], "pymc");
    assert_eq!(exact["source_backend"]["name"], "pymc");
    assert_eq!(exact["maximum_quadrature_nodes"], 4);
    assert_eq!(exact["maximum_neighbor_pairs"], 4);
    for parameter in ["intercept", "coefficient"] {
        let difference = (exact["posterior"][parameter]["mean"].as_f64().unwrap()
            - rectangular["posterior"][parameter]["mean"]
                .as_f64()
                .unwrap())
        .abs();
        assert!(difference <= 1e-10, "{parameter} difference={difference}");
    }
    for (exact_cell, rectangular_cell) in exact["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .zip(rectangular["cells"].as_array().unwrap())
    {
        let difference = (exact_cell["latent_effect"]["mean"].as_f64().unwrap()
            - rectangular_cell["latent_effect"]["mean"].as_f64().unwrap())
        .abs();
        assert!(difference <= 1e-10, "latent difference={difference}");
    }
    assert_eq!(exact["model"]["window"], "exact_multipolygon");
    assert_eq!(
        exact["model"]["kernel"],
        "fixed_matern_3_2_on_physical_quadrature_representatives"
    );
    assert_eq!(exact["spatial_posterior_predictive"]["replicate_count"], 8);
    assert_eq!(
        exact["spatial_posterior_predictive"]["neighbor_pair_count"],
        4
    );
    assert_eq!(
        exact["spatial_posterior_predictive"]["node_density_variance"]["observed"],
        28.75
    );
    assert_eq!(
        exact["spatial_posterior_predictive"]["neighbor_density_mean_absolute_difference"]
            ["observed"],
        7.0
    );
    assert_eq!(
        exact["claim_status"],
        "experimental_single_pattern_latent_field"
    );
}

fn rectangular_command(events: &Path, grid: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
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
        "0.2",
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
    ]);
    command
}
