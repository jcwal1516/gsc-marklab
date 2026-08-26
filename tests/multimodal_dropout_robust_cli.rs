#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn modality_dropout_training_recovers_each_missing_view_on_heldout_patients() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("dropout.json");
    let rows = (0..24)
        .map(|index| {
            let z = (index as f64 - 11.5) / 5.0;
            serde_json::json!({
                "entity_id":format!("p{index:02}"),
                "split":if index < 18 {"train"} else {"test"},
                "views":[
                    {"values":[z,0.5*z]},
                    {"values":[1.5*z,-z]}
                ]
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "design":{
            "entity_level":"patient",
            "modalities":[
                {"id":"morphology","measurement_status":"measured","likelihood":"gaussian","feature_names":["m1","m2"]},
                {"id":"ihc","measurement_status":"measured","likelihood":"gaussian","feature_names":["i1","i2"]}
            ],
            "missingness_assumption":"structurally_absent_or_mar",
            "coordinate_frame":null
        },
        "rows":rows,
        "latent_dimensions":1,
        "dropout_patterns":[
            {"pattern_id":"morphology_only","retained_modalities":["morphology"],"probability":0.5},
            {"pattern_id":"ihc_only","retained_modalities":["ihc"],"probability":0.5}
        ],
        "required_anchor_modalities":["morphology","ihc"],
        "consistency_weight":0.5,
        "parameter_precision":0.1,
        "maximum_iterations":500,
        "seed":43,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "dropout-robust",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.modality_robust_inference_model");
    assert_eq!(result["pattern_validation"].as_array().unwrap().len(), 2);
    assert!(result["pattern_validation"]
        .as_array()
        .unwrap()
        .iter()
        .all(|pattern| {
            pattern["heldout_missing_view_rmse"].as_f64().unwrap() < 0.3
                && pattern["heldout_patient_count"].as_u64().unwrap() == 6
        }));
    assert!(
        result["diagnostics"]["objective_final"].as_f64().unwrap()
            < result["diagnostics"]["objective_initial"].as_f64().unwrap()
    );
    assert_eq!(result["fit_state"], "approximate_only");
}
