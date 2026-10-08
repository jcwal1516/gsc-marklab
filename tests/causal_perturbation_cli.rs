#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn randomized_perturbation_recovers_direct_spillover_and_research_only_mediation_paths() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("perturbation.json");
    let mut rows = Vec::new();
    for cluster in 0..16 {
        let treatment = (cluster % 2) as f64;
        for unit in 0..12 {
            let exposure = ((unit + cluster) % 3) as f64 / 2.0;
            let noise = ((unit * 11 + cluster * 7) % 19) as f64 / 100.0 - 0.09;
            let mediator = 0.3 + 1.5 * treatment + 0.5 * exposure + noise;
            let outcome = -0.2 + 1.0 * treatment + 0.8 * mediator + 0.4 * exposure;
            rows.push(serde_json::json!({
                "unit_id":format!("c{cluster:02}u{unit:02}"), "cluster_id":format!("c{cluster:02}"),
                "treatment":treatment, "neighbor_exposure":exposure, "mediator":mediator,
                "outcome":outcome, "negative_control_outcome":noise
            }));
        }
    }
    let fixture = serde_json::json!({
        "rows":rows, "assignment":"cluster_randomized", "temporal_order":["treatment","mediator","outcome"],
        "mediator_outcome_no_unmeasured_confounding_declared":true,
        "sensitivity_shifts":[-0.2,0.0,0.2], "seed":223, "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "perturbation",
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
        "marklab.synthetic_spatial_perturbation_analysis"
    );
    assert!((result["primary"]["direct_effect"].as_f64().unwrap() - 1.0).abs() < 0.08);
    assert!((result["primary"]["spillover_effect"].as_f64().unwrap() - 0.4).abs() < 0.08);
    assert!((result["mediation"]["indirect_effect"].as_f64().unwrap() - 1.2).abs() < 0.08);
    assert_eq!(
        result["mediation"]["claim_status"],
        "research_only_design_gated_synthetic_decomposition"
    );
    assert_eq!(result["replication"]["independent_cluster_count"], 16);
}
