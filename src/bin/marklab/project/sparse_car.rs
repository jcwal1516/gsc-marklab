use std::path::PathBuf;

use marklab_bayes::{fit_sparse_car, SparseCarError, SparseCarResult, SparseCarSpec};
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

const INPUT_KIND: &str = "application/vnd.marklab.source.sparse-car-spec+json;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.sparse-car-fit+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-sparse-car-fit-node-v1";

pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let (spec, request_bytes) = bayes::sparse_car::read_spec(&input_path)?;
    spec.validate().map_err(map_error)?;
    let before = ArtifactRef::from_bytes(INPUT_KIND, &request_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let after = source_artifact(&input_path, INPUT_KIND)?;
    if before != after {
        return Err(BayesCliError::Input(
            "sparse CAR input changed while the durable request was prepared".into(),
        ));
    }

    let node = SparseCarProjectNode::new(input_path, before.clone(), spec)?;
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
        ArtifactSchema::new("marklab.sparse_car_fit", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project sparse-car-fit cache_status={cache_status}");
    Ok(())
}

struct SparseCarProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    analysis: SparseCarSpec,
}

impl SparseCarProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        analysis: SparseCarSpec,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("sparse-car-fit")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "bayesian_sparse_car_fit",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            analysis,
        })
    }
}

impl WorkflowNode for SparseCarProjectNode {
    type Output = SparseCarResult;

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
                "sparse CAR input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([IMPLEMENTATION_IDENTITY.as_bytes()]),
            execution_policy: b"native-safe-rust-sparse-car-fit-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        fit_sparse_car(&self.analysis).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: SparseCarResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(&self.analysis)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

fn map_error(error: SparseCarError) -> BayesCliError {
    match error {
        SparseCarError::InvalidInput(message) | SparseCarError::ResourceLimit(message) => {
            BayesCliError::Input(message)
        }
        SparseCarError::Numerical(message) => BayesCliError::Backend(message),
    }
}
