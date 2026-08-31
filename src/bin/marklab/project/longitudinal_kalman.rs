use std::{fs, path::PathBuf};

use marklab_longitudinal::{
    kalman_filter_and_smooth, LinearGaussianStateSpaceResult, LinearGaussianStateSpaceSpec,
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
    "application/vnd.marklab.source.longitudinal-linear-gaussian-state-space+json;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.longitudinal-linear-gaussian-state-space+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-longitudinal-kalman-rts-node-v1";

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
            "longitudinal Kalman input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let analysis: LinearGaussianStateSpaceSpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, INPUT_KIND)?;
    if before != after {
        return Err(BayesCliError::Input(
            "longitudinal Kalman input changed while the durable request was prepared".into(),
        ));
    }

    let node =
        LongitudinalKalmanProjectNode::new(input_path, before.clone(), analysis, request_bytes)?;
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
        ArtifactSchema::new("marklab.longitudinal_linear_gaussian_state_space", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project longitudinal-kalman-smooth cache_status={cache_status}");
    Ok(())
}

struct LongitudinalKalmanProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    analysis: LinearGaussianStateSpaceSpec,
    request_bytes: Vec<u8>,
}

impl LongitudinalKalmanProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        analysis: LinearGaussianStateSpaceSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("longitudinal-kalman-smooth")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "longitudinal_linear_gaussian_kalman_rts",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            analysis,
            request_bytes,
        })
    }

    fn validate_output(&self, output: &LinearGaussianStateSpaceResult) -> Result<(), String> {
        let expected_steps = self.analysis.observations.len();
        let state_dimension = self.analysis.initial_mean.len();
        let state_valid = |state: &marklab_longitudinal::StateEstimate| {
            state.time_index < expected_steps
                && state.mean.len() == state_dimension
                && state.covariance.len() == state_dimension
                && state.covariance.iter().all(|row| {
                    row.len() == state_dimension && row.iter().all(|value| value.is_finite())
                })
                && state.mean.iter().all(|value| value.is_finite())
        };
        if output.version != 1
            || output.state_dimension != state_dimension
            || output.time_steps != expected_steps
            || output.observed_components_per_step.len() != expected_steps
            || output.predicted_states.len() != expected_steps
            || output.filtered_states.len() != expected_steps
            || output.smoothed_states.len() != expected_steps
            || output.maximum_matrix_operations != self.analysis.maximum_matrix_operations
            || output.planned_matrix_operations_upper_bound > output.maximum_matrix_operations
            || !output.log_likelihood.is_finite()
            || !output.predicted_states.iter().all(state_valid)
            || !output.filtered_states.iter().all(state_valid)
            || !output.smoothed_states.iter().all(state_valid)
        {
            return Err(
                "longitudinal Kalman cached result identity, dimensions, bounds, or finite policy differs"
                    .into(),
            );
        }
        Ok(())
    }
}

impl WorkflowNode for LongitudinalKalmanProjectNode {
    type Output = LinearGaussianStateSpaceResult;

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
                "longitudinal Kalman input no longer matches its durable identity".into(),
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
            execution_policy: b"native-safe-rust-bounded-kalman-joseph-rts-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        kalman_filter_and_smooth(self.analysis.clone()).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: LinearGaussianStateSpaceResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate_output(&output)
            .map_err(|message| NodeError::decode(BayesCliError::Backend(message)))?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
