use approx::assert_abs_diff_eq;
use marklab::{
    CacheStatus, IsotropicPairCorrelationAnalysisNode, IsotropicPairCorrelationConfig,
    IsotropicPairCorrelationResultDocument, IsotropicSpatialLimits, LocalScheduler, MarklabProject,
    NodeId, ObservationWindow2D, ObservationWindowLimits, Pattern, PatternMeta, SchedulerLimits,
    WorkflowGraph, WorkflowNode,
};

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![1.0, 3.0],
        vec![5.0, 5.0],
        vec![0, 0],
        PatternMeta {
            case_id: "isotropic-g-workflow".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("slide".into()),
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
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("limits"),
    )
    .expect("window")
}

fn config(maximum_arc_segment_tests: usize) -> IsotropicPairCorrelationConfig {
    IsotropicPairCorrelationConfig::new(
        vec![2.0],
        0.5,
        19,
        71,
        0.05,
        IsotropicSpatialLimits::new(
            16,
            8,
            1_000,
            2_000,
            maximum_arc_segment_tests,
            100_000,
            100_000,
            1 << 20,
        )
        .expect("limits"),
    )
    .expect("config")
}

#[test]
fn isotropic_g_node_misses_hits_round_trips_and_invalidates_arc_limits() {
    let pattern = pattern();
    let window = window();
    let base = config(200_000);
    let mut project = MarklabProject::new();
    let node = IsotropicPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("isotropic-pair-correlation").expect("ID"),
        &pattern,
        &window,
        &base,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let miss = scheduler
        .run_single(&mut project, &graph, &node)
        .expect("miss");
    assert_eq!(miss.cache_status, CacheStatus::Miss);
    let hit = scheduler
        .run_single(&mut project, &graph, &node)
        .expect("hit");
    assert_eq!(hit.cache_status, CacheStatus::Hit);
    assert_eq!(hit.output, miss.output);
    assert_eq!(project.successful_run_count(), 1);

    let direct = node.execute().expect("direct");
    assert_abs_diff_eq!(
        miss.output.curve[0].g.unwrap(),
        direct.curve[0].g.unwrap(),
        epsilon = f64::EPSILON * 128.0
    );
    let encoded = IsotropicPairCorrelationResultDocument::new(direct)
        .and_then(|document| document.to_json_pretty())
        .expect("JSON");
    assert!(encoded.contains("\"format\": \"marklab.isotropic_pair_correlation\""));
    assert_eq!(
        IsotropicPairCorrelationResultDocument::from_json(&encoded)
            .expect("decode")
            .to_json_pretty()
            .expect("stable"),
        encoded
    );
    let mut corrupt: serde_json::Value = serde_json::from_str(&encoded).expect("value");
    corrupt["analysis"]["curve"][0]["inverse_visible_arc_weighted_kernel_sum"] =
        serde_json::json!(0.0);
    assert!(IsotropicPairCorrelationResultDocument::from_json(&corrupt.to_string()).is_err());

    let changed_config = config(199_999);
    let changed = IsotropicPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("isotropic-pair-correlation").expect("ID"),
        &pattern,
        &window,
        &changed_config,
    )
    .expect("changed");
    let changed_run = scheduler
        .run_single(&mut project, &graph, &changed)
        .expect("changed run");
    assert_eq!(changed_run.cache_status, CacheStatus::Miss);
    assert_ne!(changed_run.cache_key, miss.cache_key);
}
