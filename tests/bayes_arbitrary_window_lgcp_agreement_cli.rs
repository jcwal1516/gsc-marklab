#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_the_same_weighted_physical_lgcp() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let membership = directory.path().join("membership.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("agreement.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    let mut membership_csv = String::from("event_id,quadrature_node_id\n");
    for (index, count) in [2, 4, 8, 16].into_iter().enumerate() {
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
            "arbitrary-window-lgcp-agreement",
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
            "--maximum-standardized-difference",
            "5",
            "--minimum-parameter-tolerance",
            "0.12",
            "--minimum-field-tolerance",
            "0.2",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.arbitrary_window_lgcp_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    assert_eq!(result["numpyro"]["source_backend"]["name"], "numpyro");
    assert_eq!(result["comparison"]["intercept"]["passes"], true);
    assert_eq!(result["comparison"]["coefficient"]["passes"], true);
    assert_eq!(result["comparison"]["latent_effect"]["passes"], true);
    assert_eq!(result["comparison"]["expected_count"]["passes"], true);
    assert_eq!(result["comparison"]["node_count"], 4);
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_validation"
    );
}
