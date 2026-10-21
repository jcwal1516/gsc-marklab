#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::{fmt::Write as _, fs};

#[test]
fn projected_variograms_fit_pca_on_training_units_and_apply_max_t() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("embeddings.csv");
    let bins = directory.path().join("bins.csv");
    let output = directory.path().join("output.json");
    let mut csv = String::from(
        "object_id,biological_unit,split,permutation_stratum,x_um,y_um,embedding_0,embedding_1\n",
    );
    for (unit, split, second) in [
        ("training-unit", "train", [0.0, 0.0, 0.0, 0.0]),
        ("held-out-unit", "test", [0.0, 1000.0, -1000.0, 500.0]),
    ] {
        for index in 0..4 {
            writeln!(
                csv,
                "{unit}-{index},{unit},{split},{unit},{},0,{index},{}",
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
            "projected-embedding-variograms",
            "--input",
            input.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--components",
            "1",
            "--permutations",
            "20",
            "--seed",
            "731",
            "--maximum-pair-visits",
            "12",
            "--timeout-seconds",
            "120",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.projected_embedding_variograms");
    assert_eq!(result["version"], 1);
    assert_eq!(result["backend"]["name"], "scipy");
    assert_eq!(result["backend"]["version"], "1.18.1");
    assert_eq!(result["projection_artifact"]["fit_split"], "train");
    assert_eq!(
        result["projection_artifact"]["training_biological_units"],
        serde_json::json!(["training-unit"])
    );
    assert_eq!(
        result["projection_artifact"]["center"],
        serde_json::json!([1.5, 0.0])
    );
    let loading = result["projection_artifact"]["components"][0]
        .as_array()
        .unwrap();
    assert!((loading[0].as_f64().unwrap() - 1.0).abs() < 1e-12);
    assert!(loading[1].as_f64().unwrap().abs() < 1e-12);
    assert_eq!(result["multiplicity_control"], "single_step_max_t");
    assert_eq!(result["permutations"], 20);

    let curves = result["curves"].as_array().unwrap();
    let train = curves
        .iter()
        .find(|curve| curve["split"] == "train")
        .unwrap();
    let test = curves
        .iter()
        .find(|curve| curve["split"] == "test")
        .unwrap();
    assert_eq!(train["family_size"], 3);
    assert_eq!(test["family_size"], 3);
    assert_eq!(train["rows"], test["rows"]);
    assert_eq!(train["rows"][0]["pair_count"], 1);
    assert_eq!(train["rows"][1]["pair_count"], 3);
    assert_eq!(train["rows"][2]["pair_count"], 2);
    // Preserve the PCG64 seed stream and tied max-t comparisons across native accumulation.
    for (row, (semivariance, mean, sd, adjusted)) in train["rows"].as_array().unwrap().iter().zip([
        (0.5, 1.3, 1.0563093645728086, 1.0),
        (1.0, 1.7333333333333332, 0.6805570473787207, 14.0 / 21.0),
        (3.25, 1.75, 1.0130724502589556, 6.0 / 21.0),
    ]) {
        for (field, expected) in [
            ("semivariance", semivariance),
            ("permutation_mean", mean),
            ("permutation_sd", sd),
        ] {
            assert!((row[field].as_f64().unwrap() - expected).abs() < 1e-14);
        }
        assert_eq!(row["max_t_adjusted_p"].as_f64().unwrap(), adjusted);
    }
    for row in train["rows"].as_array().unwrap() {
        if let Some(adjusted_p) = row["max_t_adjusted_p"].as_f64() {
            assert!((1.0 / 21.0..=1.0).contains(&adjusted_p));
        }
    }
}

#[test]
fn projected_variograms_keep_biological_units_separate_in_pairs_and_permutations() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("embeddings.csv");
    let bins = directory.path().join("bins.csv");
    fs::write(&bins, "bin_id,lower_um,upper_um\nnear,0,2\nfar,2,2000\n").unwrap();
    let mut curves = Vec::new();
    for (run, shift) in [(0, 0), (1, 1000)] {
        let output = directory.path().join(format!("output-{run}.json"));
        let mut csv = String::from(
            "object_id,biological_unit,split,permutation_stratum,x_um,y_um,embedding_0,embedding_1\n",
        );
        for (unit, split, base, translated) in [
            ("a", "train", 0, false),
            ("b", "train", 10, true),
            ("c", "test", 2, false),
            ("d", "test", 12, true),
        ] {
            for index in 0..2 {
                let x = index + if translated { shift } else { 0 };
                // A shared stratum name must not allow donors from another biological unit.
                writeln!(
                    csv,
                    "{unit}{index},{unit},{split},shared,{x},0,{},0",
                    base + index
                )
                .unwrap();
            }
        }
        fs::write(&input, csv).unwrap();
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "bayes",
                "projected-embedding-variograms",
                "--input",
                input.to_str().unwrap(),
                "--bins",
                bins.to_str().unwrap(),
                "--components",
                "1",
                "--permutations",
                "20",
                "--seed",
                "731",
                "--maximum-pair-visits",
                "4",
                "--timeout-seconds",
                "120",
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();
        let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        for curve in result["curves"].as_array().unwrap() {
            assert_eq!(curve["pair_visits"], 2);
            assert_eq!(curve["family_size"], 1);
            assert_eq!(curve["rows"][0]["pair_count"], 2);
            assert_eq!(curve["rows"][0]["semivariance"], 0.5);
            assert_eq!(curve["rows"][0]["permutation_mean"], 0.5);
            assert_eq!(curve["rows"][0]["permutation_sd"], serde_json::Value::Null);
            assert_eq!(
                curve["rows"][0]["max_t_adjusted_p"],
                serde_json::Value::Null
            );
            assert_eq!(curve["rows"][1]["pair_count"], 0);
            assert_eq!(curve["rows"][1]["semivariance"], serde_json::Value::Null);
        }
        curves.push(result["curves"].clone());
    }
    assert_eq!(curves[0], curves[1]);
}

#[test]
fn projected_variograms_accept_opposite_sign_loadings_with_tied_magnitudes() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("embeddings.csv");
    let bins = directory.path().join("bins.csv");
    let output = directory.path().join("output.json");
    let mut csv = String::from(
        "object_id,biological_unit,split,permutation_stratum,x_um,y_um,embedding_0,embedding_1\n",
    );
    for (unit, split) in [("a", "train"), ("b", "test")] {
        for index in 0..4 {
            writeln!(
                csv,
                "{unit}{index},{unit},{split},{unit},{index},0,{index},{}",
                -index
            )
            .unwrap();
        }
    }
    fs::write(&input, csv).unwrap();
    fs::write(&bins, "bin_id,lower_um,upper_um\nall,0,10\n").unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "projected-embedding-variograms",
            "--input",
            input.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--components",
            "1",
            "--permutations",
            "20",
            "--seed",
            "731",
            "--maximum-pair-visits",
            "12",
            "--timeout-seconds",
            "120",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    let loading = &result["projection_artifact"]["components"][0];
    assert!((loading[0].as_f64().unwrap() - 0.5_f64.sqrt()).abs() < 1e-12);
    assert!((loading[1].as_f64().unwrap() + 0.5_f64.sqrt()).abs() < 1e-12);
    for curve in result["curves"].as_array().unwrap() {
        assert!((curve["rows"][0]["semivariance"].as_f64().unwrap() - 10.0 / 3.0).abs() < 1e-12);
    }
}
