use std::path::PathBuf;

use marklab_cohort::NoninferiorityDirection;
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
use crate::cohort::{equivalence, noninferiority};

const INPUT_KIND: &str = "application/vnd.marklab.source.patient-effects+csv;version=1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run_equivalence(
    project_path: PathBuf,
    input_path: PathBuf,
    lower_margin: f64,
    upper_margin: f64,
    alpha: f64,
    margin_rationale: String,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input = source_artifact(&input_path, INPUT_KIND)?;
    let prepared = equivalence::prepare(
        input_path.clone(),
        lower_margin,
        upper_margin,
        alpha,
        margin_rationale,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = EquivalenceNode::new(input_path, input.clone(), prepared)?;
    let execution = execute_node(&project_path, input, &node, "marklab.cohort_equivalence")?;
    bayes::publish_json(&output_path, &execution.output)?;
    eprintln!(
        "project cohort-equivalence cache_status={}",
        cache_name(execution.cache_status)
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_noninferiority(
    project_path: PathBuf,
    input_path: PathBuf,
    direction: NoninferiorityDirection,
    margin: f64,
    alpha: f64,
    margin_rationale: String,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input = source_artifact(&input_path, INPUT_KIND)?;
    let prepared = noninferiority::prepare(
        input_path.clone(),
        direction,
        margin,
        alpha,
        margin_rationale,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = NoninferiorityNode::new(input_path, input.clone(), prepared)?;
    let execution = execute_node(&project_path, input, &node, "marklab.cohort_noninferiority")?;
    bayes::publish_json(&output_path, &execution.output)?;
    eprintln!(
        "project cohort-noninferiority cache_status={}",
        cache_name(execution.cache_status)
    );
    Ok(())
}

fn execute_node<N: WorkflowNode>(
    project_path: &std::path::Path,
    input: ArtifactRef,
    node: &N,
    schema: &str,
) -> Result<marklab_workflow::NodeRun<N::Output>, BayesCliError> {
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(project_path, limits)
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
    execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        node,
        &scheduler,
        ArtifactSchema::new(schema, 1).map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))
}

fn cache_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    }
}

struct EquivalenceNode {
    spec: NodeSpec,
    input_path: PathBuf,
    inputs: [ArtifactRef; 1],
    prepared: equivalence::PreparedEquivalence,
    configuration: ContentDigest,
}

impl EquivalenceNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: equivalence::PreparedEquivalence,
    ) -> Result<Self, BayesCliError> {
        let configuration = ContentDigest::from_bytes(
            &serde_json::to_vec(&(
                prepared.spec.lower_margin,
                prepared.spec.upper_margin,
                prepared.spec.alpha,
                &prepared.spec.margin_rationale,
            ))
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        );
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("cohort-equivalence")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "patient_tost_equivalence",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            inputs: [input],
            prepared,
            configuration,
        })
    }
}

impl WorkflowNode for EquivalenceNode {
    type Output = equivalence::EquivalenceOutput;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        verify(&self.input_path, &self.inputs[0])
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration,
            execution_policy: b"patient-unit-one-sample-student-t-tost-declared-margins-v1",
            implementation_identity: "marklab-project-cohort-equivalence-v1",
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        equivalence::execute(&self.prepared).map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        equivalence::validate_result(&self.prepared, &output)
            .map_err(|error| NodeError::decode(std::io::Error::other(error)))?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.cohort-equivalence+json;version=1"
    }
}

struct NoninferiorityNode {
    spec: NodeSpec,
    input_path: PathBuf,
    inputs: [ArtifactRef; 1],
    prepared: noninferiority::PreparedNoninferiority,
    configuration: ContentDigest,
}

impl NoninferiorityNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: noninferiority::PreparedNoninferiority,
    ) -> Result<Self, BayesCliError> {
        let direction = match prepared.spec.direction {
            NoninferiorityDirection::HigherIsBetter => "higher_is_better",
            NoninferiorityDirection::LowerIsBetter => "lower_is_better",
        };
        let configuration = ContentDigest::from_bytes(
            &serde_json::to_vec(&(
                direction,
                prepared.spec.margin,
                prepared.spec.alpha,
                &prepared.spec.margin_rationale,
            ))
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        );
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("cohort-noninferiority")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "patient_student_t_noninferiority",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            inputs: [input],
            prepared,
            configuration,
        })
    }
}

impl WorkflowNode for NoninferiorityNode {
    type Output = noninferiority::NoninferiorityOutput;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        verify(&self.input_path, &self.inputs[0])
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration,
            execution_policy:
                b"patient-unit-one-sample-student-t-noninferiority-declared-margin-v1",
            implementation_identity: "marklab-project-cohort-noninferiority-v1",
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        noninferiority::execute(&self.prepared).map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        noninferiority::validate_result(&self.prepared, &output)
            .map_err(|error| NodeError::decode(std::io::Error::other(error)))?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.cohort-noninferiority+json;version=1"
    }
}

fn verify(path: &std::path::Path, expected: &ArtifactRef) -> Result<(), NodeError> {
    let current = source_artifact(path, INPUT_KIND).map_err(NodeError::input)?;
    if &current != expected {
        return Err(NodeError::input(BayesCliError::Input(
            "patient-effect source changed after preparation".into(),
        )));
    }
    Ok(())
}
