use std::path::PathBuf;

use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::super::bayes::joint_replicated_location_embedding::{
    self, Args, EmbeddingResidualFamily, Output, Prepared,
};
use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};

const LOCATION_KIND: &str =
    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1";
const EMBEDDING_KIND: &str =
    "application/vnd.marklab.source.replicated-projected-cell-embedding+csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.numpyro-joint-replicated-location-embedding+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-numpyro-joint-replicated-location-embedding-node-v1";

#[derive(Debug, clap::Args)]
pub(super) struct ProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    location_input: PathBuf,
    #[arg(long)]
    embedding_input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    embedding_projection_identity: String,
    #[arg(long)]
    factors: usize,
    #[arg(long)]
    location_prior_sd: f64,
    #[arg(long)]
    group_prior_sd: f64,
    #[arg(long)]
    patient_factor_prior_scale: f64,
    #[arg(long)]
    location_factor_loading_prior_sd: f64,
    #[arg(long)]
    embedding_loading_prior_sd: f64,
    #[arg(long)]
    embedding_noise_prior_scale: f64,
    #[arg(long, value_enum, default_value_t)]
    embedding_residual_family: EmbeddingResidualFamily,
    #[arg(long)]
    student_t_degrees_of_freedom: Option<f64>,
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
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
    maximum_location_rows: usize,
    #[arg(long)]
    maximum_embedding_points: usize,
    #[arg(long)]
    maximum_embedding_dimension: usize,
    #[arg(long)]
    maximum_factor_count: usize,
    #[arg(long)]
    maximum_nearest_node_visits: u64,
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    maximum_draw_observation_work: u64,
    #[arg(long)]
    maximum_working_bytes: u64,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: ProjectArgs) -> Result<(), BayesCliError> {
    let location_before = source_artifact(&arguments.location_input, LOCATION_KIND)?;
    let embedding_before = source_artifact(&arguments.embedding_input, EMBEDDING_KIND)?;
    let prepared = joint_replicated_location_embedding::prepare(Args {
        location_input: arguments.location_input.clone(),
        embedding_input: arguments.embedding_input.clone(),
        reference_group: arguments.reference_group,
        comparison_group: arguments.comparison_group,
        embedding_projection_identity: arguments.embedding_projection_identity,
        factors: arguments.factors,
        location_prior_sd: arguments.location_prior_sd,
        group_prior_sd: arguments.group_prior_sd,
        patient_factor_prior_scale: arguments.patient_factor_prior_scale,
        location_factor_loading_prior_sd: arguments.location_factor_loading_prior_sd,
        embedding_loading_prior_sd: arguments.embedding_loading_prior_sd,
        embedding_noise_prior_scale: arguments.embedding_noise_prior_scale,
        embedding_residual_family: arguments.embedding_residual_family,
        student_t_degrees_of_freedom: arguments.student_t_degrees_of_freedom,
        field_length_scale_prior_scale_um: arguments.field_length_scale_prior_scale_um,
        jitter: arguments.jitter,
        chains: arguments.chains,
        tune: arguments.tune,
        draws: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
        maximum_patients: arguments.maximum_patients,
        maximum_patterns: arguments.maximum_patterns,
        maximum_location_rows: arguments.maximum_location_rows,
        maximum_embedding_points: arguments.maximum_embedding_points,
        maximum_embedding_dimension: arguments.maximum_embedding_dimension,
        maximum_factor_count: arguments.maximum_factor_count,
        maximum_nearest_node_visits: arguments.maximum_nearest_node_visits,
        maximum_kernel_cube_work: arguments.maximum_kernel_cube_work,
        maximum_draw_observation_work: arguments.maximum_draw_observation_work,
        maximum_working_bytes: arguments.maximum_working_bytes,
        maximum_tree_depth: arguments.maximum_tree_depth,
        timeout_seconds: arguments.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let location_after = source_artifact(&arguments.location_input, LOCATION_KIND)?;
    let embedding_after = source_artifact(&arguments.embedding_input, EMBEDDING_KIND)?;
    if location_before != location_after || embedding_before != embedding_after {
        return Err(BayesCliError::Input(
            "joint location-embedding source changed while prepared".into(),
        ));
    }
    let node = JointLocationEmbeddingNode::new(
        arguments.location_input,
        arguments.embedding_input,
        [location_before.clone(), embedding_before.clone()],
        prepared,
    )?;
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
        .register_reference(location_before)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(embedding_before)
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
        ArtifactSchema::new("marklab.joint_replicated_location_embedding", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&arguments.out, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project joint-replicated-location-embedding cache_status={cache_status}");
    Ok(())
}

struct JointLocationEmbeddingNode {
    spec: NodeSpec,
    location_path: PathBuf,
    embedding_path: PathBuf,
    inputs: [ArtifactRef; 2],
    prepared: Prepared,
}

impl JointLocationEmbeddingNode {
    fn new(
        location_path: PathBuf,
        embedding_path: PathBuf,
        inputs: [ArtifactRef; 2],
        prepared: Prepared,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("joint-replicated-location-embedding")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "bayesian_joint_location_projected_embedding_fit",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            location_path,
            embedding_path,
            inputs,
            prepared,
        })
    }
}

impl WorkflowNode for JointLocationEmbeddingNode {
    type Output = Output;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let location =
            source_artifact(&self.location_path, LOCATION_KIND).map_err(NodeError::input)?;
        let embedding =
            source_artifact(&self.embedding_path, EMBEDDING_KIND).map_err(NodeError::input)?;
        if [location, embedding] != self.inputs {
            return Err(NodeError::input(BayesCliError::Input(
                "joint location-embedding sources no longer match durable identity".into(),
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
            execution_policy: b"pinned-numpyro-pattern-heldout-joint-location-embedding-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        joint_replicated_location_embedding::execute(&self.prepared).map_err(NodeError::execution)
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
