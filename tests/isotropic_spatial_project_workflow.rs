use approx::assert_abs_diff_eq;
use marklab::{
    CacheStatus, IsotropicSpatialAnalysisNode, IsotropicSpatialConfig, IsotropicSpatialLimits,
    IsotropicSpatialResultDocument, LocalScheduler, MarklabProject, NodeId, ObservationWindow2D,
    ObservationWindowLimits, Pattern, PatternMeta, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![1.0, 3.0],
        vec![5.0, 5.0],
        vec![0, 0],
        PatternMeta {
            case_id: "isotropic-workflow-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("isotropic-workflow-slide".into()),
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

fn config(maximum_arc_segment_tests: usize) -> IsotropicSpatialConfig {
    IsotropicSpatialConfig::new(
        vec![2.0],
        19,
        83,
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
fn isotropic_node_misses_hits_round_trips_and_invalidates_arc_limits() {
    let pattern = pattern();
    let window = window();
    let base_config = config(200_000);
    let mut project = MarklabProject::new();
    let node = IsotropicSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("isotropic-spatial").expect("node ID"),
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
    assert_eq!(miss.output.curve[0].directed_pairs, 2);
    assert_abs_diff_eq!(
        miss.output.curve[0].k.expect("canonical K"),
        direct.curve[0].k.expect("direct K"),
        epsilon = f64::EPSILON * 128.0
    );
    let document = IsotropicSpatialResultDocument::new(direct).expect("document");
    let encoded = document.to_json_pretty().expect("JSON");
    assert!(encoded.contains("\"format\": \"marklab.isotropic_spatial\""));
    assert_eq!(
        IsotropicSpatialResultDocument::from_json(&encoded)
            .expect("decode")
            .to_json_pretty()
            .expect("normalized"),
        encoded
    );
    let mut unknown: serde_json::Value = serde_json::from_str(&encoded).expect("JSON value");
    unknown["unexpected"] = serde_json::json!(true);
    assert!(IsotropicSpatialResultDocument::from_json(&unknown.to_string()).is_err());
    let mut corrupted: serde_json::Value = serde_json::from_str(&encoded).expect("JSON value");
    corrupted["analysis"]["curve"][0]["inverse_visible_arc_fraction_sum"] = serde_json::json!(0.0);
    assert!(IsotropicSpatialResultDocument::from_json(&corrupted.to_string()).is_err());

    let changed_config = config(199_999);
    let changed = IsotropicSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("isotropic-spatial").expect("node ID"),
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
