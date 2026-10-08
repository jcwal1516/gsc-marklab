#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::{fmt::Write as _, fs};

#[test]
fn graph_smoothness_uses_complete_rows_and_inclusive_lower_tail() {
    let directory = tempfile::tempdir().unwrap();
    let nodes = directory.path().join("nodes.csv");
    let edges = directory.path().join("edges.csv");
    let output = directory.path().join("output.json");
    let repeated = directory.path().join("repeated.json");
    let mut node_csv = String::from("node_id,permutation_stratum,signal_0,signal_1\n");
    for index in 0..6 {
        writeln!(node_csv, "n{index},all,{index},-{}", index).unwrap();
    }
    fs::write(&nodes, node_csv).unwrap();
    fs::write(
        &edges,
        "left_node_id,right_node_id,weight\nn0,n1,1\nn1,n2,1\nn2,n3,1\nn3,n4,1\nn4,n5,1\n",
    )
    .unwrap();

    for out in [&output, &repeated] {
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "bayes",
                "graph-smoothness-permutation-test",
                "--nodes",
                nodes.to_str().unwrap(),
                "--edges",
                edges.to_str().unwrap(),
                "--laplacian",
                "combinatorial",
                "--permutations",
                "39",
                "--seed",
                "1201",
                "--maximum-component-edge-visits",
                "400",
                "--out",
                out.to_str().unwrap(),
            ])
            .assert()
            .success();
    }
    assert_eq!(fs::read(&output).unwrap(), fs::read(&repeated).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.graph_smoothness_permutation_test"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["alternative"], "low_energy_spatial_smoothness");
    assert_eq!(
        result["permutation_policy"],
        "complete_signal_rows_within_declared_strata"
    );
    assert!((result["observed_energy"].as_f64().unwrap() - 2.0 / 7.0).abs() < 1e-12);
    assert_eq!(result["permutations"], 39);
    assert_eq!(result["component_edge_visits"], 400);
    let inclusive = result["null_at_or_below_observed"].as_u64().unwrap();
    let p = result["p_low"].as_f64().unwrap();
    assert!((p - (inclusive + 1) as f64 / 40.0).abs() < 1e-12);
    assert!((0.025..=1.0).contains(&p));
    assert!(result["null_min"].as_f64().unwrap() <= result["null_max"].as_f64().unwrap());
}
