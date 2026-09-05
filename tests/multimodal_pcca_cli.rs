#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn paired_gaussian_pcca_recovers_the_shared_synthetic_direction_without_split_leakage() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("pcca.json");
    let rows = (0..12)
        .map(|index| {
            let z = index as f64 - 5.5;
            let noise = if (index as usize).is_multiple_of(2) {
                0.08
            } else {
                -0.08
            };
            serde_json::json!({
                "entity_id": format!("p{index:02}"),
                "split": if index < 8 { "train" } else { "test" },
                "x": [z + noise, 0.5 * z - noise],
                "y": [2.0 * z - noise, -z + noise]
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "design": {
            "entity_level": "patient",
            "modality_x": {"id":"morphology","measurement_status":"measured","likelihood":"gaussian","feature_names":["x1","x2"]},
            "modality_y": {"id":"ihc","measurement_status":"measured","likelihood":"gaussian","feature_names":["y1","y2"]},
            "missingness_assumption":"complete_paired_rows",
            "coordinate_frame": null
        },
        "rows": rows,
        "latent_dimensions": 1,
        "regularization": 0.000001,
        "noise_floor": 0.000001,
        "maximum_iterations": 200,
        "convergence_tolerance": 0.000000001,
        "timeout_seconds": 30
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "pcca",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.probabilistic_cca");
    assert_eq!(result["design"]["validation_status"], "passed");
    assert_eq!(result["standardization"]["fit_split"], "train_only");
    assert_eq!(result["standardization"]["fit_row_count"], 8);
    assert_eq!(result["heldout_row_count"], 4);
    assert!(result["canonical_correlations"][0].as_f64().unwrap() > 0.99);
    let trace = result["diagnostics"]["log_likelihood_trace"]
        .as_array()
        .unwrap();
    assert!(trace.last().unwrap().as_f64().unwrap() >= trace[0].as_f64().unwrap() - 1e-8);
    assert!(result["heldout_cross_view_rmse_y_from_x"].as_f64().unwrap() < 0.25);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_paired_gaussian_pcca"
    );
}
