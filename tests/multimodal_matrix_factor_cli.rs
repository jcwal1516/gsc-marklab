#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pinned_bayesian_matrix_factorization_recovers_masked_rank_one_entries() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("matrix.json");
    let rows = (0..16)
        .map(|index| {
            let u = (index as f64 - 7.5) / 3.0;
            let values = [u, 2.0 * u, -u, 0.5 * u];
            let observed = (0..4)
                .map(|feature| !matches!((index, feature), (12, 0) | (13, 1) | (14, 2) | (15, 3)))
                .collect::<Vec<_>>();
            serde_json::json!({"entity_id":format!("p{index:02}"),"values":values,"observed":observed})
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "matrix_id":"patient_features",
        "entity_level":"patient",
        "likelihood":"gaussian",
        "feature_names":["f1","f2","f3","f4"],
        "rows":rows,
        "factors":2,
        "iterations":300,
        "convergence_mode":"medium",
        "seed":23,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "matrix-factor",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_matrix_factorization");
    assert_eq!(result["backend"]["version"], "mofapy2-0.7.4");
    assert_eq!(result["masked_predictions"].as_array().unwrap().len(), 4);
    assert!(result["masked_rmse"].as_f64().unwrap() < 0.35);
    assert!(result["active_factor_count"].as_u64().unwrap() >= 1);
    assert!(
        result["diagnostics"]["elbo_last"].as_f64().unwrap()
            > result["diagnostics"]["elbo_first"].as_f64().unwrap()
    );
    assert_eq!(result["alignment"]["sign"], "max_loading_positive");
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_bayesian_matrix_factorization"
    );
}
