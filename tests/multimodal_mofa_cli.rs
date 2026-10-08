#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pinned_mofa_recovers_shared_factor_and_structurally_masked_heldout_view() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("mofa.json");
    let rows = (0..20)
        .map(|index| {
            let z = (index as f64 - 9.5) / 3.0;
            let noise = if (index as usize).is_multiple_of(2) {
                0.04
            } else {
                -0.04
            };
            serde_json::json!({
                "entity_id": format!("p{index:02}"),
                "split": if index < 16 { "train" } else { "test" },
                "views": [
                    {"values":[z + noise,0.5*z-noise],"observed":[true,true]},
                    {"values":[1.5*z-noise,-z+noise],"observed":[index < 16,index < 16]},
                    {"values":[0.8*z+noise,-0.3*z-noise],"observed":[true,true]}
                ]
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "design": {
            "entity_level":"patient",
            "modalities":[
                {"id":"morphology","measurement_status":"measured","likelihood":"gaussian","feature_names":["x1","x2"]},
                {"id":"ihc","measurement_status":"measured","likelihood":"gaussian","feature_names":["y1","y2"]},
                {"id":"omics","measurement_status":"measured","likelihood":"gaussian","feature_names":["g1","g2"]}
            ],
            "missingness_assumption":"structurally_absent_or_mar",
            "coordinate_frame":null
        },
        "rows":rows,
        "maximum_factors":2,
        "iterations":300,
        "convergence_mode":"medium",
        "seed":17,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "mofa",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.multiview_factor_model");
    assert_eq!(result["backend"]["version"], "mofapy2-0.7.4");
    assert_eq!(result["design"]["validation_status"], "passed");
    assert_eq!(
        result["standardization"]["fit_split"],
        "train_observed_only"
    );
    assert_eq!(result["training_row_count"], 16);
    assert_eq!(result["missing_predictions"].as_array().unwrap().len(), 8);
    assert!(result["heldout_missing_rmse"].as_f64().unwrap() < 0.4);
    assert!(
        result["diagnostics"]["elbo_last"].as_f64().unwrap()
            > result["diagnostics"]["elbo_first"].as_f64().unwrap()
    );
    assert!(result["active_factor_count"].as_u64().unwrap() >= 1);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_multiview_factor_model"
    );
}
