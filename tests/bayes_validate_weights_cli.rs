#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn symmetric_row_standardized_weights_report_components_and_island() {
    let directory = tempfile::tempdir().expect("tempdir");
    let regions = directory.path().join("regions.csv");
    fs::write(&regions, "region_id\na\nb\nc\nd\n").expect("regions");
    let edges = directory.path().join("edges.csv");
    fs::write(
        &edges,
        "source_region,target_region,weight\n\
a,b,2\n\
b,a,2\n\
b,c,1\n\
c,b,1\n",
    )
    .expect("edges");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    for output in [&first, &second] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "bayes",
                "validate-weights",
                "--regions",
                regions.to_str().expect("region path"),
                "--edges",
                edges.to_str().expect("edge path"),
                "--symmetry",
                "required",
                "--diagonal",
                "zero",
                "--normalization",
                "row-standardize",
                "--out",
                output.to_str().expect("output path"),
            ])
            .assert()
            .success();
    }

    let bytes = fs::read(&first).expect("first result");
    assert_eq!(bytes, fs::read(&second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
    assert_eq!(result["format"], "marklab.validated_spatial_weights");
    assert_eq!(result["version"], 1);
    assert_eq!(result["components"], 2);
    assert_eq!(result["islands"], serde_json::json!(["d"]));
    assert_eq!(result["row_sums"]["a"], 1.0);
    assert_eq!(result["row_sums"]["b"], 1.0);
    assert_eq!(result["row_sums"]["c"], 1.0);
    assert_eq!(result["row_sums"]["d"], 0.0);
    assert_eq!(result["digest_sha256"].as_str().unwrap().len(), 64);
}
