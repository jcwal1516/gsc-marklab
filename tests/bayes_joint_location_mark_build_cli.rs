#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::fs;
#[test]
fn joint_categorical_mark_ir_preserves_factorization_and_null_comparison() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("points.csv");
    let g = d.path().join("grid.csv");
    let o = d.path().join("model.json");
    fs::write(&p,"point_id,x_um,y_um,mark_id,location_covariate,location_offset,mark_covariate,neighborhood_effect\np1,0.25,0.25,A,-1,0,0.2,0\np2,1.25,0.25,B,1,0,-0.3,1\np3,0.25,1.25,A,-1,0,0.4,1\np4,1.25,1.25,B,1,0,-0.1,0\n").unwrap();
    fs::write(
        &g,
        "ix,iy,location_covariate,location_offset\n0,0,-1,0\n1,0,1,0\n0,1,-1,0\n1,1,1,0\n",
    )
    .unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "build-joint-location-mark-model",
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
            "--reference-mark",
            "A",
            "--location-prior-sd",
            "2",
            "--mark-coefficient-prior-sd",
            "1",
            "--mark-field-prior-sd",
            "1",
            "--out",
            o.to_str().unwrap(),
        ])
        .assert()
        .success();
    let r: serde_json::Value = serde_json::from_slice(&fs::read(o).unwrap()).unwrap();
    assert_eq!(r["format"], "marklab.joint_location_categorical_mark_model");
    assert_eq!(r["fit_state"], "not_fitted");
    assert_eq!(r["point_count"], 4);
    assert_eq!(r["mark_counts"]["A"], 2);
    assert_eq!(r["mark_counts"]["B"], 2);
    assert_eq!(r["model"]["reference_mark"], "A");
    assert_eq!(
        r["model"]["joint_likelihood"],
        "point_process_location_plus_conditional_categorical_mark"
    );
    assert_eq!(
        r["model"]["random_labeling_comparison"],
        "required_nested_zero_neighborhood_and_mark_fields"
    );
}
