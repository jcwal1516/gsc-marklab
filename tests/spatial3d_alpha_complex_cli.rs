#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn gudhi_3d_alpha_complex_recovers_regular_tetrahedron_filtration() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("alpha3d.json");
    let fixture = serde_json::json!({
        "coordinate_frame":"volume_um",
        "points":[
            {"point_id":"a","coordinates_um":[0.0,0.0,0.0]},
            {"point_id":"b","coordinates_um":[2.0,0.0,0.0]},
            {"point_id":"c","coordinates_um":[1.0,3.0_f64.sqrt(),0.0]},
            {"point_id":"d","coordinates_um":[1.0,3.0_f64.sqrt()/3.0,2.0*(2.0_f64/3.0).sqrt()]}
        ],
        "maximum_squared_alpha_um2":2.0,
        "maximum_homology_dimension":2,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "alpha-complex",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.alpha_complex_3d");
    assert_eq!(result["backend"]["version"], "gudhi-3.13.0");
    assert_eq!(
        result["simplex_counts_by_dimension"],
        serde_json::json!([4, 6, 4, 1])
    );
    assert!(
        (result["tetrahedra"][0]["squared_alpha_um2"]
            .as_f64()
            .unwrap()
            - 1.5)
            .abs()
            < 1e-10
    );
    assert_eq!(result["maximum_simplex_dimension"], 3);
    assert_eq!(result["boundary_squared_max_absolute"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_exact_3d_alpha_complex"
    );

    // Cutting before edges, faces, or the filling tetrahedron must retain the
    // essential features in the actual highest dimension of the complex.
    for (cutoff, dimension, counts) in [
        (0.5, 0, [4, 0, 0]),
        (1.1, 1, [4, 3, 0]),
        (1.4, 2, [4, 3, 1]),
    ] {
        let mut truncated = fixture.clone();
        truncated["maximum_squared_alpha_um2"] = serde_json::json!(cutoff);
        fs::write(&input, serde_json::to_vec(&truncated).unwrap()).unwrap();
        let truncated_output = directory.path().join(format!("cutoff-{cutoff}.json"));
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "spatial3d",
                "alpha-complex",
                "--input",
                input.to_str().unwrap(),
                "--out",
                truncated_output.to_str().unwrap(),
            ])
            .assert()
            .success();
        let result: serde_json::Value =
            serde_json::from_slice(&fs::read(truncated_output).unwrap()).unwrap();
        assert_eq!(result["maximum_simplex_dimension"], dimension);
        for (homology, count) in counts.into_iter().enumerate() {
            let intervals = result["persistence_diagrams"][homology.to_string()]
                .as_array()
                .unwrap();
            assert_eq!(intervals.len(), count, "cutoff {cutoff}, H{homology}");
            if homology == dimension as usize {
                assert!(intervals.iter().all(|interval| {
                    interval["essential"] == true && interval["death_squared_alpha_um2"].is_null()
                }));
            }
        }
    }
}
