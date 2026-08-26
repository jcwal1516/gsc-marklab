#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn fixture() -> serde_json::Value {
    let patterns = (0..16)
        .map(|patient| {
            let context = if (patient as usize).is_multiple_of(2) {
                -1.0
            } else {
                1.0
            };
            let points = (0..4)
                .map(|index| {
                    let center = if context < 0.0 { 0.3 } else { 0.7 };
                    [center + 0.04 * (index as f64 - 1.5), 0.25 + 0.15 * index as f64 + 0.01 * (patient % 3) as f64]
                })
                .collect::<Vec<_>>();
            serde_json::json!({"pattern_id":format!("p{patient:02}"),"patient_id":format!("p{patient:02}"),"split":if patient<12{"train"}else{"test"},"window":[0.0,1.0,0.0,1.0],"context":[context],"points":points})
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "patterns":patterns,
        "flow":{"family":"conditional_logistic_normal_iid_equivariant","boundary_epsilon":0.0001,"variance_floor":0.01},
        "diffusion":{"schedule":"variance_preserving_linear_beta","steps":64,"beta_min":0.1,"beta_max":8.0},
        "generated_patterns_per_context":16,
        "seed":89,
        "timeout_seconds":60
    })
}

#[test]
fn generative_model_card_consumes_repeated_artifacts_and_records_supported_and_missing_evidence() {
    let directory = tempfile::tempdir().expect("tempdir");
    let data = directory.path().join("data.json");
    fs::write(&data, serde_json::to_vec_pretty(&fixture()).unwrap()).expect("fixture");
    let model = directory.path().join("model.json");
    let repeat = directory.path().join("model_repeat.json");
    for output in [&model, &repeat] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "neural",
                "point-set-generators",
                "--input",
                data.to_str().unwrap(),
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();
    }
    assert_eq!(fs::read(&model).unwrap(), fs::read(&repeat).unwrap());
    let output = directory.path().join("validation.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "neural",
            "validate-generative",
            "--data",
            data.to_str().unwrap(),
            "--model",
            model.to_str().unwrap(),
            "--model-repeat",
            repeat.to_str().unwrap(),
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.generative_tissue_model_card");
    assert_eq!(result["reproducibility"]["byte_identical_repeat"], true);
    assert!(
        result["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["status"] == "passed")
            .count()
            >= 8
    );
    assert!(result["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["validation_id"] == "graph_motif_topology"
            && entry["status"] == "not_supported_by_point_only_fixture"));
    assert_eq!(result["maturity_decision"], "research_only_synthetic");
    assert_eq!(
        result["claim_status"],
        "synthetic_generative_model_validation_no_real_promotion"
    );
}
