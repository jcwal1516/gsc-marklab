use std::path::PathBuf;

use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::super::bayes::joint_replicated_location_mark::{self, Args, Output, Prepared};
use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};

const LOCATION_KIND: &str =
    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1";
const MARK_KIND: &str =
    "application/vnd.marklab.source.replicated-conditional-multitype-mark+csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.numpyro-joint-replicated-location-mark+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-numpyro-joint-replicated-location-mark-node-v1";

#[derive(Debug, clap::Args)]
pub(super) struct ProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    location_input: PathBuf,
    #[arg(long)]
    mark_input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long)]
    neighbor_radius_um: f64,
    #[arg(long)]
    location_prior_sd: f64,
    #[arg(long)]
    group_prior_sd: f64,
    #[arg(long)]
    patient_ecology_prior_scale: f64,
    #[arg(long)]
    mark_intercept_prior_sd: f64,
    #[arg(long)]
    mark_group_prior_sd: f64,
    #[arg(long)]
    mark_loading_prior_sd: f64,
    #[arg(long)]
    interaction_prior_sd: f64,
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
    maximum_location_rows: usize,
    #[arg(long)]
    maximum_mark_points: usize,
    #[arg(long)]
    maximum_neighbor_visits: u64,
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
    let mark_before = source_artifact(&arguments.mark_input, MARK_KIND)?;
    let prepared = joint_replicated_location_mark::prepare(Args {
        location_input: arguments.location_input.clone(),
        mark_input: arguments.mark_input.clone(),
        reference_group: arguments.reference_group,
        comparison_group: arguments.comparison_group,
        reference_type: arguments.reference_type,
        neighbor_radius_um: arguments.neighbor_radius_um,
        location_prior_sd: arguments.location_prior_sd,
        group_prior_sd: arguments.group_prior_sd,
        patient_ecology_prior_scale: arguments.patient_ecology_prior_scale,
        mark_intercept_prior_sd: arguments.mark_intercept_prior_sd,
        mark_group_prior_sd: arguments.mark_group_prior_sd,
        mark_loading_prior_sd: arguments.mark_loading_prior_sd,
        interaction_prior_sd: arguments.interaction_prior_sd,
        chains: arguments.chains,
        tune: arguments.tune,
        draws: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
        maximum_patients: arguments.maximum_patients,
        maximum_patterns: arguments.maximum_patterns,
        maximum_types: arguments.maximum_types,
        maximum_location_rows: arguments.maximum_location_rows,
        maximum_mark_points: arguments.maximum_mark_points,
        maximum_neighbor_visits: arguments.maximum_neighbor_visits,
        maximum_draw_observation_work: arguments.maximum_draw_observation_work,
        maximum_working_bytes: arguments.maximum_working_bytes,
        maximum_tree_depth: arguments.maximum_tree_depth,
        timeout_seconds: arguments.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let location_after = source_artifact(&arguments.location_input, LOCATION_KIND)?;
    let mark_after = source_artifact(&arguments.mark_input, MARK_KIND)?;
    if location_before != location_after || mark_before != mark_after {
        return Err(BayesCliError::Input(
            "joint location-mark source changed while prepared".into(),
        ));
    }
    let node = JointLocationMarkNode::new(
        arguments.location_input,
        arguments.mark_input,
        [location_before.clone(), mark_before.clone()],
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
        .register_reference(mark_before)
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
        ArtifactSchema::new("marklab.joint_replicated_location_mark", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&arguments.out, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project joint-replicated-location-mark cache_status={cache_status}");
    Ok(())
}

struct JointLocationMarkNode {
    spec: NodeSpec,
    location_path: PathBuf,
    mark_path: PathBuf,
    inputs: [ArtifactRef; 2],
    prepared: Prepared,
}

impl JointLocationMarkNode {
    fn new(
        location_path: PathBuf,
        mark_path: PathBuf,
        inputs: [ArtifactRef; 2],
        prepared: Prepared,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("joint-replicated-location-mark")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "bayesian_joint_location_conditional_mark_fit",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            location_path,
            mark_path,
            inputs,
            prepared,
        })
    }
}

impl WorkflowNode for JointLocationMarkNode {
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
        let marks = source_artifact(&self.mark_path, MARK_KIND).map_err(NodeError::input)?;
        if [location, marks] != self.inputs {
            return Err(NodeError::input(BayesCliError::Input(
                "joint location-mark sources no longer match durable identity".into(),
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
            execution_policy: b"pinned-numpyro-patient-pattern-heldout-joint-location-mark-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        joint_replicated_location_mark::execute(&self.prepared).map_err(NodeError::execution)
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
