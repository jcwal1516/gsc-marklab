#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn weighted_exact_window_ipp_spatial_ppc_uses_event_node_membership_and_physical_neighbors() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let event_membership = directory.path().join("event-membership.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("spatial-ppc.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    let mut membership_csv = String::from("event_id,quadrature_node_id\n");
    for index in 0..10 {
        writeln!(event_csv, "left-{index:02},0.25,0.5,-1,0").unwrap();
        writeln!(membership_csv, "left-{index:02},left-a").unwrap();
    }
    for index in 0..40 {
        writeln!(event_csv, "right-{index:02},3.25,0.5,1,0").unwrap();
        writeln!(membership_csv, "right-{index:02},right-a").unwrap();
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(&event_membership, membership_csv).unwrap();
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
            "arbitrary-window-ipp-spatial-ppc",
            "--events",
            events.to_str().unwrap(),
            "--event-membership",
            event_membership.to_str().unwrap(),
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
            "--prediction-seed",
            "20260829",
            "--neighbor-radius-um",
            "0.6",
            "--maximum-events",
            "50",
            "--maximum-quadrature-nodes",
            "4",
            "--maximum-neighbor-pairs",
            "4",
            "--maximum-draw-node-work",
            "8000",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("result JSON");
    assert_eq!(result["format"], "marklab.arbitrary_window_ipp_spatial_ppc");
    assert_eq!(result["observed_event_count"], 50);
    assert_eq!(result["quadrature_node_count"], 4);
    assert_eq!(result["neighbor_pair_count"], 2);
    assert_eq!(result["posterior_predictive_replicates"], 2000);
    assert_eq!(
        result["summaries"]["node_density_variance"]["observed"],
        1075.0
    );
    assert_eq!(
        result["summaries"]["neighbor_density_mean_absolute_difference"]["observed"],
        50.0
    );
    for name in [
        "node_density_variance",
        "neighbor_density_mean_absolute_difference",
    ] {
        assert!(result["summaries"][name]["replicated_mean"]
            .as_f64()
            .unwrap()
            .is_finite());
        let probability = result["summaries"][name]["probability_replicated_at_least_observed"]
            .as_f64()
            .unwrap();
        assert!((0.0..=1.0).contains(&probability));
    }
    assert_eq!(
        result["claim_status"],
        "experimental_single_pattern_spatial_ppc"
    );
}
