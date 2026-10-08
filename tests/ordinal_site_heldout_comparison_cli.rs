#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write, fs};
#[test]
fn heldout_comparison_favors_real_threshold_heterogeneity() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("sites.csv");
    let out = d.path().join("out.json");
    let mut csv = String::from("patient_id,site_id,group,outcome\n");
    let a = ["I", "I", "I", "I", "II", "III", "IV", "IV"];
    let b = ["I", "II", "II", "II", "II", "II", "III", "IV"];
    for s in 0..8 {
        for (i, y) in a.iter().enumerate() {
            writeln!(csv, "s{s}-a-{i},s{s},MSS,{y}").unwrap()
        }
        for (i, y) in b.iter().enumerate() {
            writeln!(csv, "s{s}-b-{i},s{s},MSI,{y}").unwrap()
        }
    }
    fs::write(&input, csv).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "ordinal-site-heldout-comparison",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--ordered-levels",
            "I,II,III,IV",
            "--smoothing",
            "0.5",
            "--optimizer-tolerance",
            "0.000001",
            "--maximum-optimizer-evaluations",
            "200000",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&fs::read(out).unwrap()).unwrap();
    assert_eq!(v["format"], "marklab.ordinal_site_heldout_comparison");
    assert_eq!(v["site_count"], 8);
    assert_eq!(v["patient_count"], 128);
    assert!(
        v["nonproportional_minus_proportional_mean_log_score"]
            .as_f64()
            .unwrap()
            > 0.05,
        "{v}"
    );
    assert!(v["optimizer_evaluations"].as_u64().unwrap() <= 200000);
}
