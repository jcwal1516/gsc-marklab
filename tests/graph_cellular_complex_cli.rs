#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn compartment_cellular_complex_preserves_boundary_under_segmentation_perturbation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("cellular.json");
    fs::write(
        &input,
        r#"{
  "pathology_interpretation":"single annotated tumor compartment",
  "baseline":{
    "junctions":[
      {"id":"a","coordinates_um":[0.0,0.0]},
      {"id":"b","coordinates_um":[1.0,0.0]},
      {"id":"c","coordinates_um":[1.0,1.0]},
      {"id":"d","coordinates_um":[0.0,1.0]}
    ],
    "interfaces":[
      {"id":"ab","source_id":"a","target_id":"b"},
      {"id":"bc","source_id":"b","target_id":"c"},
      {"id":"cd","source_id":"c","target_id":"d"},
      {"id":"da","source_id":"d","target_id":"a"}
    ],
    "domains":[{"id":"tumor","oriented_interfaces":[
      {"interface_id":"ab","orientation":1},
      {"interface_id":"bc","orientation":1},
      {"interface_id":"cd","orientation":1},
      {"interface_id":"da","orientation":1}
    ]}]
  },
  "segmentation_perturbations":[{
    "id":"shifted_boundary",
    "segmentation":{
      "junctions":[
        {"id":"a","coordinates_um":[0.0,0.0]},
        {"id":"b","coordinates_um":[1.1,0.0]},
        {"id":"c","coordinates_um":[1.1,1.0]},
        {"id":"d","coordinates_um":[0.0,1.0]}
      ],
      "interfaces":[
        {"id":"ab","source_id":"a","target_id":"b"},
        {"id":"bc","source_id":"b","target_id":"c"},
        {"id":"cd","source_id":"c","target_id":"d"},
        {"id":"da","source_id":"d","target_id":"a"}
      ],
      "domains":[{"id":"tumor","oriented_interfaces":[
        {"interface_id":"ab","orientation":1},
        {"interface_id":"bc","orientation":1},
        {"interface_id":"cd","orientation":1},
        {"interface_id":"da","orientation":1}
      ]}]
    }
  }],
  "maximum_incidence_entries":100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "cellular-complex",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.cellular_complex");
    assert_eq!(
        result["pathology_interpretation"],
        "single annotated tumor compartment"
    );
    assert_eq!(result["baseline"]["cells_0"].as_array().unwrap().len(), 4);
    assert_eq!(result["baseline"]["cells_1"].as_array().unwrap().len(), 4);
    assert_eq!(result["baseline"]["cells_2"], serde_json::json!(["tumor"]));
    assert!(
        result["baseline"]["boundary_of_boundary_max_abs"]
            .as_f64()
            .unwrap()
            < 1e-12
    );
    assert_eq!(result["perturbations"][0]["topology_unchanged"], true);
    assert_eq!(
        result["perturbations"][0]["boundary_matrices_unchanged"],
        true
    );
    assert!(
        (result["perturbations"][0]["maximum_junction_displacement_um"]
            .as_f64()
            .unwrap()
            - 0.1)
            .abs()
            < 1e-12
    );
    assert_eq!(
        result["robustness_status"],
        "stable_for_all_declared_segmentation_perturbations"
    );
    assert_eq!(
        result["claim_status"],
        "research_only_declared_compartment_interpretation"
    );
}
