#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(directory: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let input = directory.join("embeddings.csv");
    let bins = directory.join("bins.csv");
    fs::write(
        &input,
        "object_id,permutation_stratum,x_um,y_um,embedding_0,embedding_1\n\
a0,s1,0,0,0,0\n\
a1,s1,1,0,0.1,0.1\n\
a2,s1,3,0,3,3\n\
a3,s1,6,0,3.1,3.1\n\
b0,s2,10,0,0,0\n\
b1,s2,11,0,0.1,0.1\n\
b2,s2,13,0,3,3\n\
b3,s2,16,0,3.1,3.1\n",
    )
    .expect("embedding fixture");
    fs::write(
        &bins,
        "bin_id,lower_um,upper_um\nnear,0,2\nmid,2,5\nfar,5,17\n",
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
    permutations: &str,
    maximum_points: &str,
    maximum_dimension: &str,
    maximum_pair_visits: &str,
    memory_budget_mib: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "embedding-spatial-dependence-envelope",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--bins",
        bins.to_str().expect("bins path"),
        "--curve",
        "vector_semivariogram",
        "--permutations",
        permutations,
        "--alpha",
        "0.05",
        "--seed",
        "991",
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
fn embedding_spatial_envelope_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let (input, bins) = fixture(directory.path());

    for (name, points, dimension, pairs, message) in [
        (
            "point-limit",
            "7",
            "2",
            "252",
            "point count exceeds maximum_points 7",
        ),
        (
            "dimension-limit",
            "8",
            "1",
            "252",
            "embedding dimension exceeds maximum_dimension 1",
        ),
        (
            "pair-limit",
            "8",
            "2",
            "251",
            "252 observed/permuted pair visits exceed maximum_pair_visits 251",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary, &project, &input, &bins, &output, "20", points, dimension, pairs, "8",
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(!project.exists(), "{name} must precede project creation");
        assert!(!output.exists(), "{name} must precede publication");
    }

    let memory_bins = directory.path().join("many-bins.csv");
    let mut bin_csv = String::from("bin_id,lower_um,upper_um\n");
    for index in 0..256 {
        writeln!(bin_csv, "bin-{index},{index},{}", index + 1).expect("memory bin");
    }
    fs::write(&memory_bins, bin_csv).expect("memory bins");
    let memory_project = directory.path().join("memory-project");
    let memory_output = directory.path().join("memory.json");
    project_command(
        &binary,
        &memory_project,
        &input,
        &memory_bins,
        &memory_output,
        "10000",
        "8",
        "2",
        "120012",
        "1",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "embedding envelope retained-memory estimate",
    ));
    assert!(
        !memory_project.exists(),
        "permutation/ERL memory admission must precede project creation"
    );
    assert!(
        !memory_output.exists(),
        "permutation/ERL memory admission must precede publication"
    );

    let invalid_input = directory.path().join("invalid-stratum.csv");
    fs::write(
        &invalid_input,
        fs::read_to_string(&input)
            .expect("fixture source")
            .replace("b0,s2", "b0, s2"),
    )
    .expect("invalid source");
    let invalid_project = directory.path().join("invalid-project");
    let invalid_output = directory.path().join("invalid.json");
    project_command(
        &binary,
        &invalid_project,
        &invalid_input,
        &bins,
        &invalid_output,
        "20",
        "8",
        "2",
        "252",
        "8",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "envelope rows require unique exact identities/strata",
    ));
    assert!(!invalid_project.exists());
    assert!(!invalid_output.exists());

    let invalid_bins = directory.path().join("invalid-bins.csv");
    fs::write(
        &invalid_bins,
        "bin_id,lower_um,upper_um\nnear,0,2\nmid,3,5\nfar,5,17\n",
    )
    .expect("invalid bins");
    let invalid_bins_project = directory.path().join("invalid-bins-project");
    let invalid_bins_output = directory.path().join("invalid-bins.json");
    project_command(
        &binary,
        &invalid_bins_project,
        &input,
        &invalid_bins,
        &invalid_bins_output,
        "20",
        "8",
        "2",
        "252",
        "8",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "distance bins must be unique, contiguous, and increasing",
    ));
    assert!(!invalid_bins_project.exists());
    assert!(!invalid_bins_output.exists());

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "bayes",
            "embedding-spatial-dependence-envelope",
            "--input",
            input.to_str().expect("input path"),
            "--bins",
            bins.to_str().expect("bins path"),
            "--curve",
            "vector_semivariogram",
            "--permutations",
            "20",
            "--alpha",
            "0.05",
            "--seed",
            "991",
            "--maximum-pair-visits",
            "252",
            "--out",
            direct.to_str().expect("direct result"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(
        &binary, &project, &input, &bins, &first, "20", "8", "2", "252", "8",
    )
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=miss"));
    project_command(
        &binary, &project, &input, &bins, &replay, "20", "8", "2", "252", "8",
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
    assert_eq!(
        identity["node"]["id"],
        "embedding-spatial-dependence-envelope"
    );
    let node = NodeSpec::new(
        NodeId::new("embedding-spatial-dependence-envelope").expect("node ID"),
        "embedding_spatial_dependence_envelope",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.embedding_spatial_dependence_envelope"
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
        b"marklab-embedding-spatial-envelope-configuration-v1".as_slice(),
        b"vector_semivariogram".as_slice(),
        0.05_f64.to_bits().to_string().as_bytes(),
        b"20".as_slice(),
        b"991".as_slice(),
        b"8".as_slice(),
        b"2".as_slice(),
        b"252".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = b"serial;complete-vector-within-stratum-random-labeling;physical-distance-bins;canonical-erl;curve=vector_semivariogram;permutations=20;alpha_bits=4587366580439587226;seed=991;maximum_points=8;maximum_dimension=2;maximum_pair_visits=252;memory_budget_mib=8";
    assert_eq!(
        identity["execution_policy_digest"],
        ContentDigest::from_bytes(execution_policy).to_string()
    );
    assert_eq!(identity["runtime"]["backend"], "native");
    assert_eq!(
        identity["runtime"]["features"],
        serde_json::json!(["cli", "csv", "parallel", "parquet"])
    );
    let executable = fs::read(&binary).expect("stable executable");
    assert_eq!(
        identity["runtime"]["executable"]["digest"],
        ContentDigest::from_bytes(&executable).to_string()
    );
    assert_eq!(
        identity["runtime"]["executable"]["byte_len"],
        executable.len() as u64
    );
}
