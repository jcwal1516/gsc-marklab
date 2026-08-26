#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn observational_estimators_cross_fit_clusters_and_detect_overlap_and_negative_controls() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observational.json");
    let mut rows = Vec::new();
    for cluster in 0..12 {
        for unit in 0..24 {
            let x = ((unit * 7 + cluster * 3) % 23) as f64 / 11.0 - 1.0;
            let treatment = ((unit + 2 * cluster) % 5 < 2) as u8;
            let dose = 0.25 + 0.5 * treatment as f64 + 0.2 * x;
            let exposure = ((unit + cluster) % 4) as f64 / 3.0;
            let noise = ((unit * 13 + cluster * 5) % 17) as f64 / 80.0 - 0.1;
            let outcome = 0.4 + 2.0 * dose + 0.75 * exposure + 0.6 * x + noise;
            let negative_control = -0.2 + 0.3 * x + noise;
            rows.push(serde_json::json!({
                "unit_id":format!("c{cluster:02}u{unit:02}"), "cluster_id":format!("c{cluster:02}"),
                "baseline_covariates":[x], "treatment":treatment, "dose":dose,
                "neighbor_exposure":exposure, "outcome":outcome,
                "negative_control_outcome":negative_control
            }));
        }
    }
    let fixture = serde_json::json!({
        "rows":rows, "cluster_folds":4, "propensity_clip":0.02,
        "dose_basis_degree":2, "seed":211, "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "observational",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.synthetic_observational_causal_estimators"
    );
    assert!((result["dose_response"]["linear_effect"].as_f64().unwrap() - 2.0).abs() < 0.12);
    assert!((result["spatial_dml"]["dose_effect"].as_f64().unwrap() - 2.0).abs() < 0.12);
    assert!((result["spatial_dml"]["spillover_effect"].as_f64().unwrap() - 0.75).abs() < 0.12);
    assert!(result["propensity"]["minimum"].as_f64().unwrap() > 0.02);
    assert!(result["propensity"]["maximum"].as_f64().unwrap() < 0.98);
    assert!(
        result["negative_control"]["adjusted_treatment_effect"]
            .as_f64()
            .unwrap()
            .abs()
            < 0.08
    );
    assert_eq!(result["cross_fitting"]["cluster_leakage"], false);
    assert_eq!(
        result["claim_status"],
        "synthetic_observational_estimator_validation_no_identified_real_effect"
    );
}
