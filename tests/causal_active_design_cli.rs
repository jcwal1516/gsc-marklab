#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn active_design_selects_budgeted_actions_and_robust_biological_replication() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("active.json");
    let fixture = serde_json::json!({
        "posterior":{"mean":0.0,"variance":1.0,"observation_variance":0.25},
        "candidates":[
            {"id":"roi_a","kind":"roi","cost":2.0,"information":1.0,"quality":1.0,"redundancy":0.0,"robustness":1.0,"coverage":0.9,"observation":0.8},
            {"id":"roi_b","kind":"roi","cost":2.0,"information":0.3,"quality":0.8,"redundancy":0.1,"robustness":1.0,"coverage":0.3,"observation":0.1},
            {"id":"stain_a","kind":"stain","cost":2.0,"information":0.9,"quality":1.0,"redundancy":0.1,"robustness":0.95,"coverage":0.5,"observation":0.7},
            {"id":"stain_b","kind":"stain","cost":2.0,"information":0.4,"quality":1.0,"redundancy":0.3,"robustness":0.7,"coverage":0.5,"observation":0.2},
            {"id":"landmark_a","kind":"landmark","cost":1.0,"information":0.8,"quality":1.0,"redundancy":0.0,"robustness":1.0,"coverage":0.9,"observation":0.9},
            {"id":"landmark_b","kind":"landmark","cost":1.0,"information":0.2,"quality":1.0,"redundancy":0.0,"robustness":1.0,"coverage":0.2,"observation":0.0}
        ],
        "budgets":{"sequential":4.0,"roi":2.0,"stain":2.0,"landmark":1.0},
        "allocation_options":[
            {"id":"more_patients","cost":10.0,"biological_replicates":8,"technical_replicates":1,"effect_standard_error":0.25},
            {"id":"more_slides","cost":10.0,"biological_replicates":2,"technical_replicates":8,"effect_standard_error":0.45}
        ],
        "power_designs":[{"id":"n20","clusters":20,"effect":0.5,"standard_deviation":1.0},{"id":"n80","clusters":80,"effect":0.5,"standard_deviation":1.0}],
        "power_replicates":1000,"alpha":0.05,"seed":227,"timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "active-design",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.synthetic_active_design");
    assert_eq!(result["roi_selection"]["selected_ids"][0], "roi_a");
    assert_eq!(result["stain_selection"]["selected_ids"][0], "stain_a");
    assert_eq!(
        result["landmark_selection"]["selected_ids"][0],
        "landmark_a"
    );
    assert_eq!(
        result["replicate_allocation"]["recommended_id"],
        "more_patients"
    );
    assert!(
        result["power"][1]["estimated_power"].as_f64().unwrap()
            > result["power"][0]["estimated_power"].as_f64().unwrap()
    );
    assert!(
        result["sequential"]["posterior_history"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
    assert_eq!(
        result["claim_status"],
        "synthetic_active_design_not_operationally_validated"
    );
}
