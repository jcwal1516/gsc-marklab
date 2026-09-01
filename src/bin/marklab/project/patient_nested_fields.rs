use std::{io, path::PathBuf};

use marklab_cohort::{
    execute_patient_nested_fields, prepare_patient_nested_fields, NestedFieldSpec,
    PreparedNestedFields,
};
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
use crate::cohort::patient_nested_fields::{self, PatientNestedFieldsOutput};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.patient-nested-field-endpoints+csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-patient-nested-fields+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-patient-nested-fields-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    group_a: String,
    group_b: String,
    permutations: usize,
    seed: u64,
    alpha: f64,
    maximum_patients: usize,
    maximum_specimens: usize,
    maximum_endpoints: usize,
    maximum_permutation_endpoint_evaluations: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_budget_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("memory budget overflowed".into()))?;
    let specification = NestedFieldSpec {
        group_a,
        group_b,
        permutations,
        seed,
        alpha,
        maximum_patients,
        maximum_specimens,
        maximum_endpoints,
        maximum_permutation_endpoint_evaluations,
        memory_budget_bytes,
    };
    let input_artifact = source_artifact(&input_path, INPUT_KIND)?;
    let records = patient_nested_fields::read_records(&input_path)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let prepared = prepare_patient_nested_fields(&records, &specification)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    if input_artifact != source_artifact(&input_path, INPUT_KIND)? {
        return Err(BayesCliError::Input(
            "patient-nested field source changed while prepared".into(),
        ));
    }
    let node = PatientNestedFieldsNode::new(
        input_path,
        input_artifact.clone(),
        specification,
        prepared,
        memory_budget_mib,
    )?;
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
        ArtifactSchema::new("marklab.cohort_patient_nested_fields", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project patient-nested-fields cache_status={cache_status}");
    Ok(())
}

struct PatientNestedFieldsNode {
    node_spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: Vec<ArtifactRef>,
    specification: NestedFieldSpec,
    prepared: PreparedNestedFields,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl PatientNestedFieldsNode {
    fn new(
        input_path: PathBuf,
        input_artifact: ArtifactRef,
        specification: NestedFieldSpec,
        prepared: PreparedNestedFields,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
            BayesCliError::Input(format!("input path is not JSON-serializable: {error}"))
        })?;
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-patient-nested-fields-configuration-v1".as_slice(),
            input_path_identity.as_slice(),
            specification.group_a.as_bytes(),
            specification.group_b.as_bytes(),
            specification.permutations.to_string().as_bytes(),
            specification.seed.to_string().as_bytes(),
            specification.alpha.to_bits().to_be_bytes().as_slice(),
            specification.maximum_patients.to_string().as_bytes(),
            specification.maximum_specimens.to_string().as_bytes(),
            specification.maximum_endpoints.to_string().as_bytes(),
            specification
                .maximum_permutation_endpoint_evaluations
                .to_string()
                .as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;equal-weight-specimens-inside-patients;fold-internal-z-score-nearest-centroid;whole-patient-step-down-max-t;permutations={};seed={};alpha_bits={};maximum_patients={};maximum_specimens={};maximum_endpoints={};maximum_permutation_endpoint_evaluations={};memory_budget_mib={memory_budget_mib}",
            specification.permutations,
            specification.seed,
            specification.alpha.to_bits(),
            specification.maximum_patients,
            specification.maximum_specimens,
            specification.maximum_endpoints,
            specification.maximum_permutation_endpoint_evaluations,
        )
        .into_bytes();
        Ok(Self {
            node_spec: NodeSpec::new(
                NodeId::new("patient-nested-fields")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "patient_nested_fields",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: vec![input_artifact],
            specification,
            prepared,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.node_spec
    }

    fn validate(&self, output: &PatientNestedFieldsOutput) -> io::Result<()> {
        if !output.matches_request(&self.input_path, &self.prepared, &self.specification) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "decoded patient-nested field result differs from its cache-bound request",
            ));
        }
        Ok(())
    }
}

impl WorkflowNode for PatientNestedFieldsNode {
    type Output = PatientNestedFieldsOutput;

    fn spec(&self) -> &NodeSpec {
        &self.node_spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = source_artifact(&self.input_path, INPUT_KIND).map_err(NodeError::input)?;
        if self.input_artifacts.as_slice() != [current] {
            return Err(NodeError::input(io::Error::new(
                io::ErrorKind::InvalidData,
                "patient-nested field source changed after preparation",
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
        let result = execute_patient_nested_fields(&self.prepared, &self.specification)
            .map_err(NodeError::execution)?;
        Ok(PatientNestedFieldsOutput::from_result(
            self.input_path.clone(),
            result,
        ))
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
