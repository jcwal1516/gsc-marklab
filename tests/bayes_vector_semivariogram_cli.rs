#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn model_npy_vectors_are_joined_to_spatial_rows_without_cellvit_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let points = directory.path().join("points.csv");
    let vectors = directory.path().join("vectors.npy");
    let bins = directory.path().join("bins.csv");
    fs::write(&points, "object_id,x_um,y_um\na,0,0\nb,1,0\nc,3,0\n").unwrap();
    fs::write(&bins, "bin_id,lower_um,upper_um\nall,0,4\n").unwrap();
    for dimension in [3, 2_560] {
        let dictionary =
            format!("{{'descr': '<f4', 'fortran_order': False, 'shape': (3, {dimension}), }}");
        let padding = (16 - ((10 + dictionary.len() + 1) % 16)) % 16;
        let header = format!("{dictionary}{}\n", " ".repeat(padding));
        let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
        bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
        bytes.extend_from_slice(header.as_bytes());
        for row in 0..3 {
            for column in 0..dimension {
                let value: f32 =
                    if (column == 0 && row > 0) || (column == dimension - 1 && row == 2) {
                        2.0
                    } else {
                        0.0
                    };
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        fs::write(&vectors, &bytes).unwrap();
        let output = directory.path().join(format!("output-{dimension}.json"));
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "bayes",
                "vector-semivariogram",
                "--input",
                points.to_str().unwrap(),
                "--vectors",
                vectors.to_str().unwrap(),
                "--bins",
                bins.to_str().unwrap(),
                "--maximum-pair-visits",
                "3",
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();
        let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(result["version"], 2);
        assert_eq!(result["embedding_dimension"], dimension);
        assert_eq!(result["object_count"], 3);
        assert_eq!(result["curve"][0]["pair_count"], 3);
        assert!((result["curve"][0]["semivariance"].as_f64().unwrap() - 8.0 / 3.0).abs() < 1e-12);
        assert_eq!(
            result["vectors_sha256"],
            marklab::workflow::ContentDigest::from_bytes(&bytes).to_string()
        );
    }
    fs::write(&points, "object_id,x_um,y_um\na,0,0\nb,1,0\n").unwrap();
    let rejected = directory.path().join("rejected.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "vector-semivariogram",
            "--input",
            points.to_str().unwrap(),
            "--vectors",
            vectors.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--maximum-pair-visits",
            "3",
            "--out",
            rejected.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("row count"));
    assert!(!rejected.exists());
}

#[test]
fn vector_semivariogram_matches_weighted_hand_oracle_and_rotation() {
    let directory = tempfile::tempdir().unwrap();
    let points = directory.path().join("points.csv");
    let rotated = directory.path().join("rotated.csv");
    let bins = directory.path().join("bins.csv");
    let weights = directory.path().join("weights.csv");
    let output = directory.path().join("output.json");
    let rotated_output = directory.path().join("rotated-output.json");

    fs::write(
        &points,
        "object_id,x_um,y_um,embedding_0,embedding_1\na,0,0,0,0\nb,1,0,2,0\nc,3,0,2,2\n",
    )
    .unwrap();
    fs::write(
        &rotated,
        "object_id,x_um,y_um,embedding_0,embedding_1\na,0,0,0,0\nb,1,0,0,-2\nc,3,0,2,-2\n",
    )
    .unwrap();
    fs::write(&bins, "bin_id,lower_um,upper_um\nnear,0,2\nfar,2,4\n").unwrap();
    fs::write(
        &weights,
        "left_object_id,right_object_id,weight\na,b,1\na,c,1\nb,c,2\n",
    )
    .unwrap();

    for (input, out) in [(&points, &output), (&rotated, &rotated_output)] {
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "bayes",
                "vector-semivariogram",
                "--input",
                input.to_str().unwrap(),
                "--bins",
                bins.to_str().unwrap(),
                "--weights",
                weights.to_str().unwrap(),
                "--maximum-pair-visits",
                "3",
                "--out",
                out.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    let rotated_result: serde_json::Value =
        serde_json::from_slice(&fs::read(rotated_output).unwrap()).unwrap();

    assert_eq!(result["format"], "marklab.vector_semivariogram");
    assert_eq!(result["version"], 1);
    assert_eq!(result["coordinate_unit"], "micrometer");
    assert_eq!(result["embedding_dimension"], 2);
    assert_eq!(result["pair_visits"], 3);
    assert_eq!(result["curve"][0]["bin_id"], "near");
    assert_eq!(result["curve"][0]["pair_count"], 1);
    assert_eq!(result["curve"][0]["semivariance"], 2.0);
    assert_eq!(result["curve"][0]["inference_eligible"], false);
    assert_eq!(result["curve"][1]["bin_id"], "far");
    assert_eq!(result["curve"][1]["pair_count"], 2);
    assert!((result["curve"][1]["semivariance"].as_f64().unwrap() - 8.0 / 3.0).abs() < 1e-12);
    assert_eq!(result["curve"][1]["weight_sum"], 3.0);
    assert_eq!(result["curve"][1]["inference_eligible"], true);
    assert_eq!(result["curve"], rotated_result["curve"]);
}
