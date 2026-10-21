#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use marklab_graph::{
    graph_group_centrality_workflow, GraphGroupCentralityNode, GraphGroupCentralitySpec,
};

fn triangle_with_tail() -> Vec<GraphGroupCentralityNode> {
    vec![
        GraphGroupCentralityNode {
            id: "tail".into(),
            coordinates_um: [2.0, 0.0],
            group_id: "a".into(),
        },
        GraphGroupCentralityNode {
            id: "triangle-c".into(),
            coordinates_um: [0.5, 3.0_f64.sqrt() * 0.5],
            group_id: "c".into(),
        },
        GraphGroupCentralityNode {
            id: "triangle-a".into(),
            coordinates_um: [0.0, 0.0],
            group_id: "a".into(),
        },
        GraphGroupCentralityNode {
            id: "triangle-b".into(),
            coordinates_um: [1.0, 0.0],
            group_id: "b".into(),
        },
    ]
}

fn spec(nodes: Vec<GraphGroupCentralityNode>) -> GraphGroupCentralitySpec {
    GraphGroupCentralitySpec {
        nodes,
        radius_um: 1.01,
        maximum_nodes: 8,
        maximum_candidate_pairs: 28,
        maximum_edges: 16,
        maximum_group_distance_work: 1_000,
        maximum_clustering_wedges: 100,
        maximum_working_bytes: 128 * 1024,
    }
}

#[test]
fn group_degree_closeness_and_average_clustering_match_hand_oracles() {
    let result = graph_group_centrality_workflow(spec(triangle_with_tail()))
        .expect("connected group centrality");

    assert_eq!(result.format, "marklab.graph_group_centrality");
    assert_eq!(result.version, 1);
    assert_eq!(result.node_count, 4);
    assert_eq!(result.edge_count, 4);
    assert_eq!(result.component_count, 1);
    assert_eq!(result.group_count, 3);
    assert_eq!(result.clustering_wedges, 5);
    assert!(result.candidate_pair_evaluations <= 6);
    assert!(result.group_distance_work <= 1_000);
    assert!(result.working_bytes <= 128 * 1024);

    let rows = result
        .groups
        .iter()
        .map(|row| (row.group_id.as_str(), row))
        .collect::<std::collections::BTreeMap<_, _>>();
    let a = rows["a"];
    assert_eq!(a.node_count, 2);
    assert_eq!(a.external_neighbor_count, 2);
    assert_eq!(a.degree_centrality, 1.0);
    assert_eq!(a.closeness_centrality, 1.0);
    assert_eq!(a.average_clustering, 0.5);

    let b = rows["b"];
    assert_eq!(b.node_count, 1);
    assert_eq!(b.external_neighbor_count, 3);
    assert_eq!(b.degree_centrality, 1.0);
    assert_eq!(b.closeness_centrality, 1.0);
    assert!((b.average_clustering - 1.0 / 3.0).abs() < 1.0e-15);

    let c = rows["c"];
    assert_eq!(c.node_count, 1);
    assert_eq!(c.external_neighbor_count, 2);
    assert!((c.degree_centrality - 2.0 / 3.0).abs() < 1.0e-15);
    assert_eq!(c.closeness_centrality, 0.75);
    assert_eq!(c.average_clustering, 1.0);
}

#[test]
fn disconnected_closeness_matches_networkx_and_invalid_groups_fail_closed() {
    let nodes = vec![
        GraphGroupCentralityNode {
            id: "left-anchor".into(),
            coordinates_um: [0.0, 0.0],
            group_id: "anchor".into(),
        },
        GraphGroupCentralityNode {
            id: "left-other".into(),
            coordinates_um: [1.0, 0.0],
            group_id: "other".into(),
        },
        GraphGroupCentralityNode {
            id: "right-other-1".into(),
            coordinates_um: [10.0, 0.0],
            group_id: "other".into(),
        },
        GraphGroupCentralityNode {
            id: "right-other-2".into(),
            coordinates_um: [11.0, 0.0],
            group_id: "other".into(),
        },
    ];
    let result = graph_group_centrality_workflow(spec(nodes)).expect("disconnected result");
    assert_eq!(result.component_count, 2);
    let anchor = result
        .groups
        .iter()
        .find(|row| row.group_id == "anchor")
        .expect("anchor row");
    assert_eq!(anchor.external_neighbor_count, 1);
    assert_eq!(anchor.degree_centrality, 1.0 / 3.0);
    // NetworkX retains all three non-group nodes in the numerator, while
    // unreachable nodes contribute zero to the distance sum: 3 / 1 = 3.
    assert_eq!(anchor.closeness_centrality, 3.0);
    assert_eq!(anchor.average_clustering, 0.0);

    let one_group = vec![
        GraphGroupCentralityNode {
            id: "a".into(),
            coordinates_um: [0.0, 0.0],
            group_id: "only".into(),
        },
        GraphGroupCentralityNode {
            id: "b".into(),
            coordinates_um: [1.0, 0.0],
            group_id: "only".into(),
        },
    ];
    assert!(graph_group_centrality_workflow(spec(one_group))
        .expect_err("all-node group must fail")
        .to_string()
        .contains("at least two non-empty groups"));

    let mut empty_id = triangle_with_tail();
    empty_id[0].group_id.clear();
    assert!(graph_group_centrality_workflow(spec(empty_id))
        .expect_err("empty group ID must fail")
        .to_string()
        .contains("non-empty exact group IDs"));

    let mut control_id = triangle_with_tail();
    control_id[0].group_id = "bad\nlabel".into();
    assert!(graph_group_centrality_workflow(spec(control_id))
        .expect_err("control character in group ID must fail")
        .to_string()
        .contains("without control characters"));

    let mut low_memory = spec(triangle_with_tail());
    low_memory.maximum_working_bytes = 1;
    assert!(graph_group_centrality_workflow(low_memory)
        .expect_err("preflight working-memory limit must fail")
        .to_string()
        .contains("working bytes exceed caller maximum"));
}

#[test]
fn group_centrality_cli_publishes_the_compact_typed_result() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("centrality.json");
    let output = directory.path().join("result.json");
    let nodes = triangle_with_tail()
        .into_iter()
        .map(|node| {
            serde_json::json!({
                "id": node.id,
                "coordinates_um": node.coordinates_um,
                "group_id": node.group_id,
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "nodes": nodes,
            "radius_um": 1.01,
            "maximum_nodes": 8,
            "maximum_candidate_pairs": 28,
            "maximum_edges": 16,
            "maximum_group_distance_work": 1_000,
            "maximum_clustering_wedges": 100,
            "maximum_working_bytes": 131_072
        }))
        .expect("spec JSON"),
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "group-centrality",
            "--input",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.graph_group_centrality");
    assert_eq!(result["version"], 1);
    assert_eq!(result["node_count"], 4);
    assert_eq!(result["edge_count"], 4);
    assert_eq!(result["component_count"], 1);
    assert_eq!(result["group_count"], 3);
    assert_eq!(result["groups"].as_array().expect("groups").len(), 3);
    assert!(result.get("nodes").is_none());
    assert!(result.get("edges").is_none());
}
