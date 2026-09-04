#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn normal_command(input: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "bayes", "normal-mean", "--input", input.to_str().unwrap(),
        "--prior-mean", "0", "--prior-sd", "1", "--known-sigma", "1",
        "--chains", "2", "--tune", "100", "--draws", "100",
        "--target-accept", "0.9", "--seed", "41", "--timeout-seconds", "60",
        "--out", out.to_str().unwrap(),
    ]);
    command
}

#[test]
fn explicit_missing_assets_never_fall_back_to_the_build_checkout() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("observations.csv");
    let out = directory.path().join("result.json");
    fs::write(&input, "observation\n1\n2\n3\n4\n").unwrap();
    normal_command(&input, &out)
        .env("MARKLAB_RUNTIME_ROOT", directory.path().join("absent"))
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .failure()
        .stderr(predicates::str::contains("MARKLAB_RUNTIME_ROOT"));
    assert!(!out.exists());
}

#[test]
fn relative_asset_override_is_rejected_before_execution() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("observations.csv");
    let out = directory.path().join("result.json");
    fs::write(&input, "observation\n1\n2\n3\n4\n").unwrap();
    normal_command(&input, &out)
        .env("MARKLAB_RUNTIME_ROOT", "relative-assets")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .failure()
        .stderr(predicates::str::contains("absolute"));
    assert!(!out.exists());
}

#[test]
fn backend_doctor_reports_the_resolved_assets_and_interpreter() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let interpreter = repository.join("target/pymc-venv/bin/python");
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "doctor"])
        .env("MARKLAB_RUNTIME_ROOT", repository)
        .env("MARKLAB_PYTHON", &interpreter)
        .assert()
        .success()
        .stdout(predicates::str::contains("Python 3.12"))
        .stdout(predicates::str::contains(interpreter.to_string_lossy().as_ref()));
}

#[test]
fn explicit_missing_interpreter_is_reported_before_a_fit() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("observations.csv");
    let out = directory.path().join("result.json");
    let interpreter = directory.path().join("missing-python");
    fs::write(&input, "observation\n1\n2\n3\n4\n").unwrap();
    normal_command(&input, &out)
        .env("MARKLAB_RUNTIME_ROOT", env!("CARGO_MANIFEST_DIR"))
        .env("MARKLAB_PYTHON", &interpreter)
        .assert()
        .failure()
        .stderr(predicates::str::contains(interpreter.to_string_lossy().as_ref()));
    assert!(!out.exists());
}

#[test]
fn relocated_bundle_executes_once_and_replays_without_an_interpreter() {
    let directory = tempfile::Builder::new().prefix("marklab installed ").tempdir().unwrap();
    let bundle = directory.path().join("bundle");
    let assets = bundle.join("workers/python");
    fs::create_dir_all(&assets).unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in ["uv.lock", "pyproject.toml", "marklab_pymc_worker.py"] {
        fs::copy(repository.join("workers/python").join(name), assets.join(name)).unwrap();
    }
    let executable = bundle.join(if cfg!(windows) { "marklab.exe" } else { "marklab" });
    fs::copy(assert_cmd::cargo::cargo_bin("marklab"), &executable).unwrap();
    let input = directory.path().join("observations.csv");
    fs::write(&input, "observation\n1\n2\n3\n4\n").unwrap();
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let interpreter = marklab::python_backend_interpreter(repository).unwrap();

    for (out, replay) in [(&first, false), (&second, true)] {
        let mut command = Command::new(&executable);
        command.current_dir(directory.path())
            .env_remove("MARKLAB_RUNTIME_ROOT")
            .env("MARKLAB_PYTHON", if replay { directory.path().join("absent-python") } else { interpreter.clone() })
            .env("MARKLAB_BACKEND_CACHE", directory.path().join("cache"))
            .args([
                "project", "normal-mean", "--project", project.to_str().unwrap(),
                "--input", input.to_str().unwrap(), "--prior-mean", "0",
                "--prior-sd", "1", "--known-sigma", "1", "--chains", "2",
                "--tune", "100", "--draws", "100", "--target-accept", "0.9",
                "--seed", "41", "--timeout-seconds", "120", "--out", out.to_str().unwrap(),
            ]);
        if replay {
            command.env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1");
        }
        command.assert().success().stderr(predicates::str::contains(
            if replay { "cache_status=hit" } else { "cache_status=miss" }
        ));
    }
    let first_bytes = fs::read(first).unwrap();
    assert_eq!(first_bytes, fs::read(second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).unwrap();
    assert_eq!(result["format"], "marklab.bayesian_fit");
    assert_eq!(result["backend"]["worker_sha256"], marklab_bayes::sha256_hex(
        &fs::read(assets.join("marklab_pymc_worker.py")).unwrap()
    ));
    assert_eq!(fs::read_to_string(project.join("executions.jsonl")).unwrap().lines().count(), 1);
}
