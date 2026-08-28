#[path = "support/real_cellvit_categorical.rs"]
mod real_cellvit_categorical;

use std::{fs, path::Path, process::Command};

use marklab::{
    execute_algorithm_with_store, ArtifactRef, ArtifactSchema, CacheStatus,
    CategoricalNeighborhoodMixingAnalysisNode, CategoricalNeighborhoodMixingConfig,
    CategoricalNeighborhoodMixingLimits, DeclaredScalarPatternInput, DurableProject,
    DurableProjectLimits, LocalScheduler, NativeRuntimeProvenance, NodeId, ScalarMarkId,
    SchedulerLimits, WorkflowGraph,
};
use real_cellvit_categorical::real_fixture;

const CHILD_PROJECT: &str = "MARKLAB_REAL_CELLVIT_MIXING_CHILD_PROJECT";
const CHILD_RESULT: &str = "MARKLAB_REAL_CELLVIT_MIXING_CHILD_RESULT";
const REAL_CELLS: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV";
const REAL_WINDOW: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW";

#[test]
#[ignore = "requires the admitted real CellViT coordinate CSV and exact window"]
fn admitted_cellvit_categorical_mixing_replays_across_fresh_processes() {
    let cells = std::env::var_os(REAL_CELLS).expect(REAL_CELLS);
    let window = std::env::var_os(REAL_WINDOW).expect(REAL_WINDOW);
    let root = tempfile::tempdir().expect("temporary real durable project");
    let project = root.path().join("project");
    let executable = std::env::current_exe().expect("current integration-test executable");
    let first_path = root.path().join("first.txt");
    let second_path = root.path().join("second.txt");

    for output in [&first_path, &second_path] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("admitted_cellvit_categorical_mixing_child")
            .env(CHILD_PROJECT, &project)
            .env(CHILD_RESULT, output)
            .env(REAL_CELLS, &cells)
            .env(REAL_WINDOW, &window)
            .status()
            .expect("run fresh durable child process");
        assert!(status.success());
    }

    let first = fs::read_to_string(first_path).expect("first receipt");
    let second = fs::read_to_string(second_path).expect("second receipt");
    let first_lines = first.lines().collect::<Vec<_>>();
    let second_lines = second.lines().collect::<Vec<_>>();
    assert_eq!(first_lines[0], "miss");
    assert_eq!(second_lines[0], "hit");
    assert_eq!(first_lines[1], "1");
    assert_eq!(second_lines[1], "1");
    assert_eq!(first_lines[2], second_lines[2]);

    let result: serde_json::Value =
        serde_json::from_str(first_lines[2]).expect("typed result JSON");
    assert_eq!(result["point_count"], 2_000);
    assert_eq!(result["class_ids"].as_array().expect("classes").len(), 4);
    assert!(result["directed_pair_visits"].as_u64().expect("visits") > 0);
}

#[test]
fn admitted_cellvit_categorical_mixing_child() {
    let Some(project) = std::env::var_os(CHILD_PROJECT) else {
        return;
    };
    let output = std::env::var_os(CHILD_RESULT).expect(CHILD_RESULT);
    let cells = std::env::var_os(REAL_CELLS).expect(REAL_CELLS);
    let window = std::env::var_os(REAL_WINDOW).expect(REAL_WINDOW);
    run_real_once(
        Path::new(&project),
        Path::new(&cells),
        Path::new(&window),
        Path::new(&output),
    );
}

fn run_real_once(project_path: &Path, cells_path: &Path, window_path: &Path, output_path: &Path) {
    let mut fixture = real_fixture(cells_path, window_path);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &fixture.table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("real declared input");
    let config = CategoricalNeighborhoodMixingConfig::new(
        50.0,
        CategoricalNeighborhoodMixingLimits::new(2_000, 8, 4_000_000, 128 << 20).expect("limits"),
    )
    .expect("config");
    let mark_id = ScalarMarkId::new("histologic_compartment").expect("categorical mark ID");
    let node = CategoricalNeighborhoodMixingAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("real-cellvit-categorical-mixing").expect("node ID"),
        &input,
        &fixture.window,
        &mark_id,
        &config,
    )
    .expect("analysis node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let durable_limits = DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
        .expect("durable limits");
    let mut durable =
        DurableProject::open_or_create(project_path, durable_limits).expect("durable project");
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_neighborhood_mixing", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("durable run");
    let cache = match run.cache_status {
        CacheStatus::Miss => "miss",
        CacheStatus::Hit => "hit",
    };
    let receipt = format!(
        "{cache}\n{}\n{}\n",
        durable.execution_count(),
        serde_json::to_string(&run.output).expect("typed result JSON")
    );
    fs::write(output_path, receipt).expect("child receipt");
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.1.0-real-cellvit",
        None,
        None,
        "rustc 1.96.0",
        vec!["real-cellvit-categorical".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"categorical-neighborhood-mixing-real-cellvit-v1",
        )
        .expect("executable"),
    )
    .expect("runtime")
}
