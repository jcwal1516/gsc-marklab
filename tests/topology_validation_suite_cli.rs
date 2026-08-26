#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn topology_validation_suite_records_exact_controls_and_scale_limitations() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("validation.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args(["topology", "validate", "--out", output.to_str().unwrap()])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.topology_validation");
    assert_eq!(result["exact_fixture_status"], "passed");
    let entries = result["entries"].as_array().unwrap();
    for id in [
        "hand_complex_betti",
        "alpha_against_analytic_triangle",
        "landscape_against_hand_tent",
        "persistence_image_against_erf_integral",
        "euler_betti_and_alternating_counts",
        "polygon_high_resolution_raster_minkowski",
        "lattice_connectivity_transition",
        "bounded_segmentation_perturbation",
        "essential_class_policy",
    ] {
        let entry = entries.iter().find(|entry| entry["id"] == id).unwrap();
        assert_eq!(entry["status"], "passed", "{id}");
    }
    let sparse = entries
        .iter()
        .find(|entry| entry["id"] == "sparse_memory_scaling")
        .unwrap();
    assert_eq!(sparse["status"], "not_verified");
    assert_eq!(
        result["claim_status"],
        "synthetic_exact_topology_validation_only"
    );
}
