use std::{io, path::PathBuf};

use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};
use crate::cohort::max_t_calibration::{self, MaxTCalibrationOutput, PreparedMaxTCalibration};

const INPUT_KIND: &str = "application/vnd.marklab.source.max-t-calibration+csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-max-t-calibration+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-max-t-calibration-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    group_a_count: usize,
    family_sizes: Vec<usize>,
    alpha: f64,
    maximum_assignments: usize,
    maximum_assignment_endpoint_evaluations: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = max_t_calibration::prepare(
        input_path.clone(),
        group_a_count,
        family_sizes,
        alpha,
        maximum_assignments,
        maximum_assignment_endpoint_evaluations,
        memory_budget_mib,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let input_artifact = ArtifactRef::from_bytes(INPUT_KIND, &prepared.input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    if input_artifact != source_artifact(&input_path, INPUT_KIND)? {
        return Err(BayesCliError::Input(
            "Max-T calibration source changed while prepared".into(),
        ));
    }
    let node = MaxTCalibrationNode::new(input_artifact.clone(), prepared, memory_budget_mib)?;
    let runtime = native_runtime_provenance()?;
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
        .register_reference(input_artifact)
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
        ArtifactSchema::new("marklab.cohort_max_t_calibration", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project max-t-calibration cache_status={cache_status}");
    Ok(())
}

struct MaxTCalibrationNode {
    node_spec: NodeSpec,
    input_artifacts: Vec<ArtifactRef>,
    prepared: PreparedMaxTCalibration,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl MaxTCalibrationNode {
    fn new(
        input_artifact: ArtifactRef,
        prepared: PreparedMaxTCalibration,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let path_identity = serde_json::to_vec(&prepared.input).map_err(|error| {
            BayesCliError::Input(format!("input path is not JSON-serializable: {error}"))
        })?;
        let configuration = serde_json::to_vec(&prepared.configuration)
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-max-t-calibration-configuration-v1".as_slice(),
            path_identity.as_slice(),
            configuration.as_slice(),
        ]);
        let execution_policy = format!(
            "serial;exhaustive-fixed-size-whole-patient-assignments;single-step;step-down;ordered-gatekeeping-step-down;group_a_count={};family_sizes={:?};alpha_bits={};maximum_assignments={};maximum_assignment_endpoint_evaluations={};memory_budget_mib={memory_budget_mib}",
            prepared.configuration.group_a_count,
            prepared.configuration.family_sizes,
            prepared.configuration.alpha.to_bits(),
            prepared.configuration.maximum_assignments,
            prepared
                .configuration
                .maximum_assignment_endpoint_evaluations,
        )
        .into_bytes();
        Ok(Self {
            node_spec: NodeSpec::new(
                NodeId::new("max-t-calibration")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "max_t_calibration",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_artifacts: vec![input_artifact],
            prepared,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.node_spec
    }

    fn validate(&self, output: &MaxTCalibrationOutput) -> io::Result<()> {
        if !output.validates(&self.prepared) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "decoded Max-T calibration result differs from its cache-bound request",
            ));
        }
        Ok(())
    }
}

impl WorkflowNode for MaxTCalibrationNode {
    type Output = MaxTCalibrationOutput;

    fn spec(&self) -> &NodeSpec {
        &self.node_spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current =
            source_artifact(&self.prepared.input, INPUT_KIND).map_err(NodeError::input)?;
        if self.input_artifacts.as_slice() != [current] {
            return Err(NodeError::input(io::Error::new(
                io::ErrorKind::InvalidData,
                "Max-T calibration source changed after preparation",
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: &self.execution_policy,
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        max_t_calibration::execute(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
