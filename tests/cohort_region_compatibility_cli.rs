#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn region_compatibility_is_descriptive_and_propagates_uncertainty() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("components.csv");
    fs::write(
        &input,
        "sample_id,component,axis,value,uncertainty,weight\n\
region-a,c1,0,0,0.1,2\n\
region-a,c1,1,0,0.1,2\n\
region-b,c1,0,3,0.1,2\n\
region-b,c1,1,4,0.1,2\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "region-compatibility",
            "--input",
            input.to_str().expect("input path"),
            "--left-sample",
            "region-a",
            "--right-sample",
            "region-b",
            "--spec-version",
            "v1",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.region_compatibility");
    assert_eq!(result["version"], 1);
    assert_eq!(
        result["uncertainty_method"],
        "independent_endpoint_delta_method"
    );
    assert_eq!(
        result["inferential_scope"],
        "within_patient_descriptive_only"
    );
    assert!(
        (result["total_distance"].as_f64().expect("total distance") - 2.0 * 12.5_f64.sqrt()).abs()
            < 1e-12
    );
    assert!(
        (result["total_distance_standard_uncertainty"]
            .as_f64()
            .expect("uncertainty")
            - 0.2)
            .abs()
            < 1e-12
    );
    assert!(
        (result["components"][0]["distance_standard_uncertainty"]
            .as_f64()
            .expect("component uncertainty")
            - 0.1)
            .abs()
            < 1e-12
    );
}
