#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_weighted_exact_window_ipp() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("agreement.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for index in 0..10 {
        writeln!(event_csv, "left-{index:02},0.25,0.25,-1,0").unwrap();
    }
    for index in 0..40 {
        writeln!(event_csv, "right-{index:02},3.5,0.5,1,0").unwrap();
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nleft-a,0.25,0.5,0.5,-1,0\nleft-b,0.75,0.5,0.5,-1,0\nright-a,3.25,0.5,0.5,1,0\nright-b,3.75,0.5,0.5,1,0\n",
    )
    .unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,1],[0,0]]],[[[3,0],[4,0],[4,1],[3,1],[3,0]]]]}"#,
    )
    .unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "arbitrary-window-ipp-agreement",
            "--events",
            events.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
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
            "4",
            "--maximum-draw-node-work",
            "8000",
            "--parameter-absolute-tolerance",
            "0.15",
            "--total-count-absolute-tolerance",
            "5",
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
        "marklab.arbitrary_window_ipp_backend_agreement"
    );
    assert_eq!(result["pymc"]["backend"]["version"], "6.3.0");
    assert_eq!(result["numpyro"]["backend"]["version"], "0.21.0");
    assert!(
        result["pymc"]["posterior"]["coefficient"]["mean"]
            .as_f64()
            .unwrap()
            > 0.5
    );
    assert!(
        result["numpyro"]["posterior"]["coefficient"]["mean"]
            .as_f64()
            .unwrap()
            > 0.5
    );
    assert_eq!(result["comparison"]["all_within_tolerance"], true);
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_agreement"
    );
}
