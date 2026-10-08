#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn all_predecessor_nngp_matches_full_gp_log_density() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("field.csv");
    fs::write(
        &input,
        "coordinate_id,x_um,field_value\n\
c-0,0,0.2\n\
c-1,1,-0.1\n\
c-2,2,0.4\n\
c-3,3,0.0\n",
    )
    .expect("field");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "nngp-density",
            "--input",
            input.to_str().expect("input path"),
            "--mean",
            "0",
            "--amplitude",
            "1",
            "--length-scale-um",
            "1.5",
            "--neighbors",
            "3",
            "--jitter",
            "0.000000000001",
            "--variance-tolerance",
            "0.000000000001",
            "--full-reference",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.nngp_density");
    assert_eq!(result["version"], 1);
    assert_eq!(result["ordering"], "ascending_physical_coordinate");
    assert_eq!(result["neighbor_counts"], serde_json::json!([0, 1, 2, 3]));
    assert!(result["conditional_variances"]
        .as_array()
        .unwrap()
        .iter()
        .all(|value| value.as_f64().unwrap() > 0.0));
    assert!(
        result["full_reference"]["absolute_log_density_difference"]
            .as_f64()
            .unwrap()
            <= 1e-9
    );
    assert_eq!(
        result["claim_status"],
        "experimental_approximation_diagnostic"
    );
}
