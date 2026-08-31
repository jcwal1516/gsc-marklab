use std::path::PathBuf;

use marklab_bayes::NutsSamplingSpec;
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes::{self, spatial_varying_coefficient},
    native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.spatial-varying-coefficient-csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.bayesian-spatial-varying-coefficient-fit+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-spatial-varying-coefficient-pymc-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    global_predictor_name: String,
    spatial_predictor_name: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_sd: f64,
    amplitude_prior_sd: f64,
    length_scale_prior_sd_um: f64,
    known_noise_sd: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = spatial_varying_coefficient::prepare(
        input_path.clone(),
        global_predictor_name,
        spatial_predictor_name,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_sd,
        amplitude_prior_sd,
        length_scale_prior_sd_um,
        known_noise_sd,
        jitter,
        sampling,
        timeout_seconds,
    )?;
    let input = source_artifact(&input_path, INPUT_KIND)?;
    let node = SpatialVaryingCoefficientProjectNode::new(input_path, input.clone(), prepared)?;
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
        ArtifactSchema::new("marklab.bayesian_spatial_varying_coefficient_fit", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project spatial-varying-coefficient cache_status={cache_status}");
    Ok(())
}

struct SpatialVaryingCoefficientProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    input_artifacts: [ArtifactRef; 1],
    prepared: spatial_varying_coefficient::Prepared,
}

impl SpatialVaryingCoefficientProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: spatial_varying_coefficient::Prepared,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("spatial-varying-coefficient")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "pymc_spatial_varying_coefficient_fit",
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

impl WorkflowNode for SpatialVaryingCoefficientProjectNode {
    type Output = serde_json::Value;

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
                "spatial coefficient input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                IMPLEMENTATION_IDENTITY.as_bytes(),
                self.input_path_identity.as_slice(),
                self.prepared.request_bytes(),
            ]),
            execution_policy: b"pinned-pymc-spatial-varying-coefficient-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        let output =
            spatial_varying_coefficient::execute(&self.prepared).map_err(NodeError::execution)?;
        serde_json::to_value(output).map_err(NodeError::encoding)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: serde_json::Value =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.prepared
            .validate_output(&output)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
