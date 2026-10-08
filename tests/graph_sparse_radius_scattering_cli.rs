#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use marklab_graph::{
    graph_chebyshev_heat_workflow, FrequencyBandSpec, GraphChebyshevHeatSpec, GraphNodeInput,
    GraphSpectralSpec, GraphWeightSpec, LaplacianSpec,
};

fn signals() -> Vec<f64> {
    vec![1.0, -0.5, 2.0, 0.25, -1.0]
}

fn nodes_with_signals(signals: &[f64]) -> Vec<GraphNodeInput> {
    signals
        .iter()
        .enumerate()
        .map(|(index, signal)| GraphNodeInput {
            id: format!("cell-{index}"),
            coordinates_um: [index as f64, 0.0],
            signal: *signal,
        })
        .collect()
}

fn exact_heat(signal: &[f64], time: f64) -> Vec<f64> {
    graph_chebyshev_heat_workflow(GraphChebyshevHeatSpec {
        graph: GraphSpectralSpec {
            nodes: nodes_with_signals(signal),
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

fn mean_absolute(values: &[f64]) -> f64 {
    values.iter().map(|value| value.abs()).sum::<f64>() / values.len() as f64
}

#[test]
fn sparse_order_two_scattering_matches_dense_spectral_heat_and_bounds_all_propagations() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("scattering.json");
    let output = directory.path().join("result.json");
    let times = [0.1, 0.2, 0.4];
    let mut spec = serde_json::json!({
        "nodes": nodes_with_signals(&signals()),
        "radius_um": 1.1,
        "times": times,
        "scattering_order": 2,
        "tolerance": 1e-9,
        "maximum_order": 64,
        "maximum_nodes": 5,
        "maximum_candidate_pairs": 10,
        "maximum_edges": 4,
        "maximum_matrix_vector_work": 10_000,
        "maximum_working_bytes": 16_384,
        "maximum_retained_bytes": 1_000_000,
        "maximum_total_candidate_pairs": 80,
        "maximum_total_matrix_vector_work": 80_000
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-scattering",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.graph_sparse_radius_scattering");
    assert_eq!(result["heat_applications"], 8);
    assert_eq!(result["first_order"].as_array().unwrap().len(), 3);
    assert_eq!(result["second_order"].as_array().unwrap().len(), 3);

    let original = signals();
    let heats = times
        .iter()
        .map(|time| exact_heat(&original, *time))
        .collect::<Vec<_>>();
    let mut previous = original.clone();
    let mut first_moduli = Vec::new();
    for (level, heat) in heats.iter().enumerate() {
        let modulus = previous
            .iter()
            .zip(heat)
            .map(|(left, right)| (left - right).abs())
            .collect::<Vec<_>>();
        let actual = result["first_order"][level]["mean_absolute"]
            .as_f64()
            .unwrap();
        assert!((actual - mean_absolute(&modulus)).abs() < 1e-8);
        first_moduli.push(modulus);
        previous.clone_from(heat);
    }
    let mut pair = 0;
    for first_level in 0..times.len() - 1 {
        let propagated = times[first_level..]
            .iter()
            .map(|time| exact_heat(&first_moduli[first_level], *time))
            .collect::<Vec<_>>();
        for second_level in first_level + 1..times.len() {
            let modulus = propagated[second_level - first_level - 1]
                .iter()
                .zip(&propagated[second_level - first_level])
                .map(|(left, right)| (left - right).abs())
                .collect::<Vec<_>>();
            let row = &result["second_order"][pair];
            assert_eq!(row["first_level"], first_level);
            assert_eq!(row["second_level"], second_level);
            assert!(
                (row["mean_absolute"].as_f64().unwrap() - mean_absolute(&modulus)).abs() < 1e-8
            );
            pair += 1;
        }
    }

    spec["maximum_total_candidate_pairs"] = serde_json::json!(79);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("one-short fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-scattering",
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
