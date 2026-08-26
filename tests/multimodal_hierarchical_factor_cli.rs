#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn hierarchical_factor_graph_preserves_nested_replication_and_attachment_levels() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("hierarchy.json");
    let fixture = serde_json::json!({
        "model_id":"nested_pathology_factors",
        "latent_dimensions":2,
        "entities":[
            {"entity_id":"p1","level":"patient","parent_id":null},
            {"entity_id":"s1","level":"specimen","parent_id":"p1"},
            {"entity_id":"r1","level":"region","parent_id":"s1"},
            {"entity_id":"c1","level":"cell","parent_id":"r1"},
            {"entity_id":"c2","level":"cell","parent_id":"r1"}
        ],
        "modalities":[
            {"modality_id":"clinical","entity_level":"patient","entity_ids":["p1"],"measurement_status":"measured","likelihood":"gaussian"},
            {"modality_id":"morphology","entity_level":"cell","entity_ids":["c1","c2"],"measurement_status":"measured","likelihood":"gaussian"}
        ]
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "hierarchical-factor",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.hierarchical_factor_graph");
    assert_eq!(result["latent_nodes"].as_array().unwrap().len(), 5);
    assert_eq!(result["conditional_edges"].as_array().unwrap().len(), 4);
    assert_eq!(
        result["observation_attachments"].as_array().unwrap().len(),
        3
    );
    assert_eq!(result["replication_unit"], "patient");
    assert_eq!(result["direct_cell_patient_replication"], false);
    assert_eq!(result["claim_status"], "compiled_hierarchical_factor_graph");
}
