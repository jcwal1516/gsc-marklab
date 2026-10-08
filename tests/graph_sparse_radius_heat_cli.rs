#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use marklab_graph::{
    graph_chebyshev_heat_workflow, graph_sparse_radius_heat_workflow, graph_spectral_workflow,
    FrequencyBandSpec, GraphChebyshevHeatSpec, GraphNodeInput, GraphSparseRadiusHeatSpec,
    GraphSpectralSpec, GraphWeightSpec, LaplacianSpec,
};

fn nodes() -> Vec<GraphNodeInput> {
    [1.0, -0.5, 2.0, 0.25, -1.0]
        .into_iter()
        .enumerate()
        .map(|(index, signal)| GraphNodeInput {
            id: format!("cell-{index}"),
            coordinates_um: [index as f64, 0.0],
            signal,
        })
        .collect()
}

#[test]
fn sparse_grid_matches_brute_force_across_negative_cells_and_exact_resource_edges() {
    let nodes = (0..25)
        .map(|index| GraphNodeInput {
            id: format!("grid-{index:02}"),
            coordinates_um: [index as f64 % 5.0 - 2.25, (index / 5) as f64 - 2.25],
            signal: ((index * 17) % 11) as f64 - 5.0,
        })
        .collect::<Vec<_>>();
    let graph_spec = GraphSpectralSpec {
        nodes: nodes.clone(),
        radius_um: 1.45,
        weight: GraphWeightSpec::Binary,
        laplacian: LaplacianSpec::Combinatorial,
        bands: vec![FrequencyBandSpec {
            id: "all".into(),
            minimum: 0.0,
            maximum: 100.0,
        }],
        maximum_pairs: 300,
    };
    let exact_graph = graph_spectral_workflow(graph_spec.clone()).expect("exact graph");
    let exact = graph_chebyshev_heat_workflow(GraphChebyshevHeatSpec {
        graph: graph_spec,
        time: 0.2,
        tolerance: 1e-7,
        maximum_order: 64,
    })
    .expect("exact grid");
    let make_spec =
        |maximum_candidate_pairs, maximum_edges, maximum_working_bytes| GraphSparseRadiusHeatSpec {
            nodes: nodes.clone(),
            radius_um: 1.45,
            time: 0.2,
            tolerance: 1e-7,
            maximum_order: 64,
            maximum_nodes: nodes.len(),
            maximum_candidate_pairs,
            maximum_edges,
            maximum_matrix_vector_work: 100_000,
            maximum_working_bytes,
        };
    let sparse =
        graph_sparse_radius_heat_workflow(make_spec(300, 300, 1_000_000)).expect("sparse grid");
    assert_eq!(sparse.edge_count as usize, exact_graph.edges.len());
    for (actual, expected) in sparse.filtered_signal.iter().zip(exact.exact_signal) {
        assert!((actual - expected).abs() < 1e-7);
    }
    assert!(graph_sparse_radius_heat_workflow(make_spec(
        sparse.candidate_pair_evaluations - 1,
        300,
        1_000_000,
    ))
    .is_err());
    assert!(
        graph_sparse_radius_heat_workflow(make_spec(300, 300, sparse.working_bytes - 1,)).is_err()
    );
}

#[test]
fn sparse_radius_heat_matches_the_exact_small_graph_and_enforces_work_limits() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("sparse.json");
    let output = directory.path().join("sparse-result.json");
    let spec = serde_json::json!({
        "nodes": nodes(),
        "radius_um": 1.1,
        "time": 0.35,
        "tolerance": 1e-8,
        "maximum_order": 64,
        "maximum_nodes": 5,
        "maximum_candidate_pairs": 10,
        "maximum_edges": 4,
        "maximum_matrix_vector_work": 10_000,
        "maximum_working_bytes": 16_384
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-heat",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.graph_sparse_radius_heat");
    assert_eq!(result["node_count"], 5);
    assert_eq!(result["edge_count"], 4);
    assert!(result["candidate_pair_evaluations"].as_u64().unwrap() <= 10);
    assert!(result["working_bytes"].as_u64().unwrap() <= 16_384);

    let exact = graph_chebyshev_heat_workflow(GraphChebyshevHeatSpec {
        graph: GraphSpectralSpec {
            nodes: nodes(),
            radius_um: 1.1,
            weight: GraphWeightSpec::Binary,
            laplacian: LaplacianSpec::Combinatorial,
            bands: vec![FrequencyBandSpec {
                id: "all".into(),
                minimum: 0.0,
                maximum: 100.0,
            }],
            maximum_pairs: 10,
        },
        time: 0.35,
        tolerance: 1e-8,
        maximum_order: 64,
    })
    .expect("exact small graph");
    let actual = result["filtered_signal"].as_array().expect("signal");
    for (actual, expected) in actual.iter().zip(exact.exact_signal) {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-8);
    }

    let mut one_short = spec;
    one_short["maximum_edges"] = serde_json::json!(3);
    fs::write(&input, serde_json::to_vec(&one_short).unwrap()).expect("one-short fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-heat",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "edge count exceeds caller maximum",
        ));
}
