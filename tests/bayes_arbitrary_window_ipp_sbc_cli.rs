#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn weighted_exact_window_ipp_sbc_calibrates_both_parameters_and_total_intensity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("sbc.json");
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
            "arbitrary-window-ipp-sbc",
            "--events",
            events.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--intercept-prior-mean",
            "3.2",
            "--intercept-prior-sd",
            "0.5",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "0.5",
            "--chains",
            "2",
            "--tune",
            "200",
            "--draws",
            "400",
            "--target-accept",
            "0.9",
            "--seed",
            "20260828",
            "--replicates",
            "20",
            "--maximum-events",
            "50",
            "--maximum-quadrature-nodes",
            "4",
            "--maximum-generated-count",
            "100000",
            "--maximum-total-work",
            "100000",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("result JSON");
    assert_eq!(result["format"], "marklab.arbitrary_window_ipp_sbc");
    assert_eq!(result["completed_replicates"], 20);
    assert_eq!(result["failed_replicates"], 0);
    assert_eq!(result["calibration"]["intercept"]["accepted"], true);
    assert_eq!(result["calibration"]["coefficient"]["accepted"], true);
    assert_eq!(
        result["calibration"]["total_expected_count"]["accepted"],
        true
    );
    assert_eq!(result["calibration_status"], "accepted");
    assert_eq!(
        result["claim_status"],
        "synthetic_prior_generative_calibration_only"
    );
}

#[test]
fn weighted_exact_window_ipp_sbc_reports_prior_draws_above_the_count_ceiling() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("sbc.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for index in 0..20 {
        let covariate = if index < 10 { -1 } else { 1 };
        writeln!(event_csv, "e-{index:02},0.5,0.5,{covariate},0").unwrap();
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nq1,0.25,0.25,0.25,-1,0\nq2,0.75,0.25,0.25,-1,0\nq3,0.25,0.75,0.25,1,0\nq4,0.75,0.75,0.25,1,0\n",
    )
    .unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,1],[0,0]]]]}"#,
    )
    .unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "arbitrary-window-ipp-sbc",
            "--events",
            events.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--intercept-prior-mean",
            "20",
            "--intercept-prior-sd",
            "0.01",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "0.01",
            "--chains",
            "2",
            "--tune",
            "100",
            "--draws",
            "100",
            "--target-accept",
            "0.9",
            "--seed",
            "20260828",
            "--replicates",
            "20",
            "--maximum-events",
            "20",
            "--maximum-quadrature-nodes",
            "4",
            "--maximum-generated-count",
            "1",
            "--maximum-total-work",
            "40000",
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("result JSON");
    assert_eq!(result["completed_replicates"], 0);
    assert_eq!(result["failed_replicates"], 20);
    assert_eq!(result["calibration"]["intercept"]["accepted"], false);
    assert_eq!(result["calibration_status"], "not_accepted");
    assert!(result["failures"]
        .as_array()
        .unwrap()
        .iter()
        .all(|failure| failure["reason"] == "generated_count_resource_ceiling"));
}
