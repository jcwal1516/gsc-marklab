#![cfg(feature = "cli")]

use assert_cmd::Command;
use serde_json::json;
use std::{fs, path::Path};

const LANES: [&str; 8] = [
    "coordinate_only",
    "scalar_mark",
    "vector_embedding",
    "annotation_combination",
    "patch_multiscale",
    "patient_level",
    "pymc_bayesian",
    "raw_patch_embedding",
];

fn write_index(root: &Path) {
    let results = root.join("results");
    fs::create_dir_all(&results).expect("results directory");
    for name in ["admission.json", "diagnostics.json", "provenance.json"] {
        fs::write(root.join(name), b"{}\n").expect("bundle metadata");
    }
    let mut lanes = Vec::new();
    for lane in LANES {
        if lane == "raw_patch_embedding" {
            lanes.push(json!({
                "lane_id": lane,
                "status": "unavailable",
                "result": null,
                "blocker": "source artifacts contain cell embeddings and patch links but no independent patch-vector tensor"
            }));
        } else {
            let relative = format!("results/{lane}.json");
            fs::write(root.join(&relative), format!("{{\"lane\":\"{lane}\"}}\n")).expect("result");
            lanes.push(json!({
                "lane_id": lane,
                "status": "available",
                "result": relative,
                "blocker": null
            }));
        }
    }
    fs::write(
        root.join("bundle_index.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_name": "marklab_cellvit_results_index",
            "schema_version": "1.0",
            "objective": "RESULTS-CELLVIT-2DAY-01",
            "lanes": lanes
        }))
        .expect("index JSON"),
    )
    .expect("index");
}

fn adapter() -> Command {
    let mut command = Command::new("python3");
    command.arg(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("workers/python/marklab_cellvit_results_bundle.py"),
    );
    command
}

#[test]
fn cellvit_result_bundle_seals_required_lanes_and_rejects_tampering() {
    let directory = tempfile::tempdir().expect("bundle root");
    write_index(directory.path());
    fs::write(
        directory.path().join("results/bundle_manifest.json"),
        b"{}\n",
    )
    .expect("nested result named like the root manifest");

    adapter()
        .args(["seal", "--bundle"])
        .arg(directory.path())
        .assert()
        .success();
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(directory.path().join("bundle_manifest.json")).expect("manifest"),
    )
    .expect("manifest JSON");
    assert!(manifest["file_sha256"]
        .as_object()
        .expect("file digests")
        .contains_key("results/bundle_manifest.json"));
    adapter()
        .args(["verify", "--bundle"])
        .arg(directory.path())
        .assert()
        .success();

    fs::write(
        directory.path().join("results/vector_embedding.json"),
        b"tampered\n",
    )
    .expect("tamper result");
    adapter()
        .args(["verify", "--bundle"])
        .arg(directory.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("digest mismatch"));
}
