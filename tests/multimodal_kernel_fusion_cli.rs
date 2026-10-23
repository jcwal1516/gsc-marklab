#![cfg(feature = "cli")]

#[test]
fn kernel_fusion_cli_evaluates_patient_folds_and_preserves_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("fusion.json");
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/multimodal-kernel-fusion.json");
    let invoke = || {
        let mut command = assert_cmd::Command::cargo_bin("marklab").unwrap();
        command
            .args(["multimodal", "kernel-fusion", "--input"])
            .arg(&input)
            .arg("--out")
            .arg(&output);
        command
    };
    invoke().assert().success();
    let bytes = std::fs::read(&output).unwrap();
    let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result["format"], "marklab.multimodal_kernel_fusion");
    assert_eq!(result["patient_count"], 24);
    assert_eq!(result["models"].as_array().unwrap().len(), 4);
    let learned = &result["models"][0];
    assert_eq!(learned["id"], "learned");
    assert_eq!(learned["predictions"].as_array().unwrap().len(), 24);
    assert!(learned["rmse"].as_f64().unwrap() < 0.1);
    invoke().assert().failure();
    assert_eq!(std::fs::read(output).unwrap(), bytes);
}
