use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};
use crate::bayes::PreparedOrdinalSiteHeldout;
use marklab_bayes::OrdinalSiteHeldoutComparison;
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use std::{io, path::PathBuf};
const INPUT_KIND: &str = "application/vnd.marklab.source.ordinal-site-heldout+csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.ordinal-site-heldout-comparison+json;version=1";
const IMPLEMENTATION: &str = "marklab-project-ordinal-site-heldout-comparison-node-v1";
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    ordered_levels: Vec<String>,
    smoothing: f64,
    optimizer_tolerance: f64,
    maximum_optimizer_evaluations: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = bayes::prepare_ordinal_site_heldout(
        input_path.clone(),
        reference_group,
        comparison_group,
        ordered_levels,
        smoothing,
        optimizer_tolerance,
        maximum_optimizer_evaluations,
    )?;
    let input = source_artifact(&input_path, INPUT_KIND)?;
    let node = Node::new(input.clone(), prepared)?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    project
        .register_reference(input)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let execution = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.ordinal_site_heldout_comparison", 1)
            .map_err(|e| BayesCliError::Input(e.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    eprintln!(
        "project ordinal-site-heldout-comparison cache_status={}",
        if execution.cache_status == CacheStatus::Hit {
            "hit"
        } else {
            "miss"
        }
    );
    Ok(())
}
struct Node {
    spec: NodeSpec,
    input: [ArtifactRef; 1],
    prepared: PreparedOrdinalSiteHeldout,
    configuration: ContentDigest,
    policy: Vec<u8>,
}
impl Node {
    fn new(
        input: ArtifactRef,
        prepared: PreparedOrdinalSiteHeldout,
    ) -> Result<Self, BayesCliError> {
        let bytes =
            serde_json::to_vec(&prepared.spec).map_err(|e| BayesCliError::Input(e.to_string()))?;
        Ok(Self{spec:NodeSpec::new(NodeId::new("ordinal-site-heldout-comparison").map_err(|e|BayesCliError::Input(e.to_string()))?,"ordinal_site_heldout_comparison",1,Vec::new()).map_err(|e|BayesCliError::Input(e.to_string()))?,input:[input],prepared,configuration:ContentDigest::from_framed([b"marklab-ordinal-site-heldout-v1".as_slice(),bytes.as_slice()]),policy:b"serial;leave-one-entire-site-out;training-fold-only-fit;bounded-coordinate-search;fixed-smoothing".to_vec()})
    }
    fn validate(&self, o: &OrdinalSiteHeldoutComparison) -> io::Result<()> {
        if o.validates(&self.prepared.spec) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordinal heldout output differs from cache-bound request",
            ))
        }
    }
}
impl WorkflowNode for Node {
    type Output = OrdinalSiteHeldoutComparison;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current =
            source_artifact(&self.prepared.input_path, INPUT_KIND).map_err(NodeError::input)?;
        if self.input != [current] {
            return Err(NodeError::input(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordinal heldout source changed",
            )));
        }
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration,
            execution_policy: &self.policy,
            implementation_identity: IMPLEMENTATION,
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_ordinal_site_heldout(&self.prepared).map_err(NodeError::execution)
    }
    fn encode_output(&self, o: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(o)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }
    fn decode_output(&self, b: &[u8]) -> Result<Self::Output, NodeError> {
        let o = serde_json::from_slice(b).map_err(NodeError::decode)?;
        self.validate(&o).map_err(NodeError::decode)?;
        Ok(o)
    }
    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
