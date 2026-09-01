#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

#[path = "support/joint_location_embedding_fixture.rs"]
mod fixture;

fn arguments(
    location: &Path,
    embedding: &Path,
    output: &Path,
    degrees_of_freedom: &str,
) -> Vec<String> {
    let mut arguments = fixture::arguments(location, embedding, output);
    let output_arguments = arguments.split_off(arguments.len() - 2);
    arguments.extend([
        "--embedding-residual-family".into(),
        "student-t".into(),
        "--student-t-degrees-of-freedom".into(),
        degrees_of_freedom.into(),
    ]);
    arguments.extend(output_arguments);
    arguments
}

fn project_command(
    project: &Path,
    location: &Path,
    embedding: &Path,
    output: &Path,
    degrees_of_freedom: &str,
) -> Command {
    let arguments = arguments(location, embedding, output, degrees_of_freedom);
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command
        .arg("project")
        .arg("joint-replicated-location-embedding")
        .arg("--project")
        .arg(project)
        .args(arguments.into_iter().skip(2));
    command
}

#[test]
fn robust_joint_location_embedding_uses_student_t_and_replays() {
    let directory = tempfile::tempdir().expect("tempdir");
    let location = directory.path().join("location.csv");
    let embedding = directory.path().join("embedding.csv");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fixture::write_inputs(&location, &embedding);

    Command::cargo_bin("marklab")
        .expect("binary")
        .args(arguments(&location, &embedding, &direct, "5"))
        .assert()
        .success();
    project_command(&project, &location, &embedding, &first, "5")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &location, &embedding, &second, "5")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let direct_result: serde_json::Value =
        serde_json::from_slice(&fs::read(direct).unwrap()).unwrap();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        direct_result["embedding_likelihood"],
        result["embedding_likelihood"]
    );
    assert_eq!(
        result["embedding_likelihood"],
        "independent_student_t_on_fold_frozen_projected_embeddings_df_5"
    );
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn robust_joint_location_embedding_rejects_invalid_residual_controls_before_backend() {
    let directory = tempfile::tempdir().expect("tempdir");
    let location = directory.path().join("location.csv");
    let embedding = directory.path().join("embedding.csv");
    let output = directory.path().join("output.json");
    fixture::write_inputs(&location, &embedding);

    for degrees in ["2", "nan", "101"] {
        project_command(
            &directory.path().join(format!("project-{degrees}")),
            &location,
            &embedding,
            &output,
            degrees,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "joint location-embedding controls or resources differ",
        ));
    }
}
