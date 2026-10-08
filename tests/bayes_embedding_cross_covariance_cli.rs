#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn embedding_cross_covariance_matches_hand_matrix_and_rotation_invariants() {
    let directory = tempfile::tempdir().unwrap();
    let points = directory.path().join("points.csv");
    let rotated = directory.path().join("rotated.csv");
    let bins = directory.path().join("bins.csv");
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

    for (input, out) in [(&points, &output), (&rotated, &rotated_output)] {
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "bayes",
                "embedding-cross-covariance-by-distance",
                "--input",
                input.to_str().unwrap(),
                "--bins",
                bins.to_str().unwrap(),
                "--maximum-pair-visits",
                "3",
                "--maximum-matrix-elements",
                "12",
                "--out",
                out.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    let rotated_result: serde_json::Value =
        serde_json::from_slice(&fs::read(rotated_output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.embedding_cross_covariance_by_distance"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["embedding_dimension"], 2);
    assert_eq!(result["pair_visits"], 3);
    assert_eq!(
        result["global_mean"],
        serde_json::json!([4.0 / 3.0, 2.0 / 3.0])
    );
    assert_eq!(result["summaries"][0]["bin_id"], "near");
    assert_eq!(result["summaries"][0]["pair_count"], 1);
    assert!((result["summaries"][0]["trace"].as_f64().unwrap() + 4.0 / 9.0).abs() < 1e-12);
    assert!(
        (result["summaries"][0]["frobenius_norm"].as_f64().unwrap() - 88.0_f64.sqrt() / 9.0).abs()
            < 1e-12
    );
    let matrix = result["matrix_artifact"]["matrices"][0]["matrix"]
        .as_array()
        .unwrap();
    let expected = [[-8.0 / 9.0, 2.0 / 9.0], [2.0 / 9.0, 4.0 / 9.0]];
    for row in 0..2 {
        for column in 0..2 {
            assert!((matrix[row][column].as_f64().unwrap() - expected[row][column]).abs() < 1e-12);
        }
    }
    for bin in 0..2 {
        assert_eq!(
            result["summaries"][bin]["pair_count"],
            rotated_result["summaries"][bin]["pair_count"]
        );
        assert!(
            (result["summaries"][bin]["trace"].as_f64().unwrap()
                - rotated_result["summaries"][bin]["trace"].as_f64().unwrap())
            .abs()
                < 1e-12
        );
        assert!(
            (result["summaries"][bin]["frobenius_norm"].as_f64().unwrap()
                - rotated_result["summaries"][bin]["frobenius_norm"]
                    .as_f64()
                    .unwrap())
            .abs()
                < 1e-12
        );
    }
}
