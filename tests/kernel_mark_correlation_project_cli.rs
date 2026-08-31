#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

#[path = "support/runtime_features.rs"]
mod runtime_features;

fn fixture(directory: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let input = directory.join("embeddings.csv");
    let bins = directory.join("bins.csv");
    let mut csv =
        String::from("object_id,biological_unit,split,x_um,y_um,embedding_0,embedding_1\n");
    for (unit, split, second) in [
        ("training-unit", "train", [0.0, 0.0, 0.0, 0.0]),
        ("held-out-unit", "test", [0.0, 1000.0, -1000.0, 500.0]),
    ] {
        for index in 0..4 {
            writeln!(
                csv,
                "{unit}-{index},{unit},{split},{},0,{index},{}",
                [0, 1, 3, 6][index],
                second[index]
            )
            .expect("fixture row");
        }
    }
    fs::write(&input, csv).expect("embedding fixture");
    fs::write(
        &bins,
        "bin_id,lower_um,upper_um\nnear,0,2\nmid,2,4\nfar,4,7\n",
    )
    .expect("bin fixture");
    (input, bins)
}

#[allow(clippy::too_many_arguments)]
fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    bins: &Path,
    output: &Path,
    maximum_points: &str,
    maximum_dimension: &str,
    maximum_pair_visits: &str,
    memory_budget_mib: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "kernel-mark-correlation",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--bins",
        bins.to_str().expect("bins path"),
        "--kernel",
        "rbf",
        "--global-reference-tolerance",
        "0.000000000001",
        "--maximum-points",
        maximum_points,
        "--maximum-dimension",
        maximum_dimension,
        "--maximum-pair-visits",
        maximum_pair_visits,
        "--memory-budget-mib",
        memory_budget_mib,
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn kernel_mark_correlation_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let (input, bins) = fixture(directory.path());

    for (name, points, dimension, pairs, memory, message) in [
        (
            "point-limit",
            "7",
            "2",
            "18",
            "8",
            "point count exceeds maximum_points 7",
        ),
        (
            "dimension-limit",
            "8",
            "1",
            "18",
            "8",
            "embedding dimension exceeds maximum_dimension 1",
        ),
        (
            "pair-limit",
            "8",
            "2",
            "17",
            "8",
            "18 kernel pair visits exceed maximum_pair_visits 17",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary, &project, &input, &bins, &output, points, dimension, pairs, memory,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(!project.exists(), "{name} must precede project creation");
        assert!(!output.exists(), "{name} must precede publication");
    }

    let large_input = directory.path().join("large-identities.csv");
    let mut large_csv =
        String::from("object_id,biological_unit,split,x_um,y_um,embedding_0,embedding_1\n");
    for index in 0..8 {
        let split = if index < 4 { "train" } else { "test" };
        writeln!(
            large_csv,
            "{}-{index},unit-{split},{split},{index},0,{index},0",
            "x".repeat(140_000)
        )
        .expect("large identity row");
    }
    assert!(large_csv.len() > 1024 * 1024);
    fs::write(&large_input, large_csv).expect("large source");
    let memory_project = directory.path().join("memory-limit-project");
    let memory_output = directory.path().join("memory-limit.json");
    project_command(
        &binary,
        &memory_project,
        &large_input,
        &bins,
        &memory_output,
        "8",
        "2",
        "18",
        "1",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "kernel source must be a regular file within 1048576 bytes",
    ));
    assert!(
        !memory_project.exists(),
        "memory admission must precede project creation"
    );
    assert!(
        !memory_output.exists(),
        "memory admission must precede publication"
    );

    let radial_input = directory.path().join("radial-memory.csv");
    let mut radial_csv =
        String::from("object_id,biological_unit,split,x_um,y_um,embedding_0,embedding_1\n");
    for index in 0..302 {
        let (unit, split) = if index < 300 {
            ("training-unit", "train")
        } else {
            ("held-out-unit", "test")
        };
        writeln!(
            radial_csv,
            "radial-{index},{unit},{split},{index},0,{index},{}",
            index % 17
        )
        .expect("radial memory row");
    }
    fs::write(&radial_input, radial_csv).expect("radial memory source");
    let radial_project = directory.path().join("radial-memory-project");
    let radial_output = directory.path().join("radial-memory.json");
    project_command(
        &binary,
        &radial_project,
        &radial_input,
        &bins,
        &radial_output,
        "302",
        "2",
        "89701",
        "1",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "kernel mark-correlation retained-memory estimate",
    ));
    assert!(
        !radial_project.exists(),
        "radial scale-fit memory admission must precede project creation"
    );
    assert!(
        !radial_output.exists(),
        "radial scale-fit memory admission must precede publication"
    );

    let invalid_input = directory.path().join("invalid-split.csv");
    let invalid_csv = fs::read_to_string(&input)
        .expect("fixture source")
        .replace("held-out-unit,test", "held-out-unit,holdout");
    fs::write(&invalid_input, invalid_csv).expect("invalid split source");
    let invalid_project = directory.path().join("invalid-split-project");
    let invalid_output = directory.path().join("invalid-split.json");
    project_command(
        &binary,
        &invalid_project,
        &invalid_input,
        &bins,
        &invalid_output,
        "8",
        "2",
        "18",
        "8",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "kernel rows require unique exact identities, closed splits",
    ));
    assert!(
        !invalid_project.exists(),
        "row validation must precede project creation"
    );
    assert!(
        !invalid_output.exists(),
        "row validation must precede publication"
    );

    let invalid_bins = directory.path().join("invalid-bins.csv");
    fs::write(
        &invalid_bins,
        "bin_id,lower_um,upper_um\nnear,0,2\nmid,3,4\nfar,4,7\n",
    )
    .expect("invalid bins source");
    let invalid_bins_project = directory.path().join("invalid-bins-project");
    let invalid_bins_output = directory.path().join("invalid-bins.json");
    project_command(
        &binary,
        &invalid_bins_project,
        &input,
        &invalid_bins,
        &invalid_bins_output,
        "8",
        "2",
        "18",
        "8",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "distance bins must be unique, contiguous, and increasing",
    ));
    assert!(
        !invalid_bins_project.exists(),
        "bin validation must precede project creation"
    );
    assert!(
        !invalid_bins_output.exists(),
        "bin validation must precede publication"
    );

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "bayes",
            "kernel-mark-correlation",
            "--input",
            input.to_str().expect("input path"),
            "--bins",
            bins.to_str().expect("bins path"),
            "--kernel",
            "rbf",
            "--global-reference-tolerance",
            "0.000000000001",
            "--maximum-pair-visits",
            "18",
            "--out",
            direct.to_str().expect("direct output"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(
        &binary, &project, &input, &bins, &first, "8", "2", "18", "8",
    )
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=miss"));
    project_command(
        &binary, &project, &input, &bins, &replay, "8", "2", "18", "8",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=hit"));

    let direct_bytes = fs::read(&direct).expect("direct result");
    assert_eq!(direct_bytes, fs::read(&first).expect("project result"));
    assert_eq!(direct_bytes, fs::read(&replay).expect("replayed result"));

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    let identity = &record["identity"];
    assert_eq!(identity["node"]["id"], "kernel-mark-correlation");
    let node = NodeSpec::new(
        NodeId::new("kernel-mark-correlation").expect("node ID"),
        "kernel_mark_correlation",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.kernel_mark_correlation"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["scheduler_output_limit_bytes"], 1024 * 1024);
    assert_eq!(identity["inputs"].as_array().expect("inputs").len(), 2);
    for (index, path) in [&input, &bins].into_iter().enumerate() {
        let bytes = fs::read(path).expect("source");
        assert_eq!(
            identity["inputs"][index]["digest"],
            ContentDigest::from_bytes(&bytes).to_string()
        );
        assert_eq!(identity["inputs"][index]["byte_len"], bytes.len() as u64);
    }
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-kernel-mark-correlation-configuration-v1".as_slice(),
        b"rbf".as_slice(),
        0.000000000001_f64.to_bits().to_string().as_bytes(),
        b"8".as_slice(),
        b"2".as_slice(),
        b"18".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = b"serial;training-only-kernel-fit;split-isolated-curves;physical-distance-bins;kernel=rbf;global_reference_tolerance_bits=4427486594234968593;maximum_points=8;maximum_dimension=2;maximum_pair_visits=18;memory_budget_mib=8";
    assert_eq!(
        identity["execution_policy_digest"],
        ContentDigest::from_bytes(execution_policy).to_string()
    );
    assert_eq!(identity["runtime"]["backend"], "native");
    assert_eq!(
        identity["runtime"]["features"],
        runtime_features::expected_runtime_features()
    );
    let executable = fs::read(&binary).expect("marklab executable");
    assert_eq!(
        identity["runtime"]["executable"]["digest"],
        ContentDigest::from_bytes(&executable).to_string()
    );
    assert_eq!(
        identity["runtime"]["executable"]["byte_len"],
        executable.len() as u64
    );
}
