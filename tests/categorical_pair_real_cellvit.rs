#[path = "support/real_cellvit_categorical.rs"]
mod real_cellvit_categorical;

use std::{fs, path::Path, process::Command};

use marklab::{
    execute_algorithm_with_store, ArtifactRef, ArtifactSchema, CacheStatus,
    CategoricalCrossPairCorrelationAnalysisNode, CategoricalCrossPairCorrelationConfig,
    CategoricalPairAnalysisNode, CategoricalPairConfig, CategoricalPairLimits, ContentDigest,
    DeclaredScalarPatternInput, DurableProject, DurableProjectLimits, LocalScheduler,
    NativeRuntimeProvenance, NodeId, SchedulerLimits, WorkflowGraph,
};
use real_cellvit_categorical::real_fixture;

const CHILD_PROJECT: &str = "MARKLAB_REAL_CELLVIT_PAIR_CHILD_PROJECT";
const CHILD_RESULT: &str = "MARKLAB_REAL_CELLVIT_PAIR_CHILD_RESULT";
const REAL_CELLS: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV";
const REAL_WINDOW: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW";

#[test]
#[ignore = "requires the admitted real CellViT coordinate CSV and exact window"]
fn admitted_cellvit_pair_curves_replay_across_fresh_processes() {
    let cells = std::env::var_os(REAL_CELLS).expect(REAL_CELLS);
    let window = std::env::var_os(REAL_WINDOW).expect(REAL_WINDOW);
    let root = tempfile::tempdir().expect("temporary pair projects");
    let executable = std::env::current_exe().expect("current integration-test executable");
    let first_path = root.path().join("first.json");
    let second_path = root.path().join("second.json");
    for output in [&first_path, &second_path] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("admitted_cellvit_pair_curves_child")
            .env(CHILD_PROJECT, root.path().join("durable"))
            .env(CHILD_RESULT, output)
            .env(REAL_CELLS, &cells)
            .env(REAL_WINDOW, &window)
            .status()
            .expect("run fresh pair child");
        assert!(status.success());
    }

    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(first_path).expect("first receipt")).expect("first JSON");
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(second_path).expect("second receipt"))
            .expect("second JSON");
    assert_eq!(first["pair_cache"], "miss");
    assert_eq!(first["cross_g_cache"], "miss");
    assert_eq!(second["pair_cache"], "hit");
    assert_eq!(second["cross_g_cache"], "hit");
    for receipt in [&first, &second] {
        assert_eq!(receipt["pair_executions"], 1);
        assert_eq!(receipt["cross_g_executions"], 1);
        assert_eq!(receipt["source_count"], 1_450);
        assert_eq!(receipt["target_count"], 365);
        assert_eq!(receipt["radii"], 4);
        assert_eq!(receipt["permutations"], 19);
        assert!(receipt["pair_count"].as_u64().expect("pair count") > 0);
        assert!(
            receipt["cross_g_pair_count"]
                .as_u64()
                .expect("cross-g pair count")
                > 0
        );
    }
    assert_eq!(first["pair_result_digest"], second["pair_result_digest"]);
    assert_eq!(
        first["cross_g_result_digest"],
        second["cross_g_result_digest"]
    );
}

#[test]
fn admitted_cellvit_pair_curves_child() {
    let Some(project) = std::env::var_os(CHILD_PROJECT) else {
        return;
    };
    let output = std::env::var_os(CHILD_RESULT).expect(CHILD_RESULT);
    let cells = std::env::var_os(REAL_CELLS).expect(REAL_CELLS);
    let window = std::env::var_os(REAL_WINDOW).expect(REAL_WINDOW);
    run_pair_curves(
        Path::new(&project),
        Path::new(&cells),
        Path::new(&window),
        Path::new(&output),
    );
}

fn run_pair_curves(project_root: &Path, cells_path: &Path, window_path: &Path, output_path: &Path) {
    let mut fixture = real_fixture(cells_path, window_path);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &fixture.table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("real declared input");
    let limits = CategoricalPairLimits::new(2_000, 8, 4_000_000, 80_000_000, 128 << 20)
        .expect("pair limits");
    let radii = vec![25.0, 50.0, 100.0, 200.0];
    let pair_config = CategoricalPairConfig::new(
        radii.clone(),
        "Neoplastic",
        "Inflammatory",
        19,
        20260828,
        0.05,
        limits,
    )
    .expect("pair config");
    let pair_node = CategoricalPairAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("real-cellvit-categorical-pair").expect("node ID"),
        &input,
        &fixture.window,
        &pair_config,
    )
    .expect("pair node");
    let pair_graph = WorkflowGraph::new([pair_node.spec().clone()]).expect("pair graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let durable_limits = DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
        .expect("durable limits");
    let pair_project_path = project_root.join("categorical-pair");
    let mut pair_project =
        DurableProject::open_or_create(&pair_project_path, durable_limits).expect("pair project");
    let pair_run = execute_algorithm_with_store(
        &mut pair_project,
        &mut fixture.project,
        &pair_graph,
        &pair_node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_pair", 1).expect("schema"),
        runtime(b"real-cellvit-categorical-pair-v1"),
        &fixture.store,
    )
    .expect("pair run");
    let pair_cache = cache_name(pair_run.cache_status);
    let pair_executions = pair_project.execution_count();
    let pair_result_digest =
        ContentDigest::from_bytes(format!("{:?}", pair_run.output).as_bytes()).to_string();
    let source_count = pair_run.output.source_count;
    let target_count = pair_run.output.target_count;
    let pair_count = pair_run.output.geometry.directed_pair_count;
    let permutations = pair_run.output.inference.permutations_completed;
    let radius_count = pair_run.output.curve.len();
    drop(pair_node);

    let cross_g_config = CategoricalCrossPairCorrelationConfig::new(
        radii,
        10.0,
        "Neoplastic",
        "Inflammatory",
        19,
        20260828,
        0.05,
        limits,
    )
    .expect("cross-g config");
    let cross_g_node = CategoricalCrossPairCorrelationAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("real-cellvit-categorical-cross-g").expect("node ID"),
        &input,
        &fixture.window,
        &cross_g_config,
    )
    .expect("cross-g node");
    let cross_g_graph = WorkflowGraph::new([cross_g_node.spec().clone()]).expect("cross-g graph");
    let cross_g_project_path = project_root.join("categorical-cross-g");
    let mut cross_g_project = DurableProject::open_or_create(&cross_g_project_path, durable_limits)
        .expect("cross-g project");
    let cross_g_run = execute_algorithm_with_store(
        &mut cross_g_project,
        &mut fixture.project,
        &cross_g_graph,
        &cross_g_node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_cross_pair_correlation", 1).expect("schema"),
        runtime(b"real-cellvit-categorical-cross-g-v1"),
        &fixture.store,
    )
    .expect("cross-g run");
    let cross_g_result_bytes = serde_json::to_vec(&cross_g_run.output).expect("cross-g JSON");
    let receipt = serde_json::json!({
        "pair_cache": pair_cache,
        "pair_executions": pair_executions,
        "pair_result_digest": pair_result_digest,
        "cross_g_cache": cache_name(cross_g_run.cache_status),
        "cross_g_executions": cross_g_project.execution_count(),
        "cross_g_result_digest": ContentDigest::from_bytes(&cross_g_result_bytes).to_string(),
        "source_count": source_count,
        "target_count": target_count,
        "pair_count": pair_count,
        "cross_g_pair_count": cross_g_run.output.directed_pair_count,
        "radii": radius_count,
        "permutations": permutations,
    });
    fs::write(
        output_path,
        serde_json::to_vec(&receipt).expect("receipt JSON"),
    )
    .expect("write receipt");
}

fn cache_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Miss => "miss",
        CacheStatus::Hit => "hit",
    }
}

fn runtime(executable_bytes: &[u8]) -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.1.0-real-cellvit",
        None,
        None,
        "rustc 1.96.0",
        vec!["real-cellvit-categorical-pair".into()],
        ArtifactRef::from_bytes("application/vnd.marklab.executable", executable_bytes)
            .expect("executable"),
    )
    .expect("runtime")
}
