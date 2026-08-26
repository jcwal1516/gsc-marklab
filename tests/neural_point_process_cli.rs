#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn jax_neural_cox_and_marked_likelihood_learn_heldout_spatial_and_mark_structure() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.json");
    let patterns = (0..12)
        .map(|patient| {
            let context = if (patient as usize).is_multiple_of(2) {
                -1.0
            } else {
                1.0
            };
            let points = (0..20)
                .map(|index| {
                    let base_x = if context < 0.0 { 0.2 } else { 0.8 };
                    let x = base_x + 0.08 * (((index * 7 + patient) % 11) as f64 / 10.0 - 0.5);
                    let y = ((index * 13 + patient * 3) % 19) as f64 / 19.0;
                    serde_json::json!({"coordinates":[x,y],"mark":if y > 0.5 {"high"} else {"low"}})
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "pattern_id":format!("p{patient:02}"),
                "patient_id":format!("p{patient:02}"),
                "split":if patient < 8 {"train"} else {"test"},
                "window":[0.0,1.0,0.0,1.0],
                "context":[context],
                "points":points
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "patterns":patterns,
        "mark_labels":["high","low"],
        "architecture":{"hidden_units":6,"activation":"tanh","intensity_link":"softplus","mark_link":"softmax"},
        "quadrature_grid":20,
        "quadrature_reference_grid":40,
        "parameter_precision":0.01,
        "maximum_iterations":1000,
        "seed":73,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "neural",
            "point-process",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.neural_marked_cox_process");
    assert_eq!(result["backend"]["jax_version"], "0.11.1");
    assert!(
        result["heldout"]["joint_log_likelihood"].as_f64().unwrap()
            > result["heldout"]["homogeneous_independent_mark_log_likelihood"]
                .as_f64()
                .unwrap()
    );
    assert!(result["heldout"]["mark_accuracy"].as_f64().unwrap() > 0.9);
    assert!(
        result["quadrature"]["relative_integral_difference"]
            .as_f64()
            .unwrap()
            < 0.05
    );
    assert!(
        result["probe_intensities"]["context_positive_high_x"]
            .as_f64()
            .unwrap()
            > result["probe_intensities"]["context_positive_low_x"]
                .as_f64()
                .unwrap()
                * 2.0
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_neural_marked_cox_process"
    );
}
