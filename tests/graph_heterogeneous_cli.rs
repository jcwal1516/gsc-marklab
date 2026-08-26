#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn typed_spatial_relation_drives_one_explicit_message_layer() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("heterogeneous.json");
    fs::write(
        &input,
        r#"{
  "nodes":[
    {"id":"cell-1","node_type":"cell","coordinates_um":[0.0,0.0],"features":[1.0,2.0]},
    {"id":"vessel-1","node_type":"vessel","coordinates_um":[1.0,0.0],"features":[10.0,20.0]}
  ],
  "relations":[
    {"name":"cell_near_vessel","source_type":"cell","target_type":"vessel","radius_um":2.0,"message_scale":2.0,"aggregation":"sum"}
  ],
  "maximum_pair_evaluations":10
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "heterogeneous-message",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.heterogeneous_graph_message");
    assert_eq!(result["nodes"][0]["id"], "cell-1");
    assert_eq!(result["edges"].as_array().unwrap().len(), 1);
    assert_eq!(result["edges"][0]["relation"], "cell_near_vessel");
    assert_eq!(
        result["updated_features"]["cell-1"],
        serde_json::json!([1.0, 2.0])
    );
    assert_eq!(
        result["updated_features"]["vessel-1"],
        serde_json::json!([12.0, 24.0])
    );
    assert_eq!(result["pair_evaluations"], 1);
    assert_eq!(
        result["claim_status"],
        "descriptive_typed_message_passing_only"
    );
}
