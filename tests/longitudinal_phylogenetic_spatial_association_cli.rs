#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn collinear_tree_and_space_have_unit_association_and_replay() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "tree_provenance": "synthetic_path_v1",
            "tree_edges": [
                {"left_node": "n1", "right_node": "n2", "branch_length": 1.0},
                {"left_node": "n2", "right_node": "n3", "branch_length": 1.0},
                {"left_node": "n3", "right_node": "n4", "branch_length": 1.0}
            ],
            "clones": [
                {"clone_id": "c1", "tree_node": "n1", "centroid_um": [0.0, 0.0, 0.0], "patient_id": "p1", "specimen_id": "s1"},
                {"clone_id": "c2", "tree_node": "n2", "centroid_um": [1.0, 0.0, 0.0], "patient_id": "p1", "specimen_id": "s1"},
                {"clone_id": "c3", "tree_node": "n3", "centroid_um": [2.0, 0.0, 0.0], "patient_id": "p1", "specimen_id": "s1"},
                {"clone_id": "c4", "tree_node": "n4", "centroid_um": [3.0, 0.0, 0.0], "patient_id": "p1", "specimen_id": "s1"}
            ],
            "permutations": 199,
            "seed": 20260825,
            "maximum_pair_permutation_evaluations": 10000
        }))
        .unwrap(),
    )
    .unwrap();
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    run(&input, &first);
    run(&input, &second);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.phylogenetic_spatial_association");
    assert!((result["observed_statistic"].as_f64().unwrap() - 1.0).abs() < 1.0e-12);
    assert_eq!(result["pairs"].as_array().unwrap().len(), 6);
    assert_eq!(result["null_statistics"].as_array().unwrap().len(), 199);
    assert!(result["p_value"].as_f64().unwrap() <= 0.2);
    assert_eq!(
        result["interpretation"],
        "cross_sectional_noncausal_association_only"
    );
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "longitudinal",
            "phylogenetic-spatial-association",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}
