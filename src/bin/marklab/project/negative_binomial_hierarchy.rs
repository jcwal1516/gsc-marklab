use std::path::PathBuf;

use marklab_bayes::NegativeBinomialHierarchyResult;
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes::{self, negative_binomial_hierarchy},
    native_runtime_provenance, report_recovery, BayesCliError, MAXIMUM_RESULT_BYTES,
    PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const OUTPUT_KIND: &str = "application/vnd.marklab.negative-binomial-hierarchy+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-negative-binomial-hierarchy-v1";

pub(super) fn run(
    project_path: PathBuf,
    input: PathBuf,
    output: PathBuf,
) -> Result<(), BayesCliError> {
    // Validate the full design and budgets before creating durable state.
    let prepared = negative_binomial_hierarchy::prepare(&input)?;
    let node = CountHierarchyNode {
        spec: NodeSpec::new(
            NodeId::new("negative-binomial-hierarchy")
                .map_err(|e| BayesCliError::Input(e.to_string()))?,
            "bayesian_negative_binomial_patient_slide_fit",
            1,
            Vec::new(),
        )
        .map_err(|e| BayesCliError::Input(e.to_string()))?,
        input,
        prepared,
    };
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    for artifact in &node.prepared.artifacts {
        project
            .register_reference(artifact.clone())
            .map_err(|e| BayesCliError::Input(e.to_string()))?;
    }
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let execution = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.negative_binomial_hierarchy", 1)
            .map_err(|e| BayesCliError::Input(e.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    bayes::publish_json(&output, &execution.output)?;
    let status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project negative-binomial-hierarchy cache_status={status}");
    Ok(())
}

struct CountHierarchyNode {
    spec: NodeSpec,
    input: PathBuf,
    prepared: negative_binomial_hierarchy::Prepared,
}

impl WorkflowNode for CountHierarchyNode {
    type Output = NegativeBinomialHierarchyResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.prepared.artifacts
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current =
            negative_binomial_hierarchy::prepare(&self.input).map_err(NodeError::input)?;
        if current.artifacts != self.prepared.artifacts
            || current.request_bytes != self.prepared.request_bytes
        {
            return Err(NodeError::input(BayesCliError::Input(
                "negative-binomial source, worker, lock or request changed after preparation"
                    .into(),
            )));
        }
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                IMPLEMENTATION_IDENTITY.as_bytes(),
                self.prepared.request_bytes.as_slice(),
            ]),
            execution_policy: b"bounded-pinned-numpyro-negative-binomial-hierarchy-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        negative_binomial_hierarchy::execute(&self.prepared).map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let result: NegativeBinomialHierarchyResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        result
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(result)
    }
    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
