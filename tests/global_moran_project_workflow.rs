#![allow(dead_code)]

use marklab::{
    execute_algorithm_with_store, ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CacheStatus,
    DeclaredScalarPatternInput, DurableProject, DurableProjectLimits, GlobalMoranAlternative,
    GlobalMoranAnalysisNode, GlobalMoranDesign, GlobalMoranLimits, GlobalMoranPrePostNode,
    GlobalMoranPrePostResult, GlobalMoranWeightPolicy, LocalScheduler, MarkTable,
    MeasurementStatus, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
    SchedulerLimits, WorkflowGraph,
};
use tempfile::TempDir;

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

struct GraphRun {
    statuses: [CacheStatus; 3],
    result: GlobalMoranPrePostResult,
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".to_owned()],
        ArtifactRef::from_bytes("application/vnd.marklab.executable", b"moran-workflow-test")
            .expect("executable identity"),
    )
    .expect("runtime")
}

fn run_graph(fixture: &mut Fixture, durable: &mut DurableProject, seed: u64) -> GraphRun {
    let binary_provenance = publish_record(
        fixture,
        b"moran-workflow-binary-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::Measured,
            "independent",
        ),
    );
    let area_provenance = publish_record(
        fixture,
        b"moran-workflow-area-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    let binary = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        binary_provenance,
    )
    .expect("binary declaration");
    let area =
        marklab::NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, area_provenance)
            .expect("area declaration");

    let mut pre_pattern = fixture.pattern.clone();
    pre_pattern.meta.timepoint = "pre".into();
    pre_pattern.nucleus_area_um2 = Some(vec![1.0, 2.0, 8.0, 9.0].into_boxed_slice());
    let mut post_pattern = fixture.pattern.clone();
    post_pattern.meta.timepoint = "post".into();
    post_pattern.nucleus_area_um2 = Some(vec![1.0, 8.0, 2.0, 9.0].into_boxed_slice());

    let table = |pattern: &marklab::Pattern| {
        MarkTable::new(
            fixture.cell_ids.clone(),
            vec![
                ScalarMarkColumn::binary(
                    binary.clone(),
                    ScalarMarkModality::Immunohistochemistry,
                    ScalarMarkUnit::Unitless,
                    marklab::MissingnessPolicy::NotPermitted,
                    pattern.mark.clone(),
                )
                .expect("binary column"),
                ScalarMarkColumn::continuous(
                    area.clone(),
                    ScalarMarkModality::Morphology,
                    ScalarMarkUnit::SquareMicrometer,
                    marklab::MissingnessPolicy::NotPermitted,
                    pattern.nucleus_area_um2.clone().expect("continuous values"),
                )
                .expect("continuous column"),
            ],
            fixture.cell_ids.len(),
            cell_id_text_bytes(&fixture.cell_ids),
        )
        .expect("mark table")
    };
    let pre_table = table(&pre_pattern);
    let post_table = table(&post_pattern);
    let pre_input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pre_pattern,
        &pre_table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("pre input");
    let post_input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &post_pattern,
        &post_table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("post input");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    let mark_id = ScalarMarkId::new("nucleus_area_um2").expect("area mark ID");
    let design = GlobalMoranDesign::random_labeling(31, seed, GlobalMoranAlternative::TwoSided)
        .expect("design");
    let limits = GlobalMoranLimits::new(4, 6, 6 * 31).expect("limits");
    let pre = GlobalMoranAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("moran-pre").expect("pre ID"),
        &pre_input,
        &window,
        mark_id.clone(),
        1.1,
        GlobalMoranWeightPolicy::BinarySymmetric,
        &design,
        limits,
    )
    .expect("pre node");
    let post = GlobalMoranAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("moran-post").expect("post ID"),
        &post_input,
        &window,
        mark_id,
        1.1,
        GlobalMoranWeightPolicy::BinarySymmetric,
        &design,
        limits,
    )
    .expect("post node");
    let upstream =
        WorkflowGraph::new([pre.spec().clone(), post.spec().clone()]).expect("upstream graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 64 * 1024,
    })
    .expect("scheduler");
    let result_schema =
        ArtifactSchema::new("marklab.spatial.global-moran-result", 1).expect("result schema");
    let pre_run = execute_algorithm_with_store(
        durable,
        &mut fixture.project,
        &upstream,
        &pre,
        &scheduler,
        result_schema.clone(),
        runtime(),
        &fixture.store,
    )
    .expect("pre run");
    let post_run = execute_algorithm_with_store(
        durable,
        &mut fixture.project,
        &upstream,
        &post,
        &scheduler,
        result_schema,
        runtime(),
        &fixture.store,
    )
    .expect("post run");
    let comparison = GlobalMoranPrePostNode::new(
        NodeId::new("moran-prepost").expect("comparison ID"),
        pre.spec().id().clone(),
        &pre_run,
        post.spec().id().clone(),
        &post_run,
    )
    .expect("comparison node");
    let graph = WorkflowGraph::new([
        pre.spec().clone(),
        post.spec().clone(),
        comparison.spec().clone(),
    ])
    .expect("complete graph");
    let comparison_run = execute_algorithm_with_store(
        durable,
        &mut fixture.project,
        &graph,
        &comparison,
        &scheduler,
        ArtifactSchema::new("marklab.spatial.global-moran-prepost-result", 1)
            .expect("comparison schema"),
        runtime(),
        &fixture.store,
    )
    .expect("comparison run");

    GraphRun {
        statuses: [
            pre_run.cache_status,
            post_run.cache_status,
            comparison_run.cache_status,
        ],
        result: comparison_run.output,
    }
}

#[test]
fn durable_moran_prepost_graph_reopens_as_three_hits() {
    let root = TempDir::new().expect("durable project root");
    let project_path = root.path().join("project");
    let limits = DurableProjectLimits::new(64 * 1024, 1024 * 1024, 64, 64 * 1024, 64 * 1024)
        .expect("durable limits");

    let first = {
        let mut fixture = fixture();
        let mut durable = DurableProject::open_or_create(&project_path, limits).expect("project");
        let run = run_graph(&mut fixture, &mut durable, 20260826);
        assert_eq!(durable.execution_count(), 3);
        run
    };
    assert_eq!(first.statuses, [CacheStatus::Miss; 3]);
    assert_eq!(
        first.result.statistic_delta_post_minus_pre,
        first.result.post.statistic - first.result.pre.statistic
    );
    assert!(first.result.statistic_delta_post_minus_pre.is_finite());

    let mut replay_fixture = fixture();
    let mut durable =
        DurableProject::open_or_create(&project_path, limits).expect("reopen project");
    let replay = run_graph(&mut replay_fixture, &mut durable, 20260826);
    assert_eq!(replay.statuses, [CacheStatus::Hit; 3]);
    assert_eq!(replay.result, first.result);
    assert_eq!(
        durable.execution_count(),
        3,
        "hits must not append executions"
    );

    let mut changed_fixture = fixture();
    let changed = run_graph(&mut changed_fixture, &mut durable, 20260827);
    assert_eq!(changed.statuses, [CacheStatus::Miss; 3]);
    assert_eq!(durable.execution_count(), 6);
}
