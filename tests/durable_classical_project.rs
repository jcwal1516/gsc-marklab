#![cfg(feature = "cli")]

use std::{fs, path::Path, process::Command as ProcessCommand};

use assert_cmd::Command;
use predicates::prelude::*;

#[allow(dead_code)]
#[path = "../build.rs"]
mod build_script;

fn run_git(repository: &Path, arguments: &[&str]) {
    let status = ProcessCommand::new("git")
        .current_dir(repository)
        .args(arguments)
        .status()
        .expect("run git");
    assert!(status.success(), "git {arguments:?} failed with {status}");
}

#[test]
fn native_build_provenance_marks_untracked_source_state_dirty() {
    let repository = tempfile::tempdir().expect("temporary Git repository");
    run_git(repository.path(), &["init", "--quiet"]);
    fs::write(repository.path().join("tracked.rs"), "fn tracked() {}\n").expect("tracked file");
    run_git(repository.path(), &["add", "tracked.rs"]);
    run_git(
        repository.path(),
        &[
            "-c",
            "user.name=Marklab Test",
            "-c",
            "user.email=marklab@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );
    assert_eq!(build_script::git_dirty(repository.path()), Some(false));

    fs::write(
        repository.path().join("untracked.rs"),
        "fn changes_the_binary() {}\n",
    )
    .expect("untracked source file");
    assert_eq!(build_script::git_dirty(repository.path()), Some(true));
}

#[test]
fn unified_execute_algorithm_symbol_owns_the_durable_typed_node_boundary() {
    let _execute =
        marklab_workflow::execute_algorithm::<marklab::ClassicalSpatialAnalysisNode<'static>>;
}

fn write_fixture(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let cells = root.join("cells.csv");
    let window = root.join("window.geojson");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0,0,0,durable_classical,baseline,unmarked,true,true\n\
1,0,1,durable_classical,baseline,unmarked,true,true\n\
2,0,0,durable_classical,baseline,unmarked,true,true\n\
3,0,1,durable_classical,baseline,unmarked,true,true\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,4],[-1,4],[-1,-1]]]]}"#,
    )
    .expect("window");
    (cells, window)
}

fn project_classical_command(project: &Path, cells: &Path, window: &Path, out: &Path) -> Command {
    project_classical_command_with_options(
        project,
        cells,
        window,
        out,
        ProjectClassicalOptions::default(),
    )
}

fn project_classical_command_with_seed(
    project: &Path,
    cells: &Path,
    window: &Path,
    out: &Path,
    seed: &str,
) -> Command {
    project_classical_command_with_options(
        project,
        cells,
        window,
        out,
        ProjectClassicalOptions {
            seed,
            ..ProjectClassicalOptions::default()
        },
    )
}

#[derive(Clone, Copy)]
struct ProjectClassicalOptions<'a> {
    r_max_um: &'a str,
    r_steps: &'a str,
    simulations: &'a str,
    seed: &'a str,
    alpha: &'a str,
    memory_budget_mib: &'a str,
    maximum_pair_visits: &'a str,
    maximum_csr_draws: &'a str,
}

impl Default for ProjectClassicalOptions<'static> {
    fn default() -> Self {
        Self {
            r_max_um: "0.5",
            r_steps: "2",
            simulations: "19",
            seed: "43",
            alpha: "0.05",
            memory_budget_mib: "4",
            maximum_pair_visits: "1000000",
            maximum_csr_draws: "100000",
        }
    }
}

fn project_classical_command_with_options(
    project: &Path,
    cells: &Path,
    window: &Path,
    out: &Path,
    options: ProjectClassicalOptions<'_>,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "classical",
        "--project",
        project.to_str().expect("project path"),
        "--cells",
        cells.to_str().expect("cells path"),
        "--mask",
        window.to_str().expect("window path"),
        "--out",
        out.to_str().expect("output path"),
        "--r-max-um",
        options.r_max_um,
        "--r-steps",
        options.r_steps,
        "--simulations",
        options.simulations,
        "--seed",
        options.seed,
        "--alpha",
        options.alpha,
        "--memory-budget-mib",
        options.memory_budget_mib,
        "--max-pair-visits",
        options.maximum_pair_visits,
        "--max-csr-draws",
        options.maximum_csr_draws,
    ]);
    command
}

fn ledger_records(project: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(project.join("executions.jsonl"))
        .expect("ledger")
        .lines()
        .map(|line| serde_json::from_str(line).expect("ledger record"))
        .collect()
}

fn durable_object_path(project: &Path) -> std::path::PathBuf {
    let records = ledger_records(project);
    let artifact_id = records[0]["output"]["artifact_id"]
        .as_str()
        .expect("artifact ID");
    project
        .join("artifacts/objects/sha256")
        .join(&artifact_id[..2])
        .join(artifact_id)
}

fn durable_object_count(project: &Path) -> usize {
    fs::read_dir(project.join("artifacts/objects/sha256"))
        .expect("object prefixes")
        .map(|prefix| prefix.expect("object prefix").path())
        .map(|prefix| fs::read_dir(prefix).expect("objects in prefix").count())
        .sum()
}

#[test]
fn separate_processes_miss_then_replay_one_verified_durable_success() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    let first_out = directory.path().join("first");
    let second_out = directory.path().join("second");

    project_classical_command(&project, &cells, &window, &first_out)
        .assert()
        .success();
    project_classical_command(&project, &cells, &window, &second_out)
        .assert()
        .success();

    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(first_out.join("result.json")).expect("first result"))
            .expect("first JSON");
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(second_out.join("result.json")).expect("second result"))
            .expect("second JSON");
    assert_eq!(first["workflow"]["cache_status"], "miss");
    assert_eq!(second["workflow"]["cache_status"], "hit");
    assert_eq!(first["analysis"], second["analysis"]);
    assert_eq!(
        first["workflow"]["cache_key"],
        second["workflow"]["cache_key"]
    );
    assert_eq!(
        first["workflow"]["output_artifact_digest"],
        second["workflow"]["output_artifact_digest"]
    );

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    assert_eq!(ledger.lines().count(), 1, "a hit must not append execution");
    assert_eq!(durable_object_count(&project), 1, "a hit must not publish");
    let head: serde_json::Value =
        serde_json::from_slice(&fs::read(project.join("project.json")).expect("head"))
            .expect("head JSON");
    assert_eq!(head["format"], "marklab.project");
    assert_eq!(head["version"], 1);
    assert_eq!(head["ledger"]["record_count"], 1);
    assert_eq!(
        head["latest_execution"]["cache_key"],
        first["workflow"]["cache_key"]
    );

    let head_bytes = fs::read(project.join("project.json")).expect("head bytes");
    assert_eq!(head_bytes.last(), Some(&b'\n'));
    assert_eq!(ledger.as_bytes().last(), Some(&b'\n'));
    serde_json::from_str::<serde_json::Value>(ledger.lines().next().expect("one record"))
        .expect("ledger JSON");
}

#[test]
fn changed_seed_is_a_new_durable_miss_while_exact_repeat_hits() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");

    for (name, seed, expected) in [
        ("first", "43", "miss"),
        ("changed", "44", "miss"),
        ("repeat", "44", "hit"),
    ] {
        let out = directory.path().join(name);
        project_classical_command_with_seed(&project, &cells, &window, &out, seed)
            .assert()
            .success();
        let result: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("result.json")).expect("result"))
                .expect("result JSON");
        assert_eq!(result["workflow"]["cache_status"], expected);
    }
    assert_eq!(ledger_records(&project).len(), 2);
}

#[test]
fn changed_scientific_and_resource_arguments_are_distinct_durable_misses() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("baseline"),
    )
    .assert()
    .success();

    let cases = [
        (
            "radii",
            ProjectClassicalOptions {
                r_max_um: "0.4",
                ..ProjectClassicalOptions::default()
            },
        ),
        (
            "simulations",
            ProjectClassicalOptions {
                simulations: "23",
                ..ProjectClassicalOptions::default()
            },
        ),
        (
            "alpha",
            ProjectClassicalOptions {
                alpha: "0.1",
                ..ProjectClassicalOptions::default()
            },
        ),
        (
            "memory-budget",
            ProjectClassicalOptions {
                memory_budget_mib: "5",
                ..ProjectClassicalOptions::default()
            },
        ),
        (
            "pair-budget",
            ProjectClassicalOptions {
                maximum_pair_visits: "999999",
                ..ProjectClassicalOptions::default()
            },
        ),
        (
            "csr-budget",
            ProjectClassicalOptions {
                maximum_csr_draws: "99999",
                ..ProjectClassicalOptions::default()
            },
        ),
    ];
    for (name, options) in cases {
        let output = directory.path().join(name);
        project_classical_command_with_options(&project, &cells, &window, &output, options)
            .assert()
            .success();
        let result: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("result.json")).expect("result"))
                .expect("result JSON");
        assert_eq!(result["workflow"]["cache_status"], "miss", "{name}");
    }

    let repeat = directory.path().join("csr-budget-repeat");
    project_classical_command_with_options(
        &project,
        &cells,
        &window,
        &repeat,
        cases.last().expect("last case").1,
    )
    .assert()
    .success();
    let repeat_result: serde_json::Value =
        serde_json::from_slice(&fs::read(repeat.join("result.json")).expect("repeat result"))
            .expect("repeat JSON");
    assert_eq!(repeat_result["workflow"]["cache_status"], "hit");
    assert_eq!(ledger_records(&project).len(), 1 + cases.len());
}

#[test]
fn changed_source_bytes_are_a_miss_even_when_the_parsed_pattern_is_unchanged() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    let first_out = directory.path().join("first");
    let changed_out = directory.path().join("changed");

    project_classical_command(&project, &cells, &window, &first_out)
        .assert()
        .success();
    let mut source = fs::read_to_string(&cells).expect("cell source");
    source.push('\n');
    fs::write(&cells, source).expect("changed cell source bytes");
    project_classical_command(&project, &cells, &window, &changed_out)
        .assert()
        .success();

    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(first_out.join("result.json")).expect("first result"))
            .expect("first JSON");
    let changed: serde_json::Value =
        serde_json::from_slice(&fs::read(changed_out.join("result.json")).expect("changed result"))
            .expect("changed JSON");
    assert_eq!(first["analysis"], changed["analysis"]);
    assert_eq!(first["workflow"]["cache_status"], "miss");
    assert_eq!(changed["workflow"]["cache_status"], "miss");
    assert_ne!(
        first["workflow"]["cache_key"],
        changed["workflow"]["cache_key"]
    );
    assert_eq!(ledger_records(&project).len(), 2);
}

#[test]
fn tampered_durable_object_fails_integrity_before_replay_or_output() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    let first_out = directory.path().join("first");
    let replay_out = directory.path().join("replay");
    project_classical_command(&project, &cells, &window, &first_out)
        .assert()
        .success();

    let object = durable_object_path(&project);
    let mut bytes = fs::read(&object).expect("object");
    bytes[0] ^= 1;
    fs::write(&object, bytes).expect("tamper object");

    project_classical_command(&project, &cells, &window, &replay_out)
        .assert()
        .failure()
        .stderr(predicate::str::contains("content verification"));
    assert!(!replay_out.exists());
    assert_eq!(ledger_records(&project).len(), 1);
}

#[test]
fn missing_truncated_and_appended_objects_fail_before_replay() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    project_classical_command(&project, &cells, &window, &directory.path().join("first"))
        .assert()
        .success();
    let object = durable_object_path(&project);
    let original = fs::read(&object).expect("object");

    let missing = object.with_extension("temporarily-missing");
    fs::rename(&object, &missing).expect("hide object");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("missing-replay"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("is missing at store key"));
    fs::rename(&missing, &object).expect("restore object");

    fs::write(&object, &original[..original.len() - 1]).expect("truncate object");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("truncated-replay"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("content verification"));

    let mut appended = original.clone();
    appended.push(b'\n');
    fs::write(&object, appended).expect("append object");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("appended-replay"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("content verification"));
    fs::write(&object, original).expect("restore object bytes");

    assert_eq!(ledger_records(&project).len(), 1);
    for output in ["missing-replay", "truncated-replay", "appended-replay"] {
        assert!(!directory.path().join(output).exists());
    }
}

#[cfg(unix)]
#[test]
fn object_control_and_store_symlinks_are_rejected_without_traversal() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    project_classical_command(&project, &cells, &window, &directory.path().join("first"))
        .assert()
        .success();

    let object = durable_object_path(&project);
    let object_target = object.with_extension("symlink-target");
    fs::rename(&object, &object_target).expect("move object");
    symlink(&object_target, &object).expect("object symlink");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("object-symlink-replay"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("symlink boundary"));
    fs::remove_file(&object).expect("remove object symlink");
    fs::rename(&object_target, &object).expect("restore object");

    let head = project.join("project.json");
    let head_target = project.join("project-head-target.json");
    fs::rename(&head, &head_target).expect("move head");
    symlink(&head_target, &head).expect("head symlink");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("head-symlink-replay"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("symlink boundary"));
    fs::remove_file(&head).expect("remove head symlink");
    fs::rename(&head_target, &head).expect("restore head");

    let store = project.join("artifacts");
    let store_target = project.join("artifact-store-target");
    fs::rename(&store, &store_target).expect("move store");
    symlink(&store_target, &store).expect("store symlink");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("store-symlink-replay"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("symlink boundary"));

    assert_eq!(ledger_records(&project).len(), 1);
    for output in [
        "object-symlink-replay",
        "head-symlink-replay",
        "store-symlink-replay",
    ] {
        assert!(!directory.path().join(output).exists());
    }
}

#[test]
fn malformed_or_noncanonical_ledger_is_not_repaired_as_a_cache_miss() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    project_classical_command(&project, &cells, &window, &directory.path().join("first"))
        .assert()
        .success();
    let ledger = project.join("executions.jsonl");
    let mut bytes = fs::read(&ledger).expect("ledger");
    bytes.push(b' ');
    fs::write(&ledger, bytes).expect("malformed ledger");

    project_classical_command(&project, &cells, &window, &directory.path().join("replay"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("execution ledger is truncated"));
    assert!(!directory.path().join("replay").exists());
}

#[test]
fn head_and_ledger_reject_unknown_fields_versions_and_noncanonical_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    project_classical_command(&project, &cells, &window, &directory.path().join("first"))
        .assert()
        .success();

    let head_path = project.join("project.json");
    let head = fs::read(&head_path).expect("head");
    let mut unknown_head = head.clone();
    let closing = unknown_head
        .windows(3)
        .rposition(|window| window == b"\n}\n")
        .expect("head closing brace");
    unknown_head.splice(
        closing..closing + 1,
        b",\n  \"unknown\": true\n".iter().copied(),
    );
    fs::write(&head_path, unknown_head).expect("unknown head field");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("unknown-head"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("unknown field"));

    let mut version_head = head.clone();
    let version = b"\"version\": 1";
    let offset = version_head
        .windows(version.len())
        .position(|window| window == version)
        .expect("head version");
    version_head[offset + version.len() - 1] = b'2';
    fs::write(&head_path, version_head).expect("unsupported head version");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("version-head"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("unsupported durable version 2"));

    let mut noncanonical_head = vec![b' '];
    noncanonical_head.extend_from_slice(&head);
    fs::write(&head_path, noncanonical_head).expect("noncanonical head");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("noncanonical-head"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("not canonically encoded"));
    fs::write(&head_path, head).expect("restore head");

    let ledger_path = project.join("executions.jsonl");
    let ledger = fs::read(&ledger_path).expect("ledger");
    let mut unknown_ledger = ledger[..ledger.len() - 2].to_vec();
    unknown_ledger.extend_from_slice(b",\"unknown\":true}\n");
    fs::write(&ledger_path, unknown_ledger).expect("unknown ledger field");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("unknown-ledger"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("unknown field"));

    let mut version_ledger = ledger.clone();
    let version = b"\"version\":1";
    let offset = version_ledger
        .windows(version.len())
        .position(|window| window == version)
        .expect("ledger version");
    version_ledger[offset + version.len() - 1] = b'2';
    fs::write(&ledger_path, version_ledger).expect("unsupported ledger version");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("version-ledger"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("unsupported durable version 2"));

    let mut noncanonical_ledger = vec![b' '];
    noncanonical_ledger.extend_from_slice(&ledger);
    fs::write(&ledger_path, noncanonical_ledger).expect("noncanonical ledger");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("noncanonical-ledger"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("not canonically encoded"));

    let first_line = std::str::from_utf8(&ledger[..ledger.len() - 1]).expect("ledger UTF-8");
    let prior_digest = marklab::ContentDigest::from_bytes(first_line.as_bytes());
    let duplicate = first_line
        .replace("\"sequence\":1", "\"sequence\":2")
        .replace(
            "\"previous_record_digest\":null",
            &format!("\"previous_record_digest\":\"{prior_digest}\""),
        );
    assert_ne!(
        duplicate, first_line,
        "duplicate record fixture must advance"
    );
    fs::write(&ledger_path, format!("{first_line}\n{duplicate}\n"))
        .expect("duplicate cache-key ledger");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("duplicate-ledger"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("repeats cache key"));

    fs::write(&ledger_path, ledger).expect("restore ledger");
    assert_eq!(ledger_records(&project).len(), 1);
}

#[test]
fn nonregular_control_lock_and_store_paths_are_rejected() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let project = directory.path().join("project");
    project_classical_command(&project, &cells, &window, &directory.path().join("first"))
        .assert()
        .success();

    let head = project.join("project.json");
    let head_target = project.join("project-head-target.json");
    fs::rename(&head, &head_target).expect("move head");
    fs::create_dir(&head).expect("directory at head path");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("directory-head"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("unsupported file type"));
    fs::remove_dir(&head).expect("remove head directory");
    fs::rename(&head_target, &head).expect("restore head");

    let lock = project.join(".marklab-project.lock");
    let lock_target = project.join("project-lock-target");
    fs::rename(&lock, &lock_target).expect("move lock");
    fs::create_dir(&lock).expect("directory at lock path");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("directory-lock"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("unsupported file type"));
    fs::remove_dir(&lock).expect("remove lock directory");
    fs::rename(&lock_target, &lock).expect("restore lock");

    let store = project.join("artifacts");
    let store_target = project.join("artifact-store-target");
    fs::rename(&store, &store_target).expect("move store");
    fs::write(&store, b"not a directory").expect("file at store path");
    project_classical_command(
        &project,
        &cells,
        &window,
        &directory.path().join("file-store"),
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("unsupported file type"));

    assert_eq!(ledger_records(&project).len(), 1);
    for output in ["directory-head", "directory-lock", "file-store"] {
        assert!(!directory.path().join(output).exists());
    }
}

#[cfg(unix)]
#[test]
fn symlink_project_root_is_rejected_without_following_it() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let actual = directory.path().join("actual-project");
    fs::create_dir(&actual).expect("actual project");
    let linked = directory.path().join("linked-project");
    symlink(&actual, &linked).expect("symlink");
    project_classical_command(&linked, &cells, &window, &directory.path().join("out"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink boundary"));
    assert!(fs::read_dir(actual)
        .expect("actual directory")
        .next()
        .is_none());
}
