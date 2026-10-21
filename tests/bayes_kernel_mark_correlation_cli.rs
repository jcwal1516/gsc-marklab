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

#[test]
fn kernel_spatial_pairs_stay_within_biological_units_under_coordinate_translations() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("embeddings.csv");
    let bins = directory.path().join("bins.csv");
    fs::write(&bins, "bin_id,lower_um,upper_um\nnear,0,2\nfar,2,2000\n").unwrap();
    let mut curves = Vec::new();
    for (run, shift, budget) in [(0, 0, 18), (1, 1000, 10)] {
        let output = directory.path().join(format!("output-{run}.json"));
        let mut csv =
            String::from("object_id,biological_unit,split,x_um,y_um,embedding_0,embedding_1\n");
        for (unit, split, base, translated) in [
            ("a", "train", 0, false),
            ("b", "train", 10, true),
            ("c", "test", 2, false),
            ("d", "test", 12, true),
        ] {
            for index in 0..2 {
                let x = index + if translated { shift } else { 0 };
                writeln!(csv, "{unit}{index},{unit},{split},{x},0,{},0", base + index).unwrap();
            }
        }
        fs::write(&input, csv).unwrap();
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
                "1e-12",
                "--maximum-pair-visits",
                &budget.to_string(),
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();
        let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(result["kernel"]["scale"], 9.5);
        let expected = (-1.0_f64 / (2.0 * 9.5 * 9.5)).exp();
        for curve in result["curves"].as_array().unwrap() {
            assert_eq!(curve["pair_visits"], 2);
            assert_eq!(curve["rows"][0]["pair_count"], 2);
            assert_eq!(curve["rows"][1]["pair_count"], 0);
            assert_eq!(curve["rows"][1]["raw_similarity"], serde_json::Value::Null);
            assert!((curve["global_reference"].as_f64().unwrap() - expected).abs() < 1e-12);
            assert!(
                (curve["rows"][0]["raw_similarity"].as_f64().unwrap() - expected).abs() < 1e-12
            );
            assert_eq!(curve["rows"][0]["normalized_similarity"], 1.0);
        }
        curves.push(result["curves"].clone());
    }
    assert_eq!(curves[0], curves[1]);
    let project = directory.path().join("project");
    for run in 0..2 {
        let output = directory.path().join(format!("project-{run}.json"));
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "project",
                "kernel-mark-correlation",
                "--project",
                project.to_str().unwrap(),
                "--input",
                input.to_str().unwrap(),
                "--bins",
                bins.to_str().unwrap(),
                "--kernel",
                "rbf",
                "--global-reference-tolerance",
                "1e-12",
                "--maximum-points",
                "8",
                "--maximum-dimension",
                "2",
                "--maximum-pair-visits",
                "10",
                "--memory-budget-mib",
                "8",
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success()
            .stderr(predicates::str::contains(if run == 0 {
                "cache_status=miss"
            } else {
                "cache_status=hit"
            }));
        let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(result["curves"], curves[0]);
    }
}
