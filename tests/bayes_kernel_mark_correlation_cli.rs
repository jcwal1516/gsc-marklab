#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::{fmt::Write as _, fs};

#[test]
fn kernel_mark_correlation_fits_rbf_only_on_training_units_and_matches_curve_oracle() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("embeddings.csv");
    let bins = directory.path().join("bins.csv");
    let output = directory.path().join("output.json");
    let mut csv =
        String::from("object_id,biological_unit,split,x_um,y_um,embedding_0,embedding_1\n");
    for (unit, split, second) in [
        ("training-unit", "train", [0.0, 0.0, 0.0, 0.0]),
        ("held-out-unit", "test", [0.0, 1000.0, -1000.0, 500.0]),
    ] {
        for index in 0..4 {
            writeln!(
                csv,
                "{unit}-{index},{unit},{split},{},0,{index},{}",
                [0, 1, 3, 6][index],
                second[index]
            )
            .unwrap();
        }
    }
    fs::write(&input, csv).unwrap();
    fs::write(
        &bins,
        "bin_id,lower_um,upper_um\nnear,0,2\nmid,2,4\nfar,4,7\n",
    )
    .unwrap();

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "kernel-mark-correlation",
            "--input",
            input.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--kernel",
            "rbf",
            "--global-reference-tolerance",
            "0.000000000001",
            "--maximum-pair-visits",
            "18",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.kernel_mark_correlation");
    assert_eq!(result["version"], 1);
    assert_eq!(result["kernel"]["kind"], "rbf");
    assert_eq!(result["kernel"]["fit_split"], "train");
    assert_eq!(
        result["kernel"]["training_biological_units"],
        serde_json::json!(["training-unit"])
    );
    assert_eq!(result["kernel"]["center"], serde_json::json!([1.5, 0.0]));
    assert_eq!(result["kernel"]["scale"], 1.5);
    assert_eq!(result["kernel"]["positive_semidefinite"], true);

    let train = result["curves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|curve| curve["split"] == "train")
        .unwrap();
    let k1 = (-1.0_f64 / 4.5).exp();
    let k2 = (-4.0_f64 / 4.5).exp();
    let k3 = (-2.0_f64).exp();
    let reference = (3.0 * k1 + 2.0 * k2 + k3) / 6.0;
    let expected = [k1, (k2 + 2.0 * k1) / 3.0, (k3 + k2) / 2.0];
    assert!((train["global_reference"].as_f64().unwrap() - reference).abs() < 1e-12);
    for (index, expected) in expected.into_iter().enumerate() {
        assert!(
            (train["rows"][index]["raw_similarity"].as_f64().unwrap() - expected).abs() < 1e-12
        );
        assert!(
            (train["rows"][index]["normalized_similarity"]
                .as_f64()
                .unwrap()
                - expected / reference)
                .abs()
                < 1e-12
        );
    }
}
