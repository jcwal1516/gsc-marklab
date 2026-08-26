#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn latent_parent_exchange_multitype_and_replicated_cluster_models_run_as_one_consumed_flow() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("clusters.json");
    let mut patterns = Vec::new();
    for patient in 0..6 {
        let mut points = Vec::new();
        for parent in 0..3 {
            let cx = 0.2 + 0.3 * parent as f64 + 0.01 * patient as f64;
            let cy = 0.25 + 0.2 * ((parent + patient) % 3) as f64;
            for child in 0..8 {
                let dx = ((child * 7 + patient) % 9) as f64 / 180.0 - 0.025;
                let dy = ((child * 5 + 2 * patient) % 11) as f64 / 220.0 - 0.025;
                points.push(serde_json::json!([cx + dx, cy + dy]));
            }
        }
        patterns.push(serde_json::json!({"pattern_id":format!("p{patient}"),"patient_id":format!("p{patient}"),"window":[0.0,1.0,0.0,1.0],"points":points}));
    }
    let fixture = serde_json::json!({
        "patterns":patterns,"cluster_family":"thomas_gaussian","maximum_parents":8,
        "latent_iterations":1200,"latent_burnin":400,
        "finite_gibbs":{"sites":6,"edges":[[0,1],[1,2],[2,3],[3,4],[4,5]],
            "observed_types":[0,0,1,1,0,1],"interaction_prior_sd":1.5,
            "proposal_sd":0.25,"draws":1600,"burnin":400},
        "hierarchical_shrinkage":2.0,"seed":311,"timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "advanced-cluster",
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
        "marklab.advanced_cluster_and_gibbs_models"
    );
    assert!(
        (result["latent_parent_model"]["posterior_parent_count_mean"]
            .as_f64()
            .unwrap()
            - 3.0)
            .abs()
            < 0.7
    );
    assert!(
        result["latent_parent_model"]["posterior_scale_mean"]
            .as_f64()
            .unwrap()
            < 0.08
    );
    assert_eq!(
        result["exchange_mcmc"]["auxiliary_draw_status"],
        "exact_finite_state_enumeration"
    );
    assert_eq!(result["exchange_mcmc"]["state_space_size"], 64);
    assert!(result["exchange_mcmc"]["acceptance_rate"].as_f64().unwrap() > 0.1);
    assert_eq!(
        result["multitype_gibbs"]["method"],
        "exact_finite_state_exchange_mcmc"
    );
    assert_eq!(result["replicated_cluster_model"]["patient_count"], 6);
    assert!(
        result["replicated_cluster_model"]["partial_pooling_applied"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_cluster_and_finite_gibbs_inference"
    );
}
