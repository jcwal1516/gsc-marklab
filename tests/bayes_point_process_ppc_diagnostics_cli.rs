#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write as _, fs};
#[test]
fn posterior_predictive_count_and_translation_k_diagnostics_are_simultaneous() {
    let d = tempfile::tempdir().unwrap();
    let obs = d.path().join("obs.csv");
    let rep = d.path().join("rep.csv");
    let rad = d.path().join("radii.csv");
    let out = d.path().join("out.json");
    fs::write(&obs,"pattern_id,point_id,x_um,y_um\np1,a,1,1\np1,b,3,1\np1,c,7,7\np2,a,2,2\np2,b,4,2\np2,c,8,8\n").unwrap();
    let mut csv = String::from("replicate,pattern_id,point_id,x_um,y_um\n");
    for s in 0..20 {
        for p in 0..2 {
            writeln!(csv, "{s},p{},a,1,1", p + 1).unwrap();
            writeln!(csv, "{s},p{},b,{},1", p + 1, 2 + (s % 4)).unwrap();
            writeln!(csv, "{s},p{},c,{},{}", p + 1, 7 + (s % 2), 7).unwrap();
        }
    }
    fs::write(&rep, csv).unwrap();
    fs::write(&rad, "radius_um\n1\n3\n6\n9\n").unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "point-process-posterior-predictive-diagnostics",
            "--observed",
            obs.to_str().unwrap(),
            "--replicated",
            rep.to_str().unwrap(),
            "--radii",
            rad.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "10",
            "--ymax-um",
            "10",
            "--alpha",
            "0.1",
            "--maximum-pair-visits",
            "100000",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    let r: serde_json::Value = serde_json::from_slice(&fs::read(out).unwrap()).unwrap();
    assert_eq!(
        r["format"],
        "marklab.point_process_posterior_predictive_diagnostics"
    );
    assert_eq!(r["pattern_count"], 2);
    assert_eq!(r["replicate_count"], 20);
    assert_eq!(r["curve"].as_array().unwrap().len(), 4);
    assert!(r["curve"].as_array().unwrap().iter().all(|x| x["lower"]
        .as_f64()
        .unwrap()
        .is_finite()
        && x["upper"].as_f64().unwrap().is_finite()));
    assert_eq!(
        r["interpretation"],
        "posterior_predictive_consistency_is_not_model_truth"
    );
}
