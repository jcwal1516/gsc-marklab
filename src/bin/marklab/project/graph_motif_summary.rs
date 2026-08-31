use std::{fs, path::PathBuf};

use marklab_graph::{
    typed_triangle_motif_summary_workflow, TypedTriangleMotifSpec, TypedTriangleMotifSummaryResult,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.graph-motif-triangle-summary+json;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.graph-motif-triangle-summary+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-graph-motif-triangle-summary-node-v1";

pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let before = source_artifact(&input_path, INPUT_KIND)?;
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "graph motif summary input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let graph_spec: TypedTriangleMotifSpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, INPUT_KIND)?;
    if before != after {
        return Err(BayesCliError::Input(
            "graph motif summary input changed while the durable request was prepared".into(),
        ));
    }

    let node =
        GraphMotifSummaryProjectNode::new(input_path, before.clone(), graph_spec, request_bytes)?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let execution = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.graph_motif_triangle_summary", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project graph-motif-triangle-summary cache_status={cache_status}");
    Ok(())
}

struct GraphMotifSummaryProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    graph_spec: TypedTriangleMotifSpec,
    request_bytes: Vec<u8>,
}

impl GraphMotifSummaryProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        graph_spec: TypedTriangleMotifSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("graph-motif-triangle-summary")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_typed_triangle_motif_summary",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            graph_spec,
            request_bytes,
        })
    }
}

impl WorkflowNode for GraphMotifSummaryProjectNode {
    type Output = TypedTriangleMotifSummaryResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, INPUT_KIND).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "graph motif summary input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                IMPLEMENTATION_IDENTITY.as_bytes(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-graph-motif-triangle-summary-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        typed_triangle_motif_summary_workflow(self.graph_spec.clone()).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: TypedTriangleMotifSummaryResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.graph_spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
