#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn radius_and_union_knn_graphs_match_hand_oracles_and_digest_replays() {
    let directory = tempfile::tempdir().unwrap();
    for (rule, radius, neighbors, expected_edges) in [
        ("radius", Some(1.5), None, vec![["a", "b"]]),
        ("knn_union", None, Some(1), vec![["a", "b"], ["b", "c"]]),
    ] {
        let input = directory.path().join(format!("{rule}.json"));
        fs::write(
            &input,
            serde_json::to_vec(&serde_json::json!({
                "points": [
                    {"id": "a", "coordinates": [0.0, 5.0, 5.0], "position_uncertainty_um": 0.0},
                    {"id": "b", "coordinates": [1.0, 5.0, 5.0], "position_uncertainty_um": 0.0},
                    {"id": "c", "coordinates": [3.0, 5.0, 5.0], "position_uncertainty_um": 0.0}
                ],
                "window": {"minimum": [0.0, 0.0, 0.0], "maximum": [10.0, 10.0, 10.0]},
                "coordinate_unit": "micrometer",
                "voxel_spacing": [1.0, 1.0, 1.0],
                "anisotropy_matrix": null,
                "rule": rule,
                "radius_um": radius,
                "neighbors": neighbors,
                "distance_basis": "nominal",
                "weight": "binary",
                "gaussian_bandwidth_um": null,
                "maximum_unordered_pairs": 10,
                "maximum_edges": 10
            }))
            .unwrap(),
        )
        .unwrap();
        let first = directory.path().join(format!("{rule}-first.json"));
        let second = directory.path().join(format!("{rule}-second.json"));
        run(&input, &first);
        run(&input, &second);
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
        assert_eq!(result["format"], "marklab.spatial_graph3d");
        let edges = result["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|edge| {
                [
                    edge["left_id"].as_str().unwrap(),
                    edge["right_id"].as_str().unwrap(),
                ]
            })
            .collect::<Vec<_>>();
        assert_eq!(edges, expected_edges);
        assert_eq!(result["graph_sha256"].as_str().unwrap().len(), 64);
    }
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "spatial3d",
            "spatial-graph",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}
