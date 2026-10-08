#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn whole_patient_bottleneck_energy_comparison_matches_exact_enumeration() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("compare.json");
    fs::write(
        &input,
        r#"{
  "diagrams":[
    {"patient_id":"a1","group":"A","stratum":"all","finite_pairs":[{"birth":0.0,"death":1.0}]},
    {"patient_id":"a2","group":"A","stratum":"all","finite_pairs":[{"birth":0.0,"death":1.0}]},
    {"patient_id":"b1","group":"B","stratum":"all","finite_pairs":[{"birth":0.0,"death":3.0}]},
    {"patient_id":"b2","group":"B","stratum":"all","finite_pairs":[{"birth":0.0,"death":3.0}]}
  ],
  "metric":"bottleneck_linf",
  "coefficient":0.0,
  "maximum_exact_assignments":100,
  "timeout_seconds":30
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "compare-persistence",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.persistence_distribution_comparison"
    );
    assert_eq!(result["permutation_unit"], "whole_patient_diagram");
    assert!((result["distance_matrix"][0][2].as_f64().unwrap() - 1.5).abs() < 1e-12);
    assert!((result["observed_energy"].as_f64().unwrap() - 3.0).abs() < 1e-12);
    assert_eq!(result["assignments_completed"], 6);
    assert!((result["p_value_upper"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-12);
    assert_eq!(
        result["claim_status"],
        "experimental_whole_patient_topology_comparison"
    );
}
