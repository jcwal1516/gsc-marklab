#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn every_exposure_mapping_branch_matches_hand_values() {
    let directory = tempfile::tempdir().unwrap();
    let cases = [
        ("binary_any_treated", None, Vec::new(), 1.0),
        ("count_treated", None, Vec::new(), 1.0),
        ("weighted_fraction_treated", None, Vec::new(), 2.0 / 3.0),
        (
            "gaussian_distance_decay",
            Some(1.0),
            Vec::new(),
            (-0.5_f64).exp(),
        ),
        ("continuous_field", None, Vec::new(), 0.2),
    ];
    for (kind, bandwidth, radii, expected) in cases {
        let result = run_case(&directory, kind, bandwidth, radii);
        assert_eq!(result["format"], "marklab.spatial_exposure_mapping");
        assert_close(
            result["exposures"][1]["scalar_value"].as_f64().unwrap(),
            expected,
        );
        assert_eq!(result["claim_status"], "exposure_construction_only");
    }
    let result = run_case(
        &directory,
        "multiscale_count_treated",
        None,
        vec![0.5, 1.0, 3.0],
    );
    assert_eq!(
        result["exposures"][1]["vector_value"],
        serde_json::json!([0.0, 1.0, 1.0])
    );
}

fn run_case(
    directory: &tempfile::TempDir,
    kind: &str,
    bandwidth_um: Option<f64>,
    radii_um: Vec<f64>,
) -> serde_json::Value {
    let input = directory.path().join(format!("{kind}.json"));
    let output = directory.path().join(format!("{kind}-out.json"));
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "graph_provenance": "prespecified_before_outcomes",
            "units": [
                {"unit_id": "a", "treatment": true, "continuous_field_value": 0.1},
                {"unit_id": "b", "treatment": false, "continuous_field_value": 0.2},
                {"unit_id": "c", "treatment": false, "continuous_field_value": 0.3}
            ],
            "edges": [
                {"left_unit": "a", "right_unit": "b", "weight": 2.0, "distance_um": 1.0},
                {"left_unit": "b", "right_unit": "c", "weight": 1.0, "distance_um": 3.0}
            ],
            "kind": kind,
            "bandwidth_um": bandwidth_um,
            "radii_um": radii_um,
            "maximum_directed_edge_visits": 100
        }))
        .unwrap(),
    )
    .unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "exposure-mapping",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "{actual} != {expected}"
    );
}
