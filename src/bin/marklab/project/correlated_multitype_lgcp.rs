use std::path::PathBuf;

use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::super::bayes::correlated_replicated_arbitrary_window_multitype_lgcp::{
    self, Args, Output, Prepared,
};
use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.numpyro-correlated-replicated-multitype-lgcp+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-numpyro-correlated-replicated-multitype-lgcp-node-v1";

#[derive(Debug, clap::Args)]
pub(super) struct ProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    field_amplitude_prior_scale: f64,
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    lkj_concentration: f64,
    #[arg(long)]
    jitter: f64,
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
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    maximum_coregionalization_work: u64,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: ProjectArgs) -> Result<(), BayesCliError> {
    let before = source_artifact(&arguments.input, INPUT_KIND)?;
    let prepared = correlated_replicated_arbitrary_window_multitype_lgcp::prepare(Args {
        input: arguments.input.clone(),
        reference_group: arguments.reference_group,
        comparison_group: arguments.comparison_group,
        reference_type: arguments.reference_type,
        intercept_prior_mean: arguments.intercept_prior_mean,
        intercept_prior_sd: arguments.intercept_prior_sd,
        group_effect_prior_sd: arguments.group_effect_prior_sd,
        covariate_effect_prior_sd: arguments.covariate_effect_prior_sd,
        patient_sd_prior_scale: arguments.patient_sd_prior_scale,
        pattern_sd_prior_scale: arguments.pattern_sd_prior_scale,
        field_amplitude_prior_scale: arguments.field_amplitude_prior_scale,
        field_length_scale_prior_scale_um: arguments.field_length_scale_prior_scale_um,
        lkj_concentration: arguments.lkj_concentration,
        jitter: arguments.jitter,
        chains: arguments.chains,
        tune: arguments.tune,
        draws: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
        maximum_patients: arguments.maximum_patients,
        maximum_patterns: arguments.maximum_patterns,
        maximum_types: arguments.maximum_types,
        maximum_nodes_per_pattern: arguments.maximum_nodes_per_pattern,
        maximum_total_nodes: arguments.maximum_total_nodes,
        maximum_total_node_type_rows: arguments.maximum_total_node_type_rows,
        maximum_total_events: arguments.maximum_total_events,
        maximum_draw_node_type_work: arguments.maximum_draw_node_type_work,
        maximum_kernel_cube_work: arguments.maximum_kernel_cube_work,
        maximum_coregionalization_work: arguments.maximum_coregionalization_work,
        maximum_tree_depth: arguments.maximum_tree_depth,
        timeout_seconds: arguments.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let after = source_artifact(&arguments.input, INPUT_KIND)?;
    if before != after {
        return Err(BayesCliError::Input(
            "correlated multitype source changed while its durable request was prepared".into(),
        ));
    }
    let node = CorrelatedMultitypeLgcpNode::new(arguments.input, before.clone(), prepared)?;
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
        ArtifactSchema::new("marklab.correlated_multitype_lgcp", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&arguments.out, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!(
        "project correlated-replicated-arbitrary-window-multitype-lgcp cache_status={cache_status}"
    );
    Ok(())
}

struct CorrelatedMultitypeLgcpNode {
    spec: NodeSpec,
    input_path: PathBuf,
    inputs: [ArtifactRef; 1],
    prepared: Prepared,
}

impl CorrelatedMultitypeLgcpNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: Prepared,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("correlated-replicated-multitype-lgcp")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "bayesian_correlated_multitype_exact_window_point_process",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            inputs: [input],
            prepared,
        })
    }
}

impl WorkflowNode for CorrelatedMultitypeLgcpNode {
    type Output = Output;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, INPUT_KIND).map_err(NodeError::input)?;
        if observed != self.inputs[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "correlated multitype source no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                IMPLEMENTATION_IDENTITY.as_bytes(),
                self.prepared.request_bytes(),
            ]),
            execution_policy: b"pinned-numpyro-lkj-coregionalized-multitype-lgcp-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        correlated_replicated_arbitrary_window_multitype_lgcp::execute(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: Output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(&self.prepared)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
