#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn constrained_gmrf_matches_projected_diagonal_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let regions = directory.path().join("regions.csv");
    fs::write(&regions, "region_id\na\nb\nc\n").expect("regions");
    let precision = directory.path().join("precision.csv");
    fs::write(
        &precision,
        "source_region,target_region,value\na,a,2\nb,b,3\nc,c,4\n",
    )
    .expect("precision");
    let field = directory.path().join("field.csv");
    fs::write(&field, "region_id,value\na,0.5\nb,-0.5\nc,0.25\n").expect("field");
    let constraints = directory.path().join("constraints.csv");
    fs::write(
        &constraints,
        "constraint_id,region_id,coefficient\nsum_ab,a,1\nsum_ab,b,1\n",
    )
    .expect("constraints");
    let output = directory.path().join("gmrf.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "gmrf-density",
            "--regions",
            regions.to_str().unwrap(),
            "--precision",
            precision.to_str().unwrap(),
            "--field",
            field.to_str().unwrap(),
            "--constraints",
            constraints.to_str().unwrap(),
            "--constraint-tolerance",
            "0.000000000001",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("GMRF JSON");
    assert_eq!(result["format"], "marklab.gmrf_density");
    assert_eq!(result["version"], 1);
    assert_eq!(result["dimension"], 3);
    assert_eq!(result["constrained_dimension"], 2);
    assert_eq!(result["rank_deficiency"], 1);
    assert_eq!(result["constraints"], serde_json::json!(["sum_ab"]));
    assert!((result["log_determinant"].as_f64().unwrap() - 10.0_f64.ln()).abs() <= 1e-12);
    assert!((result["quadratic"].as_f64().unwrap() - 1.5).abs() <= 1e-12);
    assert!((result["log_density"].as_f64().unwrap() + 1.4365845199123224).abs() <= 1e-12);
    assert_eq!(
        result["claim_status"],
        "experimental_field_density_diagnostic"
    );
}
