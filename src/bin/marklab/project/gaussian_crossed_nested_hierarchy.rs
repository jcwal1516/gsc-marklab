use std::path::PathBuf;

use clap::Args;
use marklab_bayes::{GaussianCrossedNestedWorkerResult, NutsSamplingSpec};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes::{self, gaussian_crossed_nested_hierarchy},
    native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.gaussian-crossed-nested-observations+csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.pymc-gaussian-crossed-nested-worker-result+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-pymc-gaussian-crossed-nested-hierarchy-node-v1";

#[derive(Debug, Args)]
pub(super) struct ProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    slope_prior_sd: f64,
    #[arg(long)]
    component_prior_sd: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: ProjectArgs) -> Result<(), BayesCliError> {
    let prepared = gaussian_crossed_nested_hierarchy::prepare(
        arguments.input.clone(),
        arguments.intercept_prior_sd,
        arguments.slope_prior_sd,
        arguments.component_prior_sd,
        NutsSamplingSpec {
            chains: arguments.chains,
            tune_per_chain: arguments.tune,
            draws_per_chain: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
        },
        arguments.timeout_seconds,
    )?;
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let input = source_artifact(&arguments.input, INPUT_KIND)?;
    let node = GaussianCrossedNestedProjectNode::new(arguments.input, input.clone(), prepared)?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&arguments.project, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(input)
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
        ArtifactSchema::new(
            "marklab.pymc_gaussian_crossed_nested_hierarchy_worker_result",
            1,
        )
        .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let fit = execution
        .output
        .into_fit(request_for_output, input_identity);
    bayes::publish_json(&arguments.out, &fit)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project gaussian-crossed-nested-hierarchy cache_status={cache_status}");
    Ok(())
}

struct GaussianCrossedNestedProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    input_artifacts: [ArtifactRef; 1],
    prepared: gaussian_crossed_nested_hierarchy::PreparedGaussianCrossedNestedHierarchy,
}

impl GaussianCrossedNestedProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: gaussian_crossed_nested_hierarchy::PreparedGaussianCrossedNestedHierarchy,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("pymc-gaussian-crossed-nested-hierarchy")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "bayesian_crossed_nested_hierarchy_fit",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path_identity: input_path.to_string_lossy().into_owned().into_bytes(),
            input_path,
            input_artifacts: [input],
            prepared,
        })
    }
}

impl WorkflowNode for GaussianCrossedNestedProjectNode {
    type Output = GaussianCrossedNestedWorkerResult;

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
                "crossed/nested hierarchy input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                IMPLEMENTATION_IDENTITY.as_bytes(),
                self.input_path_identity.as_slice(),
                self.prepared.request_bytes.as_slice(),
            ]),
            execution_policy: b"pinned-pymc-crossed-nested-hierarchy-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        gaussian_crossed_nested_hierarchy::execute(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GaussianCrossedNestedWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
