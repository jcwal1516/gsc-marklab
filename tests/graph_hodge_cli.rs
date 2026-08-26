#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn filled_triangle_hodge_operator_decomposition_and_filter_match_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("hodge.json");
    fs::write(
        &input,
        r#"{
  "node_ids":["a","b","c"],
  "edges":[
    {"source_id":"a","target_id":"b","flow":1.0},
    {"source_id":"a","target_id":"c","flow":2.0},
    {"source_id":"b","target_id":"c","flow":3.0}
  ],
  "maximum_dimension":2,
  "filter_step":0.1,
  "orthogonality_tolerance":0.0000000001,
  "maximum_clique_triples":100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "hodge",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.simplicial_hodge");
    assert_eq!(
        result["simplices"]["triangles"],
        serde_json::json!([["a", "b", "c"]])
    );
    for row in 0..3 {
        for column in 0..3 {
            let expected = if row == column { 3.0 } else { 0.0 };
            assert!(
                (result["hodge_laplacian_1"][row][column].as_f64().unwrap() - expected).abs()
                    < 1e-10
            );
        }
    }
    assert!(result["boundary_of_boundary_max_abs"].as_f64().unwrap() < 1e-12);
    assert!(result["decomposition"]["harmonic_norm"].as_f64().unwrap() < 1e-10);
    assert!(
        result["decomposition"]["reconstruction_max_abs_error"]
            .as_f64()
            .unwrap()
            < 1e-10
    );
    for (actual, expected) in result["filtered_flow"]
        .as_array()
        .unwrap()
        .iter()
        .zip([0.7, 1.4, 2.1])
    {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-10);
    }
}
