#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use marklab_graph::{
    typed_triangle_motif_workflow, MotifEdgeInput, MotifNodeInput, TypedTriangleMotifSpec,
};

#[test]
fn typed_triangle_count_adjacency_and_complete_tie_null_match_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("motif.json");
    fs::write(
        &input,
        r#"{
  "nodes":[
    {"id":"a","label":"cell","stratum":"all"},
    {"id":"b","label":"cell","stratum":"all"},
    {"id":"c","label":"vessel","stratum":"all"}
  ],
  "edges":[
    {"source_id":"a","target_id":"b"},
    {"source_id":"a","target_id":"c"},
    {"source_id":"b","target_id":"c"}
  ],
  "motif_id":"cell-cell-vessel-triangle",
  "required_labels":["cell","cell","vessel"],
  "permutations":39,
  "seed":77,
  "maximum_triples":100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "motif-triangle",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.typed_triangle_motif");
    assert_eq!(result["observed_count"], 1);
    assert_eq!(result["instances"], serde_json::json!([["a", "b", "c"]]));
    assert_eq!(result["motif_adjacency"][0], serde_json::json!([0, 1, 1]));
    assert_eq!(result["motif_adjacency"][1], serde_json::json!([1, 0, 1]));
    assert_eq!(result["p_value_upper"], 1.0);
    assert_eq!(result["null_counts"].as_array().unwrap().len(), 39);
    assert!(result["null_counts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|value| value == 1));
}

#[test]
fn compact_triangle_summary_preserves_the_exact_statistic_without_dense_details() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("motif.json");
    fs::write(
        &input,
        r#"{
  "nodes":[
    {"id":"a","label":"cell","stratum":"all"},
    {"id":"b","label":"cell","stratum":"all"},
    {"id":"c","label":"vessel","stratum":"all"}
  ],
  "edges":[
    {"source_id":"a","target_id":"b"},
    {"source_id":"a","target_id":"c"},
    {"source_id":"b","target_id":"c"}
  ],
  "motif_id":"cell-cell-vessel-triangle",
  "required_labels":["cell","cell","vessel"],
  "permutations":39,
  "seed":77,
  "maximum_triples":100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("summary.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "motif-triangle-summary",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.typed_triangle_motif_summary");
    assert_eq!(result["node_count"], 3);
    assert_eq!(result["edge_count"], 3);
    assert_eq!(result["observed_count"], 1);
    assert_eq!(result["triples_evaluated"], 1);
    assert!(result.get("nodes").is_none());
    assert!(result.get("edges").is_none());
    assert!(result.get("instances").is_none());
    assert!(result.get("motif_adjacency").is_none());
}

#[test]
fn sparse_two_thousand_node_motif_uses_only_forward_wedge_work() {
    const NODE_COUNT: usize = 2_000;
    const TRIANGLE_COUNT: usize = 666;
    let nodes = (0..NODE_COUNT)
        .map(|index| MotifNodeInput {
            id: format!("node_{index:04}"),
            label: match index % 3 {
                0 => "a",
                1 => "b",
                _ => "c",
            }
            .to_owned(),
            stratum: if index < TRIANGLE_COUNT * 3 {
                format!("triangle_{:04}", index / 3)
            } else {
                format!("tail_{index:04}")
            },
        })
        .collect::<Vec<_>>();
    let mut edges = Vec::with_capacity(TRIANGLE_COUNT * 3);
    for triangle in 0..TRIANGLE_COUNT {
        let first = triangle * 3;
        for (left, right) in [
            (first, first + 1),
            (first, first + 2),
            (first + 1, first + 2),
        ] {
            edges.push(MotifEdgeInput {
                source_id: format!("node_{left:04}"),
                target_id: format!("node_{right:04}"),
            });
        }
    }

    let result = typed_triangle_motif_workflow(TypedTriangleMotifSpec {
        nodes,
        edges,
        motif_id: "a-b-c-radius-triangle".to_owned(),
        required_labels: ["a".to_owned(), "b".to_owned(), "c".to_owned()],
        permutations: 20,
        seed: 17,
        maximum_triples: TRIANGLE_COUNT as u64,
    })
    .expect("sparse motif workload");

    assert_eq!(result.triples_evaluated, TRIANGLE_COUNT as u64);
    assert_eq!(result.observed_count, TRIANGLE_COUNT as u64);
    assert_eq!(result.instances.len(), TRIANGLE_COUNT);
    assert!(result
        .null_counts
        .iter()
        .all(|count| *count == TRIANGLE_COUNT as u64));
}

#[test]
fn sparse_motif_matches_an_exhaustive_triangle_oracle() {
    const NODE_COUNT: usize = 24;
    let nodes = (0..NODE_COUNT)
        .map(|index| MotifNodeInput {
            id: format!("n{index:02}"),
            label: "cell".to_owned(),
            stratum: "all".to_owned(),
        })
        .collect::<Vec<_>>();
    let mut edge_matrix = vec![vec![false; NODE_COUNT]; NODE_COUNT];
    let mut edges = Vec::new();
    let unordered_pairs =
        (0..NODE_COUNT).flat_map(|left| ((left + 1)..NODE_COUNT).map(move |right| (left, right)));
    for (left, right) in unordered_pairs {
        if (left * 17 + right * 31) % 7 < 3 {
            edge_matrix[left][right] = true;
            edge_matrix[right][left] = true;
            edges.push(MotifEdgeInput {
                source_id: format!("n{left:02}"),
                target_id: format!("n{right:02}"),
            });
        }
    }
    let mut expected_triangles = 0_u64;
    let mut expected_forward_wedges = 0_u64;
    for first in 0..NODE_COUNT {
        let forward = ((first + 1)..NODE_COUNT)
            .filter(|second| edge_matrix[first][*second])
            .collect::<Vec<_>>();
        expected_forward_wedges += (forward.len() * forward.len().saturating_sub(1) / 2) as u64;
        for second in (first + 1)..NODE_COUNT {
            for third in (second + 1)..NODE_COUNT {
                expected_triangles += u64::from(
                    edge_matrix[first][second]
                        && edge_matrix[first][third]
                        && edge_matrix[second][third],
                );
            }
        }
    }

    let result = typed_triangle_motif_workflow(TypedTriangleMotifSpec {
        nodes,
        edges,
        motif_id: "all-cell-triangle".to_owned(),
        required_labels: ["cell".to_owned(), "cell".to_owned(), "cell".to_owned()],
        permutations: 20,
        seed: 91,
        maximum_triples: expected_forward_wedges,
    })
    .expect("sparse motif oracle case");

    assert_eq!(result.triples_evaluated, expected_forward_wedges);
    assert_eq!(result.observed_count, expected_triangles);
    assert_eq!(result.instances.len() as u64, expected_triangles);
}

#[test]
fn dense_motif_adjacency_is_rejected_before_allocation() {
    let nodes = (0..4_096)
        .map(|index| MotifNodeInput {
            id: format!("n{index:04}"),
            label: "cell".to_owned(),
            stratum: "all".to_owned(),
        })
        .collect::<Vec<_>>();
    let error = typed_triangle_motif_workflow(TypedTriangleMotifSpec {
        nodes,
        edges: Vec::new(),
        motif_id: "empty-graph-triangle".to_owned(),
        required_labels: ["cell".to_owned(), "cell".to_owned(), "cell".to_owned()],
        permutations: 20,
        seed: 3,
        maximum_triples: 1,
    })
    .expect_err("dense output storage must be bounded");

    assert!(error.to_string().contains("motif adjacency bytes"));
}
