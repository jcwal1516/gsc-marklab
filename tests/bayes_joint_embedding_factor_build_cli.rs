#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write as _, fs};
#[test]
fn embedding_factor_ir_has_rotational_constraint_and_validation_comparators() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("points.csv");
    let g = d.path().join("grid.csv");
    let o = d.path().join("out.json");
    let mut csv=String::from("point_id,x_um,y_um,location_covariate,location_offset,embedding_0,embedding_1,embedding_2\n");
    for i in 0..8 {
        writeln!(
            csv,
            "p{i},{},{},{},0,{},{},{}",
            0.1 + (i % 4) as f64 * 0.45,
            0.25 + (i / 4) as f64,
            (i % 2) as f64,
            i as f64,
            i as f64 * 0.5,
            (i % 3) as f64
        )
        .unwrap();
    }
    fs::write(&p, csv).unwrap();
    fs::write(
        &g,
        "ix,iy,location_covariate,location_offset\n0,0,0,0\n1,0,1,0\n0,1,0,0\n1,1,1,0\n",
    )
    .unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "build-joint-location-embedding-factor-model",
            "--points",
            p.to_str().unwrap(),
            "--grid",
            g.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "2",
            "--ymax-um",
            "2",
            "--grid-x",
            "2",
            "--grid-y",
            "2",
            "--factors",
            "2",
            "--field-amplitude",
            "1",
            "--field-length-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--loading-prior-sd",
            "1",
            "--noise-prior-sd",
            "1",
            "--out",
            o.to_str().unwrap(),
        ])
        .assert()
        .success();
    let r: serde_json::Value = serde_json::from_slice(&fs::read(o).unwrap()).unwrap();
    assert_eq!(r["format"], "marklab.joint_location_embedding_factor_model");
    assert_eq!(r["embedding_dimension"], 3);
    assert_eq!(r["factor_count"], 2);
    assert_eq!(r["feature_names"][0], "embedding_0");
    assert_eq!(
        r["model"]["rotational_identifiability"],
        "lower_triangular_first_k_loading_rows_with_positive_diagonal"
    );
    assert_eq!(
        r["model"]["validation_comparators"],
        "vector_variogram_and_kernel_mark_correlation"
    );
    assert_eq!(r["fit_state"], "not_fitted");
}
