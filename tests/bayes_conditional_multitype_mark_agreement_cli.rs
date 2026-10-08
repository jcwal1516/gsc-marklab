#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_conditional_multitype_mark_fit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("points.csv");
    let output = directory.path().join("agreement.json");
    let mut csv = String::from("point_id,x_um,y_um,type_id\n");
    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
        for point_index in 0..30 {
            let x = type_index as f64 * 100.0 + (point_index % 6) as f64;
            let y = (point_index / 6) as f64;
            writeln!(csv, "{type_id}-{point_index:02},{x},{y},{type_id}").unwrap();
        }
    }
    fs::write(&input, csv).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "conditional-multitype-mark-agreement",
            "--input",
            input.to_str().unwrap(),
            "--reference-type",
            "A",
            "--radius-um",
            "1.5",
            "--intercept-prior-sd",
            "2",
            "--interaction-prior-sd",
            "1",
            "--chains",
            "2",
            "--tune",
            "750",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260829",
            "--maximum-points",
            "100",
            "--maximum-types",
            "4",
            "--maximum-neighbor-visits",
            "10000",
            "--maximum-edges",
            "1000",
            "--maximum-draw-parameter-work",
            "40000",
            "--maximum-working-bytes",
            "1048576",
            "--pymc-maximum-tree-depth",
            "12",
            "--numpyro-maximum-tree-depth",
            "12",
            "--maximum-standardized-difference",
            "2.5",
            "--minimum-absolute-tolerance",
            "0.05",
            "--timeout-seconds",
            "300",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.conditional_multitype_mark_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["comparison"]["intercepts"]["element_count"], 3);
    assert_eq!(result["comparison"]["pair_potentials"]["element_count"], 6);
    assert_eq!(result["comparison"]["pair_affinities"]["element_count"], 3);
    assert_eq!(result["comparison"]["all_pass"], true, "{result}");
    assert_eq!(result["statistical_unit"], "one_fixed_location_pattern");
}
