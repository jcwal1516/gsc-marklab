#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn proper_and_intrinsic_car_match_hand_log_density_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let regions = directory.path().join("regions.csv");
    fs::write(&regions, "region_id\na\nb\nc\n").expect("regions");
    let edges = directory.path().join("edges.csv");
    fs::write(
        &edges,
        "source_region,target_region,weight\n\
a,b,1\n\
b,a,1\n\
b,c,1\n\
c,b,1\n",
    )
    .expect("edges");
    let field = directory.path().join("field.csv");
    fs::write(&field, "region_id,value\na,0.2\nb,-0.1\nc,-0.1\n").expect("field");
    let proper = directory.path().join("proper.json");
    let intrinsic = directory.path().join("intrinsic.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "car-density",
            "--regions",
            regions.to_str().unwrap(),
            "--edges",
            edges.to_str().unwrap(),
            "--field",
            field.to_str().unwrap(),
            "--mode",
            "proper",
            "--tau",
            "1.5",
            "--rho",
            "0.2",
            "--constraint-tolerance",
            "0.000000000001",
            "--island-policy",
            "reject",
            "--out",
            proper.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "car-density",
            "--regions",
            regions.to_str().unwrap(),
            "--edges",
            edges.to_str().unwrap(),
            "--field",
            field.to_str().unwrap(),
            "--mode",
            "intrinsic",
            "--tau",
            "1.5",
            "--rho",
            "0",
            "--constraint-tolerance",
            "0.000000000001",
            "--island-policy",
            "reject",
            "--out",
            intrinsic.to_str().unwrap(),
        ])
        .assert()
        .success();

    let proper: serde_json::Value =
        serde_json::from_slice(&fs::read(proper).unwrap()).expect("proper JSON");
    let intrinsic: serde_json::Value =
        serde_json::from_slice(&fs::read(intrinsic).unwrap()).expect("intrinsic JSON");
    assert_eq!(proper["format"], "marklab.car_density");
    assert_eq!(proper["mode"], "proper");
    assert!((proper["log_density"].as_f64().unwrap() + 1.8779553444319261).abs() <= 1e-12);
    assert_eq!(proper["rank_deficiency"], 0);
    assert_eq!(intrinsic["mode"], "intrinsic");
    assert!((intrinsic["log_density"].as_f64().unwrap() + 0.9506058139671261).abs() <= 1e-12);
    assert_eq!(intrinsic["rank_deficiency"], 1);
    assert_eq!(
        intrinsic["constraints"],
        serde_json::json!(["sum_to_zero:a,b,c"])
    );
}
