#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn exact_window_lgcp_sbc_calibrates_global_and_physical_field_quantities() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let membership = directory.path().join("membership.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("sbc.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    let mut membership_csv = String::from("event_id,quadrature_node_id\n");
    for (index, count) in [2, 3, 4, 5].into_iter().enumerate() {
        let ix = index % 2;
        let iy = index / 2;
        let covariate = ix as f64 * 2.0 - 1.0;
        for event in 0..count {
            let id = format!("c{index}-e{event:02}");
            writeln!(
                event_csv,
                "{id},{},{},{covariate},0",
                ix as f64 + 0.5,
                iy as f64 + 0.5
            )
            .unwrap();
            writeln!(membership_csv, "{id},q-{index}").unwrap();
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
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,2],[0,0]]]]}"#,
    )
    .unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "arbitrary-window-lgcp-sbc",
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
            "--maximum-events",
            "14",
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
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.7",
            "--maximum-coverage-90",
            "1",
            "--timeout-seconds",
            "300",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.arbitrary_window_lgcp_sbc");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 20);
    assert!(result["failures"].as_array().unwrap().is_empty());
    assert_eq!(result["calibration"]["latent_cell_index"], 0);
    assert_eq!(result["physical_latent_node_id"], "q-0");
    assert_eq!(result["backend"]["name"], "numpyro");
    assert_eq!(result["source_backend"]["name"], "numpyro");
    for parameter in ["intercept", "coefficient", "latent_cell"] {
        assert_eq!(
            result["diagnostics"][parameter]["rank_histogram"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap())
                .sum::<u64>(),
            20
        );
    }
    assert_eq!(
        result["claim_status"],
        "experimental_simulation_calibration"
    );
}
