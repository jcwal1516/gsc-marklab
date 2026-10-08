use approx::assert_abs_diff_eq;
use marklab::{
    CacheStatus, LocalScheduler, MarklabProject, NodeId, ObservationWindow2D,
    ObservationWindowLimits, Pattern, PatternMeta, SchedulerLimits,
    TranslationPairCorrelationAnalysisNode, TranslationPairCorrelationConfig,
    TranslationPairCorrelationResultDocument, TranslationSpatialLimits, WorkflowGraph,
    WorkflowNode,
};

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![0.5, 1.5],
        vec![0.5, 0.5],
        vec![0, 0],
        PatternMeta {
            case_id: "translation-g-workflow-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("translation-g-workflow-slide".into()),
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
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,0],[4,1],[1,1],[1,4],[0,4],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn config(seed: u64, maximum_overlap_evaluations: usize) -> TranslationPairCorrelationConfig {
    TranslationPairCorrelationConfig::new(
        vec![1.0],
        0.5,
        19,
        seed,
        0.05,
        TranslationSpatialLimits::new(
            16,
            8,
            1_000,
            maximum_overlap_evaluations,
            100_000,
            1_000,
            100_000,
            1 << 20,
        )
        .expect("limits"),
    )
    .expect("config")
}

#[test]
fn translation_g_node_misses_hits_round_trips_and_invalidates_overlap_limits() {
    let pattern = pattern();
    let window = window();
    let base_config = config(71, 1_000);
    let mut project = MarklabProject::new();
    let node = TranslationPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("translation-pair-correlation").expect("node ID"),
        &pattern,
        &window,
        &base_config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");

    let miss = scheduler
        .run_single(&mut project, &graph, &node)
        .expect("cache miss");
    assert_eq!(miss.cache_status, CacheStatus::Miss);
    let hit = scheduler
        .run_single(&mut project, &graph, &node)
        .expect("cache hit");
    assert_eq!(hit.cache_status, CacheStatus::Hit);
    assert_eq!(hit.output, miss.output);
    assert_eq!(project.successful_run_count(), 1);

    let direct = node.execute().expect("direct result");
    assert_eq!(miss.output.case_id, direct.case_id);
    assert_eq!(miss.output.correction, direct.correction);
    assert_abs_diff_eq!(
        miss.output.curve[0].g.expect("canonical g"),
        direct.curve[0].g.expect("direct g"),
        epsilon = f64::EPSILON * 128.0
    );
    let document = TranslationPairCorrelationResultDocument::new(direct).expect("document");
    let encoded = document.to_json_pretty().expect("JSON");
    assert!(encoded.contains("\"format\": \"marklab.translation_pair_correlation\""));
    assert_eq!(
        TranslationPairCorrelationResultDocument::from_json(&encoded)
            .expect("decode")
            .to_json_pretty()
            .expect("normalized"),
        encoded
    );
    let mut unknown: serde_json::Value = serde_json::from_str(&encoded).expect("JSON value");
    unknown["unexpected"] = serde_json::json!(true);
    assert!(TranslationPairCorrelationResultDocument::from_json(&unknown.to_string()).is_err());
    let mut corrupted: serde_json::Value = serde_json::from_str(&encoded).expect("JSON value");
    corrupted["analysis"]["curve"][0]["translation_weighted_kernel_sum"] = serde_json::json!(0.0);
    assert!(TranslationPairCorrelationResultDocument::from_json(&corrupted.to_string()).is_err());
    let mut corrupt_observed_work: serde_json::Value =
        serde_json::from_str(&encoded).expect("JSON value");
    corrupt_observed_work["analysis"]["geometry"]["observed_overlap_evaluations"] =
        serde_json::json!(0);
    assert!(TranslationPairCorrelationResultDocument::from_json(
        &corrupt_observed_work.to_string()
    )
    .is_err());

    let changed_config = config(71, 999);
    let changed = TranslationPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("translation-pair-correlation").expect("node ID"),
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
