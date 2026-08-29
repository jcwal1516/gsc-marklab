#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn request(maximum_projection_work: u64) -> serde_json::Value {
    let size = 5.0_f64;
    let signal = (0..5)
        .map(|index| {
            2.0 / size.sqrt()
                + 3.0
                    * (2.0 / size).sqrt()
                    * (((index as f64 + 0.5) * std::f64::consts::PI) / size).cos()
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "basis": {
            "nodes": signal.iter().enumerate().map(|(index, value)| serde_json::json!({
                "id": format!("cell-{index}"),
                "coordinates_um": [index as f64, 0.0],
                "signal": value
            })).collect::<Vec<_>>(),
            "radius_um": 1.1,
            "mode_count": 3,
            "maximum_iterations": 96,
            "residual_tolerance": 1e-9,
            "maximum_nodes": 5,
            "maximum_candidate_pairs": 10,
            "maximum_edges": 4,
            "maximum_components": 1,
            "maximum_matrix_vector_work": 10_000,
            "maximum_orthogonalization_work": 100_000,
            "maximum_ritz_rotations": 1_000,
            "maximum_working_bytes": 64_000,
            "maximum_retained_bytes": 64_000
        },
        "maximum_projection_work": maximum_projection_work,
        "maximum_fourier_retained_bytes": 8_192
    })
}

#[test]
fn sparse_radius_fourier_energy_matches_the_path_eigenmode_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("fourier.json");
    let output = directory.path().join("result.json");
    fs::write(&input, serde_json::to_vec(&request(15)).unwrap()).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-fourier-energy",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&output).unwrap()).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.graph_sparse_radius_fourier_energy"
    );
    assert_eq!(result["component_count"], 1);
    assert_eq!(result["returned_mode_count"], 3);
    assert_eq!(result["projection_work"], 15);
    assert_eq!(result["total_signal_status"], "positive");
    assert_eq!(result["centered_signal_status"], "positive");
    assert!((result["total_signal_energy"].as_f64().unwrap() - 13.0).abs() <= 1e-9);
    assert!((result["component_zero_energy"].as_f64().unwrap() - 4.0).abs() <= 1e-9);
    assert!((result["nonzero_low_frequency_energy"].as_f64().unwrap() - 9.0).abs() <= 1e-8);
    assert!((result["captured_energy"].as_f64().unwrap() - 13.0).abs() <= 1e-8);
    assert!(result["unresolved_centered_energy"].as_f64().unwrap() <= 1e-8);
    assert!(
        (result["captured_centered_energy_fraction"]
            .as_f64()
            .unwrap()
            - 1.0)
            .abs()
            <= 1e-8
    );

    fs::write(&input, serde_json::to_vec(&request(14)).unwrap()).expect("one-short fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-fourier-energy",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "projection work exceeds caller maximum",
        ));
}
