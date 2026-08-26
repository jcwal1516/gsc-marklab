#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn single_pixel_raster_minkowski_and_disk_curves_match_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("morphology.json");
    fs::write(
        &input,
        r#"{
  "mask":[
    [false,false,false,false,false],
    [false,false,false,false,false],
    [false,false,true,false,false],
    [false,false,false,false,false],
    [false,false,false,false,false]
  ],
  "pixel_size_um":2.0,
  "connectivity":4,
  "crofton_directions":4,
  "radii_um":[0.0,2.0],
  "timeout_seconds":30
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "raster-morphology",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.raster_morphology");
    assert_eq!(result["backend"]["name"], "scikit-image");
    assert_eq!(result["backend"]["version"], "0.26.0");
    assert_eq!(result["baseline"]["area_um2"], 4.0);
    assert_eq!(result["baseline"]["euler_characteristic"], 1);
    assert!(result["baseline"]["perimeter_um"].as_f64().unwrap() > 0.0);
    assert_eq!(result["curves"][0]["dilation"], result["baseline"]);
    assert_eq!(result["curves"][0]["erosion"], result["baseline"]);
    assert_eq!(result["curves"][1]["dilation"]["area_um2"], 20.0);
    assert_eq!(result["curves"][1]["dilation"]["euler_characteristic"], 1);
    assert_eq!(result["curves"][1]["erosion"]["area_um2"], 0.0);
    assert_eq!(result["curves"][1]["erosion"]["euler_characteristic"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_supplied_binary_raster"
    );
}
