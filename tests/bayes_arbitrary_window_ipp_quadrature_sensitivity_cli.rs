#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn exact_window_ipp_compares_three_area_conserving_quadrature_resolutions() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let window = directory.path().join("window.geojson");
    let coarse = directory.path().join("coarse.csv");
    let baseline = directory.path().join("baseline.csv");
    let fine = directory.path().join("fine.csv");
    let output = directory.path().join("sensitivity.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for index in 0..10 {
        writeln!(event_csv, "left-{index:02},0.25,0.25,-1,0").unwrap();
    }
    for index in 0..40 {
        writeln!(event_csv, "right-{index:02},3.5,0.5,1,0").unwrap();
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,1],[0,0]]],[[[3,0],[4,0],[4,1],[3,1],[3,0]]]]}"#,
    )
    .unwrap();
    write_quadrature(&coarse, 2);
    write_quadrature(&baseline, 4);
    write_quadrature(&fine, 8);

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "arbitrary-window-ipp-quadrature-sensitivity",
            "--events",
            events.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--coarse-quadrature",
            coarse.to_str().unwrap(),
            "--baseline-quadrature",
            baseline.to_str().unwrap(),
            "--fine-quadrature",
            fine.to_str().unwrap(),
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "3",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "2",
            "--chains",
            "2",
            "--tune",
            "500",
            "--draws",
            "1000",
            "--target-accept",
            "0.9",
            "--seed",
            "20260828",
            "--maximum-events",
            "50",
            "--maximum-quadrature-nodes",
            "16",
            "--maximum-total-draw-node-work",
            "56000",
            "--material-standardized-shift",
            "0.75",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.arbitrary_window_ipp_quadrature_sensitivity"
    );
    assert_eq!(result["resolutions"].as_array().unwrap().len(), 3);
    assert_eq!(result["all_fits_complete"], true);
    assert!(result["resolutions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|resolution| resolution["backend"]["name"] == "pymc"
            && resolution["request_sha256"].as_str().unwrap().len() == 64
            && resolution["input"]["quadrature_digest"]
                .as_str()
                .unwrap()
                .len()
                == 64));
    assert_eq!(result["material_change"], false);
    assert!(result["maximum_standardized_shift"].as_f64().unwrap() < 0.1);
    assert_eq!(
        result["claim_status"],
        "experimental_quadrature_sensitivity"
    );
}

fn write_quadrature(path: &std::path::Path, nodes_per_component: usize) {
    let mut csv = String::from("node_id,x_um,y_um,weight_um2,covariate,offset\n");
    for component in ["left", "right"] {
        let covariate = if component == "left" { -1 } else { 1 };
        let x = if component == "left" { 0.5 } else { 3.5 };
        for index in 0..nodes_per_component {
            writeln!(
                csv,
                "{component}-{index:02},{x},0.5,{},{covariate},0",
                1.0 / nodes_per_component as f64
            )
            .unwrap();
        }
    }
    fs::write(path, csv).unwrap();
}
