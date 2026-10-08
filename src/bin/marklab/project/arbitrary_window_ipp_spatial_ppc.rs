use std::path::PathBuf;

use marklab_bayes::NutsSamplingSpec;
use marklab_workflow::{
    execute_algorithm, ArtifactRef, CacheKeyMaterial, CacheStatus, DurableProject,
    DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId, NodeSpec,
    SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact,
    ArbitraryWindowIppSpatialPpcProjectArgs, BayesCliError, StaticBackendDescriptor,
    StaticBackendWorkflow, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

pub(super) fn run(arguments: ArbitraryWindowIppSpatialPpcProjectArgs) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcArbitraryWindowIppSpatialPpc.descriptor();
    let paths = [
        arguments.events,
        arguments.event_membership,
        arguments.quadrature,
        arguments.window,
    ];
    let before = [
        source_artifact(&paths[0], backend.input_kinds[0])?,
        source_artifact(&paths[1], backend.input_kinds[1])?,
        source_artifact(&paths[2], backend.input_kinds[2])?,
        source_artifact(&paths[3], backend.input_kinds[3])?,
    ];
    let prepared = bayes::prepare_arbitrary_window_ipp_spatial_ppc(bayes::SpatialPpcParameters {
        events: paths[0].clone(),
        event_membership: paths[1].clone(),
        quadrature: paths[2].clone(),
        window: paths[3].clone(),
        intercept_prior_mean: arguments.intercept_prior_mean,
        intercept_prior_sd: arguments.intercept_prior_sd,
        coefficient_prior_mean: arguments.coefficient_prior_mean,
        coefficient_prior_sd: arguments.coefficient_prior_sd,
        sampling: NutsSamplingSpec {
            chains: arguments.chains,
            tune_per_chain: arguments.tune,
            draws_per_chain: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
        },
        prediction_seed: arguments.prediction_seed,
        neighbor_radius_um: arguments.neighbor_radius_um,
        maximum_events: arguments.maximum_events,
        maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
        maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
        maximum_draw_node_work: arguments.maximum_draw_node_work,
        timeout_seconds: arguments.timeout_seconds,
    })?;
    let after = [
        source_artifact(&paths[0], backend.input_kinds[0])?,
        source_artifact(&paths[1], backend.input_kinds[1])?,
        source_artifact(&paths[2], backend.input_kinds[2])?,
        source_artifact(&paths[3], backend.input_kinds[3])?,
    ];
    if before != after {
        return Err(BayesCliError::Input(
            "arbitrary-window IPP spatial PPC source changed while the durable request was prepared"
                .into(),
        ));
    }

    let node = SpatialPpcProjectNode::new(paths, before.clone(), prepared, backend)?;
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
    for artifact in before {
        project
            .register_reference(artifact)
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
        backend.result_schema()?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&arguments.out, &execution.output)?;
    let status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project arbitrary-window-ipp-spatial-ppc cache_status={status}");
    Ok(())
}

struct SpatialPpcProjectNode {
    spec: NodeSpec,
    input_paths: [PathBuf; 4],
    input_artifacts: [ArtifactRef; 4],
    prepared: bayes::PreparedSpatialPpc,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl SpatialPpcProjectNode {
    fn new(
        input_paths: [PathBuf; 4],
        input_artifacts: [ArtifactRef; 4],
        prepared: bayes::PreparedSpatialPpc,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(prepared.backend_contract())?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_paths,
            input_artifacts,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for SpatialPpcProjectNode {
    type Output = bayes::SpatialPpcResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        for ((path, artifact), kind) in self
            .input_paths
            .iter()
            .zip(&self.input_artifacts)
            .zip(self.backend.input_kinds)
        {
            let observed = source_artifact(path, kind).map_err(NodeError::input)?;
            if &observed != artifact {
                return Err(NodeError::input(BayesCliError::Input(
                    "arbitrary-window IPP spatial PPC source no longer matches its durable identity"
                        .into(),
                )));
            }
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.backend.configuration_digest(
                self.prepared.backend_contract(),
                self.prepared.request_bytes(),
            ),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_arbitrary_window_ipp_spatial_ppc(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::SpatialPpcResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        bayes::validate_arbitrary_window_ipp_spatial_ppc(&self.prepared, &output)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}
