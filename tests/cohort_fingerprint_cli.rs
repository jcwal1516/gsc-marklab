#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn fingerprint_distance_matches_the_hand_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("components.csv");
    fs::write(
        &input,
        "sample_id,component,axis,value,uncertainty,weight\n\
left,c1,0,0,0.1,2\n\
left,c1,1,0,0.1,2\n\
right,c1,0,3,0.2,2\n\
right,c1,1,4,0.2,2\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "fingerprint-distance",
            "--input",
            input.to_str().expect("input path"),
            "--left-sample",
            "left",
            "--right-sample",
            "right",
            "--spec-version",
            "v1",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.spatial_fingerprint_distance");
    assert_eq!(result["version"], 1);
    assert_eq!(result["specification"]["version"], "v1");
    assert_eq!(
        result["specification"]["missing_component_policy"],
        "reject"
    );
    assert_eq!(result["components"][0]["component"], "c1");
    assert!(
        (result["components"][0]["distance"]
            .as_f64()
            .expect("component distance")
            - 12.5_f64.sqrt())
        .abs()
            < 1e-12
    );
    assert!(
        (result["total_distance"].as_f64().expect("total distance") - 2.0 * 12.5_f64.sqrt()).abs()
            < 1e-12
    );
    assert_eq!(
        result["left_fingerprint"]["digest"].as_str().unwrap().len(),
        64
    );
    assert_eq!(
        result["right_fingerprint"]["digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
}
