#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn graph_validation_suite_records_exact_oracles_and_scoped_stress_limits() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("validation.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args(["graph", "validate", "--out", output.to_str().unwrap()])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_mathematics_validation");
    assert_eq!(result["exact_fixture_status"], "passed");
    assert_eq!(
        result["scientific_stress_status"],
        "partial_with_declared_unsupported_dimensions"
    );
    let entries = result["entries"].as_array().unwrap();
    for id in [
        "dense_path_laplacian_eigenpairs",
        "chebyshev_against_exact_heat",
        "spectral_wavelet_finite_frame",
        "diffusion_wavelet_reconstruction",
        "scattering_controlled_signal_stability",
        "hypergraph_hand_incidence",
        "motif_exhaustive_triangle",
        "simplicial_boundary_and_hodge",
        "cellular_boundary_and_segmentation_perturbation",
        "radius_choice_sensitivity",
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("missing validation entry {id}"));
        assert_eq!(entry["status"], "passed", "{id}");
    }
    for id in [
        "knn_kernel_barrier_component_sensitivity",
        "registration_perturbation",
        "fixed_density_sparse_memory",
        "cpu_gpu_parity",
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("missing limitation entry {id}"));
        assert!(matches!(
            entry["status"].as_str().unwrap(),
            "not_supported" | "not_applicable"
        ));
    }
    assert_eq!(
        result["claim_status"],
        "synthetic_exact_validation_only_no_pathology_performance_claim"
    );
}
