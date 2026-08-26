#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write as _, fs};
#[test]
fn continuous_mark_ir_identifies_shared_field_and_separate_model_comparison() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("points.csv");
    let g = d.path().join("grid.csv");
    let o = d.path().join("out.json");
    let mut csv = String::from(
        "point_id,x_um,y_um,mark_y,location_covariate,location_offset,mark_covariate\n",
    );
    for i in 0..8 {
        let x = (i % 4) as f64 * 0.45 + 0.1;
        let y = (i / 4) as f64 + 0.25;
        writeln!(
            csv,
            "p{i},{x},{y},{},{},0,{}",
            1.0 + i as f64 * 0.2,
            (i % 2) as f64,
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
            "build-joint-continuous-mark-model",
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
            "--known-mark-noise-sd",
            "0.2",
            "--shared-field-amplitude",
            "1",
            "--shared-field-length-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--mark-loading-prior-sd",
            "1",
            "--private-field-prior-sd",
            "1",
            "--out",
            o.to_str().unwrap(),
        ])
        .assert()
        .success();
    let r: serde_json::Value = serde_json::from_slice(&fs::read(o).unwrap()).unwrap();
    assert_eq!(r["format"], "marklab.joint_location_continuous_mark_model");
    assert_eq!(r["fit_state"], "not_fitted");
    assert_eq!(r["model"]["location_shared_loading"], "fixed_one");
    assert_eq!(r["model"]["mark_shared_loading"], "positive_half_normal");
    assert_eq!(r["model"]["kernel"], "matern_3_2_euclidean_2d");
    assert_eq!(
        r["model"]["separate_model_comparison"],
        "required_nested_zero_shared_loading"
    );
}
