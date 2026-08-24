use marklab::{
    CacheStatus, ClassicalSpatialAnalysisNode, ClassicalSpatialConfig, ClassicalSpatialLimits,
    ClassicalSpatialResultDocument, LocalScheduler, MarklabProject, NodeId, ObservationWindow2D,
    ObservationWindowLimits, Pattern, PatternMeta, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![2.0, 4.0, 8.0],
        vec![2.0, 2.0, 8.0],
        vec![0, 0, 0],
        PatternMeta {
            case_id: "workflow-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("workflow-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn window() -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn config(seed: u64, maximum_pair_visits: usize) -> ClassicalSpatialConfig {
    ClassicalSpatialConfig::new(
        vec![0.1, 2.0],
        19,
        seed,
        0.05,
        ClassicalSpatialLimits::new(16, 16, maximum_pair_visits, 100_000, 1 << 20)
            .expect("analysis limits"),
    )
    .expect("config")
}

#[test]
fn classical_node_misses_hits_and_invalidates_on_seed_change() {
    let pattern = pattern();
    let window = window();
    let first_config = config(31, 100_000);
    let mut project = MarklabProject::new();
    let first = ClassicalSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("classical-spatial").expect("node ID"),
        &pattern,
        &window,
        &first_config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([first.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");

    let miss = scheduler
        .run_single(&mut project, &graph, &first)
        .expect("cache miss");
    assert_eq!(miss.cache_status, CacheStatus::Miss);
    let hit = scheduler
        .run_single(&mut project, &graph, &first)
        .expect("cache hit");
    assert_eq!(hit.cache_status, CacheStatus::Hit);
    assert_eq!(hit.output, miss.output);
    assert_eq!(hit.cache_key, miss.cache_key);

    let changed_config = config(32, 100_000);
    let changed = ClassicalSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("classical-spatial").expect("node ID"),
        &pattern,
        &window,
        &changed_config,
    )
    .expect("changed node");
    let changed_run = scheduler
        .run_single(&mut project, &graph, &changed)
        .expect("changed run");
    assert_eq!(changed_run.cache_status, CacheStatus::Miss);
    assert_ne!(changed_run.cache_key, miss.cache_key);
}

#[test]
fn classical_result_document_is_canonical_and_strict() {
    let pattern = pattern();
    let window = window();
    let config = config(37, 100_000);
    let mut project = MarklabProject::new();
    let node = ClassicalSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("classical-codec").expect("node ID"),
        &pattern,
        &window,
        &config,
    )
    .expect("node");
    let output = node.execute().expect("output");
    let document = ClassicalSpatialResultDocument::new(output.clone()).expect("document");
    let first = document.to_json_pretty().expect("JSON");
    let decoded = ClassicalSpatialResultDocument::from_json(&first).expect("decode");
    assert_eq!(decoded.analysis(), &output);
    assert_eq!(decoded.to_json_pretty().expect("normalized JSON"), first);
    assert_eq!(
        node.decode_output(&node.encode_output(&output).expect("encoded"))
            .expect("node decode"),
        output
    );

    let mut unknown: serde_json::Value = serde_json::from_str(&first).expect("value");
    unknown["unexpected"] = serde_json::json!(true);
    assert!(ClassicalSpatialResultDocument::from_json(&unknown.to_string()).is_err());

    let mut inconsistent: serde_json::Value = serde_json::from_str(&first).expect("value");
    inconsistent["analysis"]["curve"][1]["eligible_centers"] = serde_json::json!(999);
    assert!(ClassicalSpatialResultDocument::from_json(&inconsistent.to_string()).is_err());
}

#[test]
fn classical_node_failure_does_not_commit_a_successful_run() {
    let pattern = pattern();
    let window = window();
    let config = config(41, 1);
    let mut project = MarklabProject::new();
    let node = ClassicalSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("classical-failure").expect("node ID"),
        &pattern,
        &window,
        &config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");

    assert!(scheduler.run_single(&mut project, &graph, &node).is_err());
    assert_eq!(project.successful_run_count(), 0);
    assert_eq!(project.inline_artifact_count(), 0);
}
