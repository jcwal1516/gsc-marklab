#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use marklab_graph::{
    graph_chebyshev_heat_workflow, FrequencyBandSpec, GraphChebyshevHeatSpec, GraphNodeInput,
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

fn exact_heat(time: f64) -> Vec<f64> {
    graph_chebyshev_heat_workflow(GraphChebyshevHeatSpec {
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
        time,
        tolerance: 1e-9,
        maximum_order: 64,
    })
    .expect("exact small heat")
    .exact_signal
}

#[test]
fn sparse_diffusion_wavelet_matches_dense_heat_and_telescopes_under_total_bounds() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("wavelet.json");
    let output = directory.path().join("result.json");
    let times = [0.1, 0.2, 0.4];
    let mut spec = serde_json::json!({
        "nodes": nodes(),
        "radius_um": 1.1,
        "times": times,
        "tolerance": 1e-9,
        "maximum_order": 64,
        "maximum_nodes": 5,
        "maximum_candidate_pairs": 10,
        "maximum_edges": 4,
        "maximum_matrix_vector_work": 10_000,
        "maximum_working_bytes": 16_384,
        "maximum_retained_bytes": 1_000_000,
        "maximum_total_candidate_pairs": 30,
        "maximum_total_matrix_vector_work": 30_000
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-diffusion-wavelet",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&output).expect("result")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.graph_sparse_radius_diffusion_wavelet"
    );
    assert_eq!(result["scales"].as_array().unwrap().len(), times.len());
    let mut filtered = nodes()
        .into_iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    for (scale, time) in result["scales"].as_array().unwrap().iter().zip(times) {
        let expected = exact_heat(time);
        assert!(scale.get("filtered_signal").is_none());
        for (value, detail) in filtered
            .iter_mut()
            .zip(scale["detail_signal"].as_array().unwrap())
        {
            *value -= detail.as_f64().unwrap();
        }
        for (actual, expected) in filtered.iter().zip(expected) {
            assert!((*actual - expected).abs() < 1e-8);
        }
    }
    for (actual, coarse) in filtered
        .iter()
        .zip(result["coarse_signal"].as_array().unwrap())
    {
        assert!((*actual - coarse.as_f64().unwrap()).abs() < 1e-12);
    }
    assert!(result.get("reconstructed_signal").is_none());
    assert!(result["reconstruction_max_abs_error"].as_f64().unwrap() < 1e-12);

    spec["maximum_total_candidate_pairs"] = serde_json::json!(29);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("one-short fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-diffusion-wavelet",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "aggregate candidate work exceeds caller maximum",
        ));
}
