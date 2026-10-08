use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use marklab::{
    execute_algorithm, AnalysisConfig, AnalysisEngine, ArtifactId, ArtifactKey, ArtifactLocator,
    ArtifactRecord, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalArtifactStore, LocalScheduler, MarkedAnalysisNode,
    MarkedPrePostNode, MarklabProject, NativeRuntimeProvenance, NodeError, NodeId, NodeSpec,
    OutputWriter, Pattern, PatternMeta, ProjectError, ResultDocument, SchedulerLimits, StoreId,
    WorkflowError, WorkflowGraph, WorkflowNode,
};

#[derive(Clone)]
struct FakeNode {
    spec: NodeSpec,
    inputs: Vec<ArtifactRef>,
    semantic_inputs: Vec<ArtifactId>,
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
            semantic_inputs: Vec::new(),
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

    fn semantic_input_artifacts(&self) -> &[ArtifactId] {
        &self.semantic_inputs
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

fn semantic_record(bytes: &[u8], schema_version: u32, store_id: &str, key: &str) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.semantic-input", bytes).expect("content"),
        ArtifactSchema::new("marklab.test.semantic-input", schema_version).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::from([("provenance".to_owned(), "fixture-1".to_owned())]),
        vec![ArtifactLocator::new(
            StoreId::new(store_id).expect("store ID"),
            ArtifactKey::new(key).expect("artifact key"),
            None,
        )
        .expect("locator")],
    )
    .expect("record")
}

fn test_runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".to_owned()],
        ArtifactRef::from_bytes("application/vnd.marklab.executable", b"test-executable")
            .expect("test executable identity"),
    )
    .expect("test runtime")
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
    assert_eq!(
        first_run.cache_key.to_string(),
        "ddadc700530efba19202b9a4e2a6f6f5644c089aeac85f2e2a11442319cfc0b2"
    );
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
fn dependent_node_consumes_the_exact_registered_upstream_output() {
    let upstream_executions = Arc::new(AtomicUsize::new(0));
    let downstream_executions = Arc::new(AtomicUsize::new(0));
    let upstream =
        FakeNode::new("upstream", b"source", Arc::clone(&upstream_executions)).expect("upstream");
    let upstream_output = ArtifactRef::from_bytes("text/plain;charset=utf-8", b"xxxxxxxx")
        .expect("upstream output identity");
    let downstream = FakeNode {
        spec: NodeSpec::new(
            NodeId::new("downstream").expect("downstream ID"),
            "fake",
            1,
            vec![NodeId::new("upstream").expect("upstream ID")],
        )
        .expect("downstream spec"),
        inputs: vec![upstream_output.clone()],
        semantic_inputs: Vec::new(),
        input_bytes: b"xxxxxxxx".to_vec(),
        executions: Arc::clone(&downstream_executions),
        fail: false,
        decode_fail: false,
        unstable_codec: false,
        output_len: 4,
    };
    let graph = WorkflowGraph::new([upstream.spec().clone(), downstream.spec().clone()])
        .expect("dependency-valid graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1024,
    })
    .expect("scheduler");
    let mut project = MarklabProject::new();
    project
        .register_reference(upstream.input_artifacts()[0].clone())
        .expect("upstream input");

    let upstream_run = scheduler
        .run_single(&mut project, &graph, &upstream)
        .expect("upstream run");
    assert_eq!(upstream_run.artifact, upstream_output);
    let downstream_run = scheduler
        .run_single(&mut project, &graph, &downstream)
        .expect("dependent run");

    assert_eq!(upstream_run.cache_status, CacheStatus::Miss);
    assert_eq!(downstream_run.cache_status, CacheStatus::Miss);
    assert_eq!(upstream_executions.load(Ordering::SeqCst), 1);
    assert_eq!(downstream_executions.load(Ordering::SeqCst), 1);
    assert_eq!(project.successful_run_count(), 2);
}

#[test]
fn byte_identical_dependency_outputs_remain_two_valid_edges() {
    let executions = Arc::new(AtomicUsize::new(0));
    let first = FakeNode::new("identical-first", b"first", Arc::clone(&executions))
        .expect("first upstream");
    let second = FakeNode::new("identical-second", b"second", Arc::clone(&executions))
        .expect("second upstream");
    let shared_output =
        ArtifactRef::from_bytes("text/plain;charset=utf-8", b"xxxxxxxx").expect("shared output");
    let downstream = FakeNode {
        spec: NodeSpec::new(
            NodeId::new("identical-downstream").expect("downstream ID"),
            "fake",
            1,
            vec![first.spec().id().clone(), second.spec().id().clone()],
        )
        .expect("downstream spec"),
        inputs: vec![shared_output.clone(), shared_output],
        semantic_inputs: Vec::new(),
        input_bytes: b"xxxxxxxx".to_vec(),
        executions: Arc::clone(&executions),
        fail: false,
        decode_fail: false,
        unstable_codec: false,
        output_len: 4,
    };
    let graph = WorkflowGraph::new([
        first.spec().clone(),
        second.spec().clone(),
        downstream.spec().clone(),
    ])
    .expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1024,
    })
    .expect("scheduler");
    let mut project = MarklabProject::new();
    project
        .register_reference(first.input_artifacts()[0].clone())
        .and_then(|()| project.register_reference(second.input_artifacts()[0].clone()))
        .expect("upstream inputs");
    scheduler
        .run_single(&mut project, &graph, &first)
        .expect("first upstream run");
    scheduler
        .run_single(&mut project, &graph, &second)
        .expect("second upstream run");

    let run = scheduler
        .run_single(&mut project, &graph, &downstream)
        .expect("byte-identical dependency edges remain valid");
    assert_eq!(run.cache_status, CacheStatus::Miss);
    assert_eq!(executions.load(Ordering::SeqCst), 3);
}

#[test]
fn durable_reopen_restores_dependency_outputs_without_reexecution() {
    let directory = tempfile::tempdir().expect("temporary project directory");
    let project_path = directory.path().join("project");
    let limits = DurableProjectLimits::new(64 * 1024, 1024 * 1024, 32, 64 * 1024, 1024)
        .expect("durable limits");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1024,
    })
    .expect("scheduler");
    let upstream_executions = Arc::new(AtomicUsize::new(0));
    let downstream_executions = Arc::new(AtomicUsize::new(0));
    let upstream = FakeNode::new(
        "durable-upstream",
        b"durable-source",
        Arc::clone(&upstream_executions),
    )
    .expect("upstream");
    let upstream_output = ArtifactRef::from_bytes("text/plain;charset=utf-8", b"xxxxxxxx")
        .expect("upstream output identity");
    let downstream = FakeNode {
        spec: NodeSpec::new(
            NodeId::new("durable-downstream").expect("downstream ID"),
            "fake",
            1,
            vec![NodeId::new("durable-upstream").expect("upstream ID")],
        )
        .expect("downstream spec"),
        inputs: vec![upstream_output],
        semantic_inputs: Vec::new(),
        input_bytes: b"xxxxxxxx".to_vec(),
        executions: Arc::clone(&downstream_executions),
        fail: false,
        decode_fail: false,
        unstable_codec: false,
        output_len: 4,
    };
    let graph = WorkflowGraph::new([upstream.spec().clone(), downstream.spec().clone()])
        .expect("dependency-valid graph");
    let schema = ArtifactSchema::new("marklab.test.fake-result", 1).expect("result schema");

    {
        let mut durable = DurableProject::open_or_create(&project_path, limits).expect("project");
        let mut project = MarklabProject::new();
        project
            .register_reference(upstream.input_artifacts()[0].clone())
            .expect("upstream input");
        let upstream_run = execute_algorithm(
            &mut durable,
            &mut project,
            &graph,
            &upstream,
            &scheduler,
            schema.clone(),
            test_runtime(),
        )
        .expect("first upstream run");
        let downstream_run = execute_algorithm(
            &mut durable,
            &mut project,
            &graph,
            &downstream,
            &scheduler,
            schema.clone(),
            test_runtime(),
        )
        .expect("first downstream run");
        assert_eq!(upstream_run.cache_status, CacheStatus::Miss);
        assert_eq!(downstream_run.cache_status, CacheStatus::Miss);
        assert_eq!(durable.execution_count(), 2);
    }

    let mut durable =
        DurableProject::open_or_create(&project_path, limits).expect("reopen project");
    let mut project = MarklabProject::new();
    project
        .register_reference(upstream.input_artifacts()[0].clone())
        .expect("upstream input after reopen");
    let upstream_run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &upstream,
        &scheduler,
        schema.clone(),
        test_runtime(),
    )
    .expect("replayed upstream run");
    let downstream_run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &downstream,
        &scheduler,
        schema,
        test_runtime(),
    )
    .expect("replayed downstream run");

    assert_eq!(upstream_run.cache_status, CacheStatus::Hit);
    assert_eq!(downstream_run.cache_status, CacheStatus::Hit);
    assert_eq!(upstream_executions.load(Ordering::SeqCst), 1);
    assert_eq!(downstream_executions.load(Ordering::SeqCst), 1);
    assert_eq!(
        durable.execution_count(),
        2,
        "hits must not append executions"
    );
}

#[test]
fn dependency_edges_reject_unproduced_legacy_stale_and_ambiguous_outputs() {
    let upstream_executions = Arc::new(AtomicUsize::new(0));
    let downstream_executions = Arc::new(AtomicUsize::new(0));
    let upstream = FakeNode::new("edge-upstream", b"source", Arc::clone(&upstream_executions))
        .expect("upstream");
    let upstream_output =
        ArtifactRef::from_bytes("text/plain;charset=utf-8", b"xxxxxxxx").expect("upstream output");
    let downstream = FakeNode {
        spec: NodeSpec::new(
            NodeId::new("edge-downstream").expect("downstream ID"),
            "fake",
            1,
            vec![NodeId::new("edge-upstream").expect("upstream ID")],
        )
        .expect("downstream spec"),
        inputs: vec![upstream_output.clone()],
        semantic_inputs: Vec::new(),
        input_bytes: b"xxxxxxxx".to_vec(),
        executions: Arc::clone(&downstream_executions),
        fail: false,
        decode_fail: false,
        unstable_codec: false,
        output_len: 4,
    };
    let graph =
        WorkflowGraph::new([upstream.spec().clone(), downstream.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1024,
    })
    .expect("scheduler");
    let mut project = MarklabProject::new();
    project
        .register_reference(upstream.input_artifacts()[0].clone())
        .expect("upstream input");
    project
        .register_reference(upstream_output.clone())
        .expect("declared downstream input");

    assert!(matches!(
        scheduler.run_single(&mut project, &graph, &downstream),
        Err(WorkflowError::MissingDependencyOutput { .. })
    ));
    project
        .commit_success(
            "edge-upstream",
            ContentDigest::from_bytes(b"legacy-cache-key"),
            upstream_output.kind(),
            b"xxxxxxxx".to_vec().into_boxed_slice(),
        )
        .expect("legacy direct success");
    assert!(matches!(
        scheduler.run_single(&mut project, &graph, &downstream),
        Err(WorkflowError::MissingDependencyOutput { .. })
    ));

    scheduler
        .run_single(&mut project, &graph, &upstream)
        .expect("typed upstream success");
    let changed_upstream_spec = NodeSpec::new(
        NodeId::new("edge-upstream").expect("upstream ID"),
        "fake",
        2,
        Vec::new(),
    )
    .expect("changed upstream spec");
    let stale_graph = WorkflowGraph::new([changed_upstream_spec, downstream.spec().clone()])
        .expect("stale graph");
    assert!(matches!(
        scheduler.run_single(&mut project, &stale_graph, &downstream),
        Err(WorkflowError::MissingDependencyOutput { .. })
    ));

    let mut changed_upstream = FakeNode::new(
        "edge-upstream",
        b"changed-source",
        Arc::clone(&upstream_executions),
    )
    .expect("changed upstream");
    changed_upstream.output_len = 6;
    project
        .register_reference(changed_upstream.input_artifacts()[0].clone())
        .expect("changed upstream input");
    scheduler
        .run_single(&mut project, &graph, &changed_upstream)
        .expect("second typed upstream success");
    let changed_output = ArtifactRef::from_bytes("text/plain;charset=utf-8", b"xxxxxx")
        .expect("changed upstream output");

    let mut ambiguous = downstream.clone();
    ambiguous.spec = NodeSpec::new(
        NodeId::new("ambiguous-downstream").expect("ambiguous ID"),
        "fake",
        1,
        vec![NodeId::new("edge-upstream").expect("upstream ID")],
    )
    .expect("ambiguous spec");
    ambiguous.inputs.push(changed_output);
    let ambiguous_graph = WorkflowGraph::new([upstream.spec().clone(), ambiguous.spec().clone()])
        .expect("ambiguous graph");
    assert!(matches!(
        scheduler.run_single(&mut project, &ambiguous_graph, &ambiguous),
        Err(WorkflowError::AmbiguousDependencyOutput { matches: 2, .. })
    ));
    assert_eq!(downstream_executions.load(Ordering::SeqCst), 0);
}

#[test]
fn project_artifact_registration_and_coordinate_install_are_atomic() {
    let bytes = b"cataloged bytes";
    let original = semantic_record(bytes, 1, "source", "incoming/original");
    let replica = semantic_record(bytes, 1, "archive", "replicas/original");
    assert_eq!(original.id(), replica.id());
    let mut project = MarklabProject::new();
    project
        .register_artifact(original.clone())
        .expect("original record");
    project.register_artifact(replica).expect("replica");
    assert_eq!(project.artifact_catalog().len(), 1);
    assert_eq!(
        project
            .artifact_record(original.id())
            .expect("catalog record")
            .locations()
            .len(),
        2
    );
    assert_eq!(project.artifact_count(), 1);
    assert!(project.contains(original.content()));

    let missing = semantic_record(b"missing dependency", 1, "source", "incoming/missing");
    let dependent = ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.semantic-input", b"dependent")
            .expect("content"),
        ArtifactSchema::new("marklab.test.semantic-input", 1).expect("schema"),
        None,
        vec![missing.id()],
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("store"),
            ArtifactKey::new("incoming/dependent").expect("key"),
            None,
        )
        .expect("locator")],
    )
    .expect("dependent");
    let before = (project.artifact_catalog().len(), project.artifact_count());
    assert!(project.register_artifact(dependent).is_err());
    assert_eq!(
        (project.artifact_catalog().len(), project.artifact_count()),
        before
    );

    let coordinates =
        marklab_data::CoordinateRegistry::new(Vec::new(), Vec::new(), Vec::new(), Vec::new())
            .expect("empty coordinate registry");
    project
        .install_coordinate_registry(coordinates.clone())
        .expect("coordinate registry");
    assert!(project.coordinate_registry().is_some());
    assert!(matches!(
        project.install_coordinate_registry(coordinates),
        Err(ProjectError::CoordinateRegistryAlreadyInstalled)
    ));
}

#[test]
fn semantic_cache_requires_and_reverifies_a_bound_store_before_every_lookup() {
    let root = tempfile::tempdir().expect("store root");
    let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("store");
    let semantic_bytes = b"semantic bytes";
    let source = semantic_record(semantic_bytes, 1, "source", "incoming/semantic-v1");
    let published = store
        .publish(&source, |writer| writer.write_all(semantic_bytes))
        .expect("publish semantic input")
        .into_record();

    let executions = Arc::new(AtomicUsize::new(0));
    let mut node =
        FakeNode::new("semantic-cache", b"ordinary", Arc::clone(&executions)).expect("node");
    node.semantic_inputs = vec![published.id()];
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1024,
    })
    .expect("scheduler");
    let mut project = MarklabProject::new();
    project
        .register_reference(node.input_artifacts()[0].clone())
        .expect("ordinary input");
    project
        .register_artifact(published.clone())
        .expect("semantic input");

    assert!(matches!(
        scheduler.run_single(&mut project, &graph, &node),
        Err(WorkflowError::SemanticStoreRequired { .. })
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);

    let first = scheduler
        .run_single_with_store(&mut project, &graph, &node, &store)
        .expect("semantic miss");
    assert_eq!(first.cache_status, CacheStatus::Miss);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    let hit = scheduler
        .run_single_with_store(&mut project, &graph, &node, &store)
        .expect("verified semantic hit");
    assert_eq!(hit.cache_status, CacheStatus::Hit);
    assert_eq!(hit.cache_key, first.cache_key);
    assert_eq!(executions.load(Ordering::SeqCst), 1);

    let id = published.id().to_string();
    let managed_path = root.path().join("objects/sha256").join(&id[..2]).join(id);
    std::fs::write(&managed_path, b"tampered").expect("tamper fixture");
    assert!(matches!(
        scheduler.run_single_with_store(&mut project, &graph, &node, &store),
        Err(WorkflowError::SemanticInputIntegrity { .. })
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    std::fs::remove_file(&managed_path).expect("missing fixture");
    assert!(matches!(
        scheduler.run_single_with_store(&mut project, &graph, &node, &store),
        Err(WorkflowError::SemanticInputIntegrity { .. })
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

#[test]
fn semantic_schema_identity_changes_cache_key_and_missing_id_never_executes() {
    let root = tempfile::tempdir().expect("store root");
    let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("store");
    let bytes = b"same encoded bytes";
    let v1 = store
        .publish(
            &semantic_record(bytes, 1, "source", "incoming/schema-v1"),
            |writer| writer.write_all(bytes),
        )
        .expect("publish v1")
        .into_record();
    let v2 = store
        .publish(
            &semantic_record(bytes, 2, "source", "incoming/schema-v2"),
            |writer| writer.write_all(bytes),
        )
        .expect("publish v2")
        .into_record();
    assert_ne!(v1.id(), v2.id());

    let executions = Arc::new(AtomicUsize::new(0));
    let mut node =
        FakeNode::new("semantic-version", b"ordinary", Arc::clone(&executions)).expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1024,
    })
    .expect("scheduler");
    let mut project = MarklabProject::new();
    project
        .register_reference(node.input_artifacts()[0].clone())
        .expect("ordinary input");
    project.register_artifact(v1.clone()).expect("v1 catalog");
    project.register_artifact(v2.clone()).expect("v2 catalog");

    let missing = semantic_record(b"absent", 1, "source", "incoming/absent");
    node.semantic_inputs = vec![missing.id()];
    assert!(matches!(
        scheduler.run_single_with_store(&mut project, &graph, &node, &store),
        Err(WorkflowError::SemanticInputNotCataloged { artifact, .. })
            if artifact == missing.id()
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);

    node.semantic_inputs = vec![v1.id()];
    let first = scheduler
        .run_single_with_store(&mut project, &graph, &node, &store)
        .expect("v1 run");
    node.semantic_inputs = vec![v2.id()];
    let changed = scheduler
        .run_single_with_store(&mut project, &graph, &node, &store)
        .expect("v2 run");
    assert_eq!(first.cache_status, CacheStatus::Miss);
    assert_eq!(changed.cache_status, CacheStatus::Miss);
    assert_ne!(first.cache_key, changed.cache_key);
    assert_eq!(executions.load(Ordering::SeqCst), 2);
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

#[test]
fn marked_prepost_composes_two_produced_analysis_artifacts() {
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

    let pattern = |timepoint: &str, marks: Vec<u8>| {
        let mut pattern = Pattern::from_arrays(
            vec![0.0, 1.0, 0.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
            marks,
            PatternMeta {
                case_id: "workflow-case".into(),
                timepoint: timepoint.into(),
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
        pattern
    };
    let pre_pattern = pattern("pre", vec![1, 0, 0, 1]);
    let post_pattern = pattern("post", vec![1, 1, 0, 0]);
    let mut project = MarklabProject::new();
    let pre = MarkedAnalysisNode::new(
        &mut project,
        NodeId::new("pre-analysis").expect("pre ID"),
        &pre_pattern,
        &config,
    )
    .expect("pre node");
    let post = MarkedAnalysisNode::new(
        &mut project,
        NodeId::new("post-analysis").expect("post ID"),
        &post_pattern,
        &config,
    )
    .expect("post node");
    let upstream_graph =
        WorkflowGraph::new([pre.spec().clone(), post.spec().clone()]).expect("upstream graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("scheduler");
    let pre_run = scheduler
        .run_single(&mut project, &upstream_graph, &pre)
        .expect("pre run");
    let post_run = scheduler
        .run_single(&mut project, &upstream_graph, &post)
        .expect("post run");

    let comparison = MarkedPrePostNode::new(
        NodeId::new("marked-prepost").expect("comparison ID"),
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
    .expect("composed graph");
    let compared = scheduler
        .run_single(&mut project, &graph, &comparison)
        .expect("dependent comparison");
    let direct = marklab::compare_marked_prepost(&pre_run.output, &post_run.output);

    assert_eq!(compared.cache_status, CacheStatus::Miss);
    assert_eq!(compared.output, direct);
    let replay = scheduler
        .run_single(&mut project, &graph, &comparison)
        .expect("comparison replay");
    assert_eq!(replay.cache_status, CacheStatus::Hit);
    assert_eq!(replay.artifact, compared.artifact);
    assert_eq!(replay.output, direct);
}
