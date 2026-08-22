use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use marklab::{
    AnalysisConfig, AnalysisEngine, ArtifactRef, CacheKeyMaterial, CacheStatus, ContentDigest,
    LocalScheduler, MarkedAnalysisNode, MarklabProject, NodeError, NodeId, NodeSpec, OutputWriter,
    Pattern, PatternMeta, ResultDocument, SchedulerLimits, WorkflowError, WorkflowGraph,
    WorkflowNode,
};

#[derive(Clone)]
struct FakeNode {
    spec: NodeSpec,
    inputs: Vec<ArtifactRef>,
    input_bytes: Vec<u8>,
    executions: Arc<AtomicUsize>,
    fail: bool,
    decode_fail: bool,
    unstable_codec: bool,
    output_len: usize,
}

impl FakeNode {
    fn new(
        id: &str,
        input_bytes: &[u8],
        executions: Arc<AtomicUsize>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            spec: NodeSpec::new(NodeId::new(id)?, "fake", 1, Vec::new())?,
            inputs: vec![ArtifactRef::from_bytes(
                "application/vnd.marklab.test-input",
                input_bytes,
            )?],
            input_bytes: input_bytes.to_vec(),
            executions,
            fail: false,
            decode_fail: false,
            unstable_codec: false,
            output_len: 8,
        })
    }
}

impl WorkflowNode for FakeNode {
    type Output = String;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        self.inputs[0]
            .verify_bytes(&self.input_bytes)
            .map_err(NodeError::input)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_bytes(b"fake-config-v1"),
            execution_policy: b"single-thread",
            implementation_identity: "fake-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(NodeError::execution(std::io::Error::other(
                "injected failure",
            )));
        }
        Ok("x".repeat(self.output_len))
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        let mut encoded = output.as_bytes().to_vec();
        if self.unstable_codec {
            encoded.push(b'!');
        }
        Ok(encoded.into_boxed_slice())
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        if self.decode_fail {
            return Err(NodeError::decode(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "injected decode failure",
            )));
        }
        String::from_utf8(bytes.to_vec()).map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        "text/plain;charset=utf-8"
    }
}

#[test]
fn content_digest_change_invalidates_cache() {
    let executions = Arc::new(AtomicUsize::new(0));
    let first = FakeNode::new("cache-node", b"alpha", Arc::clone(&executions)).expect("node");
    let graph = WorkflowGraph::new([first.spec().clone()]).expect("graph");
    let mut project = MarklabProject::new();
    project
        .register_reference(first.input_artifacts()[0].clone())
        .expect("first input");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1024,
    })
    .expect("scheduler");

    let first_run = scheduler
        .run_single(&mut project, &graph, &first)
        .expect("first run");
    assert_eq!(first_run.cache_status, CacheStatus::Miss);
    assert_eq!(executions.load(Ordering::SeqCst), 1);

    let cached_run = scheduler
        .run_single(&mut project, &graph, &first)
        .expect("cached run");
    assert_eq!(cached_run.cache_status, CacheStatus::Hit);
    assert_eq!(cached_run.cache_key, first_run.cache_key);
    assert_eq!(cached_run.output, first_run.output);
    assert_eq!(executions.load(Ordering::SeqCst), 1);

    let changed = FakeNode::new("cache-node", b"beta", Arc::clone(&executions)).expect("node");
    project
        .register_reference(changed.input_artifacts()[0].clone())
        .expect("changed input");
    let changed_run = scheduler
        .run_single(&mut project, &graph, &changed)
        .expect("changed run");
    assert_eq!(changed_run.cache_status, CacheStatus::Miss);
    assert_ne!(changed_run.cache_key, first_run.cache_key);
    assert_eq!(executions.load(Ordering::SeqCst), 2);
    assert_eq!(project.successful_run_count(), 2);
    assert_eq!(
        project.inline_artifact_count(),
        1,
        "identical outputs share one content-addressed artifact"
    );
}

#[test]
fn cyclic_workflow_is_rejected_before_execution() {
    let a = NodeId::new("a").expect("a");
    let b = NodeId::new("b").expect("b");
    let a_spec = NodeSpec::new(a.clone(), "fake", 1, vec![b.clone()]).expect("a spec");
    let b_spec = NodeSpec::new(b.clone(), "fake", 1, vec![a.clone()]).expect("b spec");
    assert!(matches!(
        WorkflowGraph::new([a_spec.clone(), b_spec]),
        Err(WorkflowError::Cycle { .. })
    ));

    let missing = NodeSpec::new(a.clone(), "fake", 1, vec![b]).expect("missing spec");
    assert!(matches!(
        WorkflowGraph::new([missing]),
        Err(WorkflowError::MissingDependency { .. })
    ));
    assert!(matches!(
        WorkflowGraph::new([a_spec.clone(), a_spec]),
        Err(WorkflowError::DuplicateNode { .. })
    ));
}

#[test]
fn failed_node_never_commits_success_or_cache() {
    let executions = Arc::new(AtomicUsize::new(0));
    let mut failing = FakeNode::new("failure", b"input", Arc::clone(&executions)).expect("node");
    failing.fail = true;
    let graph = WorkflowGraph::new([failing.spec().clone()]).expect("graph");
    let mut project = MarklabProject::new();

    assert!(matches!(
        LocalScheduler::new(SchedulerLimits {
            max_inline_output_bytes: 16,
        })
        .expect("scheduler")
        .run_single(&mut project, &graph, &failing),
        Err(WorkflowError::InputNotCataloged { .. })
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);

    project
        .register_reference(failing.input_artifacts()[0].clone())
        .expect("input");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16,
    })
    .expect("scheduler");
    let before = (
        project.artifact_count(),
        project.inline_artifact_count(),
        project.successful_run_count(),
    );

    assert!(matches!(
        scheduler.run_single(&mut project, &graph, &failing),
        Err(WorkflowError::NodeFailed { .. })
    ));
    assert_eq!(
        (
            project.artifact_count(),
            project.inline_artifact_count(),
            project.successful_run_count(),
        ),
        before
    );

    let mut oversized =
        FakeNode::new("oversized", b"input", Arc::clone(&executions)).expect("node");
    oversized.output_len = 17;
    let graph = WorkflowGraph::new([oversized.spec().clone()]).expect("graph");
    assert!(matches!(
        scheduler.run_single(&mut project, &graph, &oversized),
        Err(WorkflowError::InlineOutputTooLarge { .. })
    ));
    assert_eq!(
        (
            project.artifact_count(),
            project.inline_artifact_count(),
            project.successful_run_count(),
        ),
        before
    );

    let mut undecodable =
        FakeNode::new("undecodable", b"input", Arc::clone(&executions)).expect("node");
    undecodable.decode_fail = true;
    let graph = WorkflowGraph::new([undecodable.spec().clone()]).expect("graph");
    assert!(matches!(
        scheduler.run_single(&mut project, &graph, &undecodable),
        Err(WorkflowError::NodeFailed { .. })
    ));
    assert_eq!(
        (
            project.artifact_count(),
            project.inline_artifact_count(),
            project.successful_run_count(),
        ),
        before
    );

    let mut tampered =
        FakeNode::new("tampered", b"original", Arc::clone(&executions)).expect("node");
    project
        .register_reference(tampered.input_artifacts()[0].clone())
        .expect("tampered input reference");
    tampered.input_bytes = b"changed".to_vec();
    let graph = WorkflowGraph::new([tampered.spec().clone()]).expect("graph");
    let before_tampered = (
        project.artifact_count(),
        project.inline_artifact_count(),
        project.successful_run_count(),
    );
    let executions_before_tampered = executions.load(Ordering::SeqCst);
    assert!(matches!(
        scheduler.run_single(&mut project, &graph, &tampered),
        Err(WorkflowError::NodeFailed { .. })
    ));
    assert_eq!(
        executions.load(Ordering::SeqCst),
        executions_before_tampered
    );
    assert_eq!(
        (
            project.artifact_count(),
            project.inline_artifact_count(),
            project.successful_run_count(),
        ),
        before_tampered
    );

    let cached_executions = Arc::new(AtomicUsize::new(0));
    let cached_source =
        FakeNode::new("cached-decode", b"input", Arc::clone(&cached_executions)).expect("node");
    let graph = WorkflowGraph::new([cached_source.spec().clone()]).expect("graph");
    let mut cached_project = MarklabProject::new();
    cached_project
        .register_reference(cached_source.input_artifacts()[0].clone())
        .expect("cached input");
    scheduler
        .run_single(&mut cached_project, &graph, &cached_source)
        .expect("populate cache");
    let before_cached_decode = (
        cached_project.artifact_count(),
        cached_project.inline_artifact_count(),
        cached_project.successful_run_count(),
    );
    let mut cached_decode_failure = cached_source.clone();
    cached_decode_failure.decode_fail = true;
    assert!(matches!(
        scheduler.run_single(&mut cached_project, &graph, &cached_decode_failure),
        Err(WorkflowError::NodeFailed { .. })
    ));
    assert_eq!(cached_executions.load(Ordering::SeqCst), 1);
    assert_eq!(
        (
            cached_project.artifact_count(),
            cached_project.inline_artifact_count(),
            cached_project.successful_run_count(),
        ),
        before_cached_decode
    );

    let unstable_executions = Arc::new(AtomicUsize::new(0));
    let mut unstable =
        FakeNode::new("unstable-codec", b"input", Arc::clone(&unstable_executions)).expect("node");
    unstable.unstable_codec = true;
    let graph = WorkflowGraph::new([unstable.spec().clone()]).expect("graph");
    let mut unstable_project = MarklabProject::new();
    unstable_project
        .register_reference(unstable.input_artifacts()[0].clone())
        .expect("unstable input");
    let before_unstable = (
        unstable_project.artifact_count(),
        unstable_project.inline_artifact_count(),
        unstable_project.successful_run_count(),
    );
    assert!(matches!(
        scheduler.run_single(&mut unstable_project, &graph, &unstable),
        Err(WorkflowError::NonCanonicalCodec { .. })
    ));
    assert_eq!(unstable_executions.load(Ordering::SeqCst), 1);
    assert_eq!(
        (
            unstable_project.artifact_count(),
            unstable_project.inline_artifact_count(),
            unstable_project.successful_run_count(),
        ),
        before_unstable
    );
}

#[test]
fn marked_workflow_matches_direct_compatibility_path() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 4;
    config.spectrum.low_k_shells = 2;
    config.spectrum.anisotropy_low_k_shells = 2;
    config.permutation.b = 9;
    config.permutation.stratified = false;
    config.inference.family_wise_alpha = 0.25;
    config.performance.threads = marklab::ThreadSetting::Count(1);
    config.output.write_parquet_curves = false;
    config.output.write_geojson_territories = false;
    config.output.write_figures = false;
    config.output.write_run_manifest = false;

    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 0.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        vec![1, 0, 0, 1],
        PatternMeta {
            case_id: "workflow-case".into(),
            timepoint: "post".into(),
            protein: "generic".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    pattern.window.area_um2 = 4.0;
    pattern.window.analysis_effective_length_um = 2.0;
    pattern.window.d_nn_mean_um = 1.0;

    let direct = AnalysisEngine::new(config.clone())
        .expect("direct engine")
        .analyze_pattern(&pattern)
        .expect("direct result");
    let mut project = MarklabProject::new();
    let node = MarkedAnalysisNode::new(
        &mut project,
        NodeId::new("marked-analysis").expect("node ID"),
        &pattern,
        &config,
    )
    .expect("marked node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("scheduler");
    let scheduled = scheduler
        .run_single(&mut project, &graph, &node)
        .expect("scheduled result");
    assert_eq!(scheduled.cache_status, CacheStatus::Miss);

    let mut expected = direct.clone();
    let mut actual = scheduled.output.clone();
    expected.timings.clear();
    actual.timings.clear();
    assert_eq!(actual, expected);

    let fresh_scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("fresh scheduler");
    let cached = fresh_scheduler
        .run_single(&mut project, &graph, &node)
        .expect("cached result");
    assert_eq!(cached.cache_status, CacheStatus::Hit);
    assert_eq!(cached.output, scheduled.output);
    assert_eq!(cached.artifact, scheduled.artifact);

    let out = tempfile::tempdir().expect("output parent");
    let direct_dir = out.path().join("direct");
    let run_dir = out.path().join("workflow");
    let direct_manifest =
        OutputWriter::write(&ResultDocument::marked(direct), &direct_dir, &config.output)
            .expect("direct output writer");
    let workflow_manifest = OutputWriter::write(
        &ResultDocument::marked(scheduled.output),
        &run_dir,
        &config.output,
    )
    .expect("workflow output writer");
    assert_eq!(workflow_manifest.artifacts, direct_manifest.artifacts);
    assert!(matches!(
        workflow_manifest.result,
        marklab::ArtifactStatus::Written { .. }
    ));

    let workflow_result_bytes = std::fs::read(run_dir.join("result.json")).expect("result bytes");
    scheduled
        .artifact
        .verify_bytes(&workflow_result_bytes)
        .expect("scheduled artifact identity");
    for deterministic_artifact in ["qc.json", "report.md"] {
        assert_eq!(
            std::fs::read(run_dir.join(deterministic_artifact)).expect("workflow artifact"),
            std::fs::read(direct_dir.join(deterministic_artifact)).expect("direct artifact"),
            "{deterministic_artifact} must match the compatibility path byte-for-byte"
        );
    }
    let inventory = |root: &std::path::Path| {
        let mut files = std::fs::read_dir(root)
            .expect("output directory")
            .map(|entry| entry.expect("directory entry").file_name())
            .collect::<Vec<_>>();
        files.sort();
        files
    };
    assert_eq!(inventory(&run_dir), inventory(&direct_dir));

    let mut written = ResultDocument::from_json(
        std::str::from_utf8(&workflow_result_bytes).expect("UTF-8 result JSON"),
    )
    .expect("result 0.3")
    .into_marked_pattern()
    .expect("marked result");
    written.timings.clear();
    assert_eq!(written, expected);
}
