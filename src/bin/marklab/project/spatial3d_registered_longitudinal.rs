use std::{fs, path::PathBuf};

use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::super::spatial3d_registered::longitudinal::{
    self, PreparedRegisteredLongitudinalVoxelK, RegisteredLongitudinalVoxelKResult,
};
use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.spatial3d-registered-longitudinal-voxel-k-change+json;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.spatial3d-registered-longitudinal-voxel-k-change+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-spatial3d-registered-longitudinal-voxel-k-change-node-v1";

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
            "registered longitudinal voxel K input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let prepared = longitudinal::prepare(&request_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let after = source_artifact(&input_path, INPUT_KIND)?;
    if before != after {
        return Err(BayesCliError::Input(
            "registered longitudinal voxel K input changed while the durable request was prepared"
                .into(),
        ));
    }

    let node = RegisteredLongitudinalProjectNode::new(
        input_path,
        before.clone(),
        prepared,
        request_bytes,
    )?;
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
        ArtifactSchema::new(
            "marklab.spatial3d_registered_longitudinal_voxel_k_change",
            1,
        )
        .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!(
        "project spatial3d-registered-longitudinal-voxel-k-change cache_status={cache_status}"
    );
    Ok(())
}

struct RegisteredLongitudinalProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedRegisteredLongitudinalVoxelK,
    request_bytes: Vec<u8>,
}

impl RegisteredLongitudinalProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedRegisteredLongitudinalVoxelK,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("spatial3d-registered-longitudinal-voxel-k-change")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "spatial3d_registered_longitudinal_voxel_k_change",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            request_bytes,
        })
    }
}

impl WorkflowNode for RegisteredLongitudinalProjectNode {
    type Output = RegisteredLongitudinalVoxelKResult;

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
                "registered longitudinal voxel K input no longer matches its durable identity"
                    .into(),
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
            execution_policy: b"native-safe-rust-registered-longitudinal-voxel-k-change-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        longitudinal::execute(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: RegisteredLongitudinalVoxelKResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_prepared(&self.prepared)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
