use std::path::PathBuf;

use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes, native_runtime_provenance, report_recovery, BayesCliError, MAXIMUM_RESULT_BYTES,
    PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};
use crate::bayes_advanced::{self, PreparedAdvancedBayes};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.nonstationary-adaptive-window-spde+json;version=1";
const LOCK_KIND: &str = "application/vnd.marklab.python-lock+text;version=1";
const WORKER_KIND: &str = "application/vnd.marklab.python-worker+source;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.nonstationary-adaptive-window-spde+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-nonstationary-adaptive-window-spde-node-v1";

pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = bayes_advanced::prepare("nonstationary_adaptive_window_spde", &input_path)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let artifacts = artifacts(&prepared)?;
    let node = NonstationaryAdaptiveSpdeProjectNode::new(input_path, artifacts.clone(), prepared)?;
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
    for artifact in &artifacts {
        project
            .register_reference(artifact.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
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
        ArtifactSchema::new("marklab.nonstationary_adaptive_window_spde", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project nonstationary-adaptive-window-spde cache_status={cache_status}");
    Ok(())
}

fn artifacts(prepared: &PreparedAdvancedBayes) -> Result<Vec<ArtifactRef>, BayesCliError> {
    [
        (INPUT_KIND, prepared.input_bytes.as_slice()),
        (LOCK_KIND, prepared.lock_bytes.as_slice()),
        (WORKER_KIND, prepared.worker_bytes.as_slice()),
    ]
    .into_iter()
    .map(|(kind, bytes)| {
        ArtifactRef::from_bytes(kind, bytes)
            .map_err(|error| BayesCliError::Input(error.to_string()))
    })
    .collect()
}

struct NonstationaryAdaptiveSpdeProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: Vec<ArtifactRef>,
    prepared: PreparedAdvancedBayes,
}

impl NonstationaryAdaptiveSpdeProjectNode {
    fn new(
        input_path: PathBuf,
        input_artifacts: Vec<ArtifactRef>,
        prepared: PreparedAdvancedBayes,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("nonstationary-adaptive-window-spde")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "nonstationary_adaptive_spatial_field_fit",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts,
            prepared,
        })
    }
}

impl WorkflowNode for NonstationaryAdaptiveSpdeProjectNode {
    type Output = serde_json::Value;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current =
            bayes_advanced::prepare("nonstationary_adaptive_window_spde", &self.input_path)
                .map_err(NodeError::input)?;
        let current_artifacts = artifacts(&current).map_err(NodeError::input)?;
        if current_artifacts != self.input_artifacts {
            return Err(NodeError::input(BayesCliError::Input(
                "nonstationary adaptive SPDE inputs changed after preparation".into(),
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
            execution_policy: b"bounded-pinned-scipy-nonstationary-adaptive-spde-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes_advanced::execute(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: serde_json::Value =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        bayes_advanced::validate_result(&self.prepared, &output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
