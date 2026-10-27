#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;
use std::io::Write;

#[test]
fn model_npy_vectors_are_joined_to_spatial_rows_without_cellvit_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let points = directory.path().join("points.csv");
    let vectors = directory.path().join("vectors.npy");
    let bins = directory.path().join("bins.csv");
    fs::write(&bins, "bin_id,lower_um,upper_um\nall,0,4\n").unwrap();
    for (rows, dimension) in [(3, 3), (3, 2_560), (3_000, 2_048)] {
        let mut coordinates = "object_id,x_um,y_um\n".to_owned();
        for row in 0..rows {
            let x = match row {
                0 => 0,
                1 => 1,
                2 => 3,
                _ => row * 10,
            };
            coordinates.push_str(&format!("p{row},{x},0\n"));
        }
        fs::write(&points, coordinates).unwrap();
        let dictionary =
            format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({rows}, {dimension}), }}");
        let padding = (16 - ((10 + dictionary.len() + 1) % 16)) % 16;
        let header = format!("{dictionary}{}\n", " ".repeat(padding));
        let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
        bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
        bytes.extend_from_slice(header.as_bytes());
        for row in 0..rows {
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
                &(rows * (rows - 1) / 2).to_string(),
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();
        let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(result["version"], 2);
        assert_eq!(result["embedding_dimension"], dimension);
        assert_eq!(result["object_count"], rows);
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
fn separate_npy_input_retains_file_and_decoded_memory_limits() {
    let directory = tempfile::tempdir().unwrap();
    let points = directory.path().join("points.csv");
    let vectors = directory.path().join("vectors.npy");
    let bins = directory.path().join("bins.csv");
    let output = directory.path().join("rejected.json");
    fs::write(&points, "object_id,x_um,y_um\na,0,0\nb,1,0\nc,3,0\n").unwrap();
    fs::write(&bins, "bin_id,lower_um,upper_um\nall,0,4\n").unwrap();

    // A valid, zero-filled 93.75 MiB float32 matrix fits the file ceiling, but
    // retaining it alongside its float64 spatial rows would exceed 256 MiB.
    let dictionary = "{'descr': '<f4', 'fortran_order': False, 'shape': (12000, 2048), }";
    let padding = (16 - ((10 + dictionary.len() + 1) % 16)) % 16;
    let header = format!("{dictionary}{}\n", " ".repeat(padding));
    let mut file = fs::File::create(&vectors).unwrap();
    file.write_all(b"\x93NUMPY\x01\x00").unwrap();
    file.write_all(&(header.len() as u16).to_le_bytes())
        .unwrap();
    file.write_all(header.as_bytes()).unwrap();
    let encoded_bytes = 10 + header.len() as u64 + 12_000 * 2_048 * 4;
    for (bytes, error) in [
        (encoded_bytes, "256 MiB retained-data limit"),
        (128 * 1024 * 1024 + 1, "128 MiB limit"),
    ] {
        file.set_len(bytes).unwrap();
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
            .failure()
            .stderr(predicates::str::contains(error));
        assert!(!output.exists());
    }
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
