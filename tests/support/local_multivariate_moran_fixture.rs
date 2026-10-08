#![allow(dead_code)]

use std::{fs, path::Path};

use assert_cmd::Command;

pub fn write_inputs(root: &Path) {
    fs::write(
        root.join("points.csv"),
        "point_id,permutation_stratum,x_um,y_um,embedding_0,embedding_1\n\
p0,tumor,0,0,-1,-1\n\
p1,tumor,1,0,-1,-1\n\
p2,tumor,2,0,1,1\n\
p3,tumor,3,0,1,1\n",
    )
    .expect("points");
    fs::write(
        root.join("window.geojson"),
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("window");
}

pub fn arguments(root: &Path, out: &Path) -> Vec<String> {
    vec![
        "--input".into(),
        root.join("points.csv").to_string_lossy().into_owned(),
        "--window".into(),
        root.join("window.geojson").to_string_lossy().into_owned(),
        "--radius-um".into(),
        "1.1".into(),
        "--permutations".into(),
        "31".into(),
        "--seed".into(),
        "7103".into(),
        "--maximum-points".into(),
        "4".into(),
        "--maximum-dimension".into(),
        "2".into(),
        "--maximum-directed-edges".into(),
        "6".into(),
        "--maximum-permutation-edge-evaluations".into(),
        "186".into(),
        "--memory-budget-mib".into(),
        "8".into(),
        "--out".into(),
        out.to_string_lossy().into_owned(),
    ]
}

pub fn command(root: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("marklab binary");
    command.args(["numerics", "local-multivariate-moran"]);
    command.args(arguments(root, out));
    command
}
