#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::fs;
#[test]
fn typed_pair_potentials_and_radii_match_hand_oracle() {
    let d = tempfile::tempdir().unwrap();
    let points = d.path().join("points.csv");
    let base = d.path().join("base.csv");
    let pairs = d.path().join("pairs.csv");
    let out = d.path().join("out.json");
    fs::write(&points, "point_id,x_um,y_um,type_id\na,0,0,A\nb,3,0,B\n").unwrap();
    fs::write(
        &base,
        format!(
            "type_id,log_baseline_per_um2\nA,{:.17}\nB,0\n",
            2.0_f64.ln()
        ),
    )
    .unwrap();
    fs::write(&pairs,format!("type_a,type_b,log_pair_potential,radius_um\nA,A,{:.17},2\nA,B,{:.17},3\nB,A,{:.17},3\nB,B,0,2\n",0.5_f64.ln(),2.0_f64.ln(),2.0_f64.ln())).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "multitype-papangelou",
            "--points",
            points.to_str().unwrap(),
            "--baselines",
            base.to_str().unwrap(),
            "--interactions",
            pairs.to_str().unwrap(),
            "--proposal-type",
            "A",
            "--proposal-x-um",
            "1",
            "--proposal-y-um",
            "0",
            "--maximum-visits",
            "100",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    let r: serde_json::Value = serde_json::from_slice(&fs::read(out).unwrap()).unwrap();
    assert_eq!(r["format"], "marklab.multitype_papangelou");
    assert!((r["papangelou_per_um2"].as_f64().unwrap() - 2.0).abs() < 1e-12);
    assert!((r["pair_log_sum"].as_f64().unwrap()).abs() < 1e-12);
    assert_eq!(r["contributions"].as_array().unwrap().len(), 2);
    assert_eq!(r["matrix_policy"], "complete_exact_symmetric");
}
