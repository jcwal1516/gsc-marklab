#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write as _, fs};
#[test]
fn replicated_lgcp_preserves_patient_pattern_windows_and_field_policy() {
    let d = tempfile::tempdir().unwrap();
    let i = d.path().join("patterns.csv");
    let o = d.path().join("out.json");
    let mut csv = String::from(
        "pattern_id,patient_id,window_sha256,grid_sha256,covariate_sha256,event_count,cell_count\n",
    );
    for p in 0..3 {
        for r in 0..2 {
            let n = p * 2 + r;
            writeln!(
                csv,
                "pat{p}_rep{r},pat{p},{:064x},{:064x},{:064x},{},9",
                n + 1,
                n + 101,
                n + 201,
                20 + n
            )
            .unwrap();
        }
    }
    fs::write(&i, csv).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "build-replicated-hierarchical-lgcp",
            "--input",
            i.to_str().unwrap(),
            "--field-policy",
            "shared-plus-replicate",
            "--global-prior-sd",
            "2",
            "--patient-intercept-prior-sd",
            "1",
            "--population-field-amplitude",
            "1",
            "--population-field-length-scale-um",
            "5",
            "--replicate-field-amplitude",
            "0.5",
            "--jitter",
            "0.000001",
            "--out",
            o.to_str().unwrap(),
        ])
        .assert()
        .success();
    let r: serde_json::Value = serde_json::from_slice(&fs::read(o).unwrap()).unwrap();
    assert_eq!(r["format"], "marklab.replicated_hierarchical_lgcp_model");
    assert_eq!(r["patient_count"], 3);
    assert_eq!(r["pattern_count"], 6);
    assert_eq!(r["model"]["field_policy"], "shared_plus_replicate");
    assert_eq!(
        r["model"]["pattern_combination"],
        "never_concatenate_preserve_each_window"
    );
    assert_eq!(r["patterns"].as_array().unwrap().len(), 6);
    assert_eq!(r["fit_state"], "not_fitted");
}
