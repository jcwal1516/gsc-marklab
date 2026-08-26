#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn rectangle_grid_assigns_events_and_builds_positive_matern_covariance() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    let output = directory.path().join("lgcp-model.json");
    fs::write(
        &events,
        "event_id,x_um,y_um,covariate,offset\na,0.25,0.25,-1,0\nb,1.75,1.75,1,0\n",
    )
    .expect("events");
    let mut grid_csv = String::from("ix,iy,covariate,offset\n");
    for iy in 0..2 {
        for ix in 0..2 {
            let covariate = (iy * 2 + ix) as f64 - 1.5;
            writeln!(grid_csv, "{ix},{iy},{covariate},0").expect("grid row");
        }
    }
    fs::write(&grid, grid_csv).expect("grid");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "build-gridded-lgcp",
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
            "0",
            "--intercept-prior-sd",
            "2",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "2",
            "--field-amplitude",
            "1.5",
            "--field-length-scale-um",
            "2",
            "--jitter",
            "0.000001",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("LGCP model JSON");
    assert_eq!(result["format"], "marklab.gridded_lgcp_model");
    assert_eq!(
        result["model"]["family"],
        "gridded_log_gaussian_cox_process"
    );
    assert_eq!(result["event_count"], 2);
    assert_eq!(result["cells"].as_array().unwrap().len(), 4);
    assert_eq!(
        result["cells"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cell| cell["count"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 0, 0, 1]
    );
    assert!(result["cells"]
        .as_array()
        .unwrap()
        .iter()
        .all(|cell| { (cell["area_um2"].as_f64().unwrap() - 1.0).abs() <= 1e-12 }));
    let covariance = result["field_covariance"].as_array().unwrap();
    assert_eq!(covariance.len(), 16);
    for index in 0..4 {
        assert!((covariance[index * 4 + index].as_f64().unwrap() - 2.250001).abs() <= 1e-12);
    }
    assert_eq!(result["covariance_positive_definite"], true);
    assert_eq!(result["claim_status"], "experimental_model_construction");
}
