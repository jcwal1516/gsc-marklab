use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::PathBuf,
};

use marklab_cohort::{
    max_t_multiple_endpoint_blocked_permutation,
    max_t_multiple_endpoint_blocked_step_down_permutation, max_t_multiple_endpoint_permutation,
    max_t_multiple_endpoint_step_down_permutation, MaxTPermutationResult, MaxTPermutationSpec,
    PatientEndpointVector, PatientExchangeabilityBlock,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.patient-endpoint-vectors-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-max-t+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-max-t-node-v1";
const FIXED_MAXIMUM_PATIENT_ENDPOINT_CELLS: u64 = 100_000_000;
const FIXED_MAXIMUM_MAX_T_EVALUATIONS: u64 = 100_000_000;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    group_a: String,
    group_b: String,
    permutations: usize,
    seed: u64,
    alpha: f64,
    step_down: bool,
    maximum_patients: usize,
    maximum_endpoints: usize,
    maximum_patient_endpoint_cells: u64,
    maximum_permutation_endpoint_evaluations: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_patients == 0
        || maximum_endpoints == 0
        || maximum_patient_endpoint_cells == 0
        || maximum_permutation_endpoint_evaluations == 0
        || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "cohort Max-T patient, endpoint, cell-work, permutation-work, and memory limits must be positive"
                .into(),
        ));
    }
    if maximum_patient_endpoint_cells > FIXED_MAXIMUM_PATIENT_ENDPOINT_CELLS
        || maximum_permutation_endpoint_evaluations > FIXED_MAXIMUM_MAX_T_EVALUATIONS
    {
        return Err(BayesCliError::Input(
            "cohort Max-T declared work limits exceed the fixed production ceilings".into(),
        ));
    }
    let input_path_json = serde_json::to_vec(&input_path).map_err(|error| {
        BayesCliError::Input(format!("input path is not serializable: {error}"))
    })?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let artifact = ArtifactRef::from_bytes(INPUT_KIND, &input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let parsed = read_patients(&input_bytes)?;
    let requirements = preflight(
        &parsed,
        &group_a,
        &group_b,
        permutations,
        alpha,
        maximum_patients,
        maximum_endpoints,
        maximum_patient_endpoint_cells,
        maximum_permutation_endpoint_evaluations,
    )?;
    let retained = retained_bytes(
        input_bytes.len(),
        requirements.patient_count,
        requirements.endpoint_names.len(),
        permutations,
    )?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "cohort Max-T retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    if source_artifact(&input_path, INPUT_KIND)? != artifact {
        return Err(BayesCliError::Input(
            "cohort Max-T input changed while prepared".into(),
        ));
    }
    drop(input_bytes);

    let node = CohortMaxTProjectNode::new(
        input_path,
        input_path_json,
        artifact.clone(),
        parsed,
        MaxTPermutationSpec {
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
        },
        step_down,
        requirements,
        maximum_patients,
        maximum_endpoints,
        maximum_patient_endpoint_cells,
        maximum_permutation_endpoint_evaluations,
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
        .register_reference(artifact)
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
        ArtifactSchema::new("marklab.cohort_max_t", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project cohort-max-t cache_status={cache_status}");
    Ok(())
}

fn read_bounded(path: &std::path::Path, memory_bytes: usize) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(BayesCliError::Input(format!(
            "cohort Max-T input must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Debug, Deserialize)]
struct EndpointCsvRecord {
    patient_id: String,
    group: String,
    endpoint: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

#[derive(Clone)]
struct MaxTInput {
    patients: Vec<PatientEndpointVector>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

fn read_patients(bytes: &[u8]) -> Result<MaxTInput, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    let headers = reader.headers()?.clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "endpoint", "value", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "endpoint", "value"])
    {
        return Err(BayesCliError::Input(
            "CSV header must be exactly patient_id,group,endpoint,value or patient_id,group,endpoint,value,block"
                .into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<EndpointCsvRecord>() {
        let row = decoded.map_err(|error| BayesCliError::Input(error.to_string()))?;
        if blocked && row.block.is_none() {
            return Err(BayesCliError::Input(format!(
                "patient {} is missing its Max-T block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), BTreeMap::new()));
        if entry.0 != row.group {
            return Err(BayesCliError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(BayesCliError::Input(format!(
                "patient {} has conflicting Max-T blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.endpoint.clone(), row.value).is_some() {
            return Err(BayesCliError::Input(format!(
                "patient {} has duplicate endpoint {:?}",
                row.patient_id, row.endpoint
            )));
        }
    }
    let mut patients = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, endpoints)) in grouped {
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
            );
        }
        patients.push(PatientEndpointVector {
            patient_id,
            group,
            values: endpoints.values().copied().collect(),
            endpoints: endpoints.into_keys().collect(),
        });
    }
    Ok(MaxTInput { patients, blocks })
}

#[derive(Clone)]
struct Requirements {
    patient_count: usize,
    endpoint_names: Vec<String>,
    group_a_count: usize,
    group_b_count: usize,
    blocked: bool,
    block_count: Option<usize>,
    patient_endpoint_cells: u64,
    permutation_endpoint_evaluations: u64,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    input: &MaxTInput,
    group_a: &str,
    group_b: &str,
    permutations: usize,
    alpha: f64,
    maximum_patients: usize,
    maximum_endpoints: usize,
    maximum_patient_endpoint_cells: u64,
    maximum_permutation_endpoint_evaluations: u64,
) -> Result<Requirements, BayesCliError> {
    if group_a.trim().is_empty()
        || group_b.trim().is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(BayesCliError::Input(
            "group labels must be non-empty, exact, and distinct".into(),
        ));
    }
    if permutations == 0 || permutations > 1_000_000 {
        return Err(BayesCliError::Input(
            "permutations must be between 1 and 1000000".into(),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(BayesCliError::Input(
            "Max-T alpha must be finite and strictly between zero and one".into(),
        ));
    }
    let patients = &input.patients;
    if patients.is_empty() || patients.len() > 1_000_000 {
        return Err(BayesCliError::Input(
            "Max-T requires 1 to 1000000 patient vectors".into(),
        ));
    }
    if patients.len() > maximum_patients {
        return Err(BayesCliError::Input(format!(
            "cohort Max-T patient count exceeds maximum_patients {maximum_patients}"
        )));
    }
    let endpoints = &patients[0].endpoints;
    if endpoints.is_empty()
        || endpoints.len() > maximum_endpoints
        || endpoints.iter().any(|endpoint| endpoint.trim().is_empty())
        || endpoints.iter().collect::<HashSet<_>>().len() != endpoints.len()
    {
        return Err(BayesCliError::Input(format!(
            "cohort Max-T endpoints must be unique, complete, nonempty, and within maximum_endpoints {maximum_endpoints}"
        )));
    }
    let mut patient_ids = HashSet::new();
    let mut group_a_count = 0;
    let mut group_b_count = 0;
    for patient in patients {
        if patient.patient_id.trim().is_empty()
            || patient.patient_id.trim() != patient.patient_id
            || !patient_ids.insert(patient.patient_id.as_str())
            || patient.endpoints != *endpoints
            || patient.values.len() != endpoints.len()
            || patient.values.iter().any(|value| !value.is_finite())
        {
            return Err(BayesCliError::Input(
                "cohort Max-T vectors require unique exact patient IDs and complete finite endpoint families"
                    .into(),
            ));
        }
        if patient.group == group_a {
            group_a_count += 1;
        } else if patient.group == group_b {
            group_b_count += 1;
        } else {
            return Err(BayesCliError::Input(format!(
                "patient {} has undeclared group {:?}",
                patient.patient_id, patient.group
            )));
        }
    }
    if group_a_count < 2 || group_b_count < 2 {
        return Err(BayesCliError::Input(
            "each group must contain at least two patients".into(),
        ));
    }
    let block_count = if let Some(assignments) = &input.blocks {
        if assignments.len() != patients.len() {
            return Err(BayesCliError::Input(
                "blocked endpoint design requires exactly one assignment per patient".into(),
            ));
        }
        let by_patient = assignments
            .iter()
            .map(|assignment| (assignment.patient_id(), assignment.block()))
            .collect::<BTreeMap<_, _>>();
        let mut groups_by_block = BTreeMap::<&str, (bool, bool)>::new();
        if by_patient.len() != patients.len() {
            return Err(BayesCliError::Input(
                "blocked endpoint assignments do not match exact patients".into(),
            ));
        }
        for patient in patients {
            let Some(block) = by_patient.get(patient.patient_id.as_str()).copied() else {
                return Err(BayesCliError::Input(
                    "blocked endpoint assignments do not match exact patients".into(),
                ));
            };
            let entry = groups_by_block.entry(block).or_default();
            if patient.group == group_a {
                entry.0 = true;
            } else {
                entry.1 = true;
            }
        }
        if !groups_by_block.values().any(|groups| groups.0 && groups.1) {
            return Err(BayesCliError::Input(
                "fingerprint blocks are fully confounded with group".into(),
            ));
        }
        Some(groups_by_block.len())
    } else {
        None
    };
    let patient_endpoint_cells = (patients.len() as u64)
        .checked_mul(endpoints.len() as u64)
        .ok_or_else(|| BayesCliError::Input("Max-T patient-endpoint cells overflowed".into()))?;
    if patient_endpoint_cells > maximum_patient_endpoint_cells {
        return Err(BayesCliError::Input(format!(
            "Max-T requires {patient_endpoint_cells} patient-endpoint cells, exceeding maximum_patient_endpoint_cells {maximum_patient_endpoint_cells}"
        )));
    }
    let permutation_endpoint_evaluations = patient_endpoint_cells
        .checked_mul(permutations as u64 + 1)
        .ok_or_else(|| BayesCliError::Input("Max-T permutation work overflowed".into()))?;
    if permutation_endpoint_evaluations > maximum_permutation_endpoint_evaluations {
        return Err(BayesCliError::Input(format!(
            "Max-T requires {permutation_endpoint_evaluations} patient-by-endpoint-by-permutation evaluations, exceeding maximum_permutation_endpoint_evaluations {maximum_permutation_endpoint_evaluations}"
        )));
    }
    Ok(Requirements {
        patient_count: patients.len(),
        endpoint_names: endpoints.clone(),
        group_a_count,
        group_b_count,
        blocked: input.blocks.is_some(),
        block_count,
        patient_endpoint_cells,
        permutation_endpoint_evaluations,
    })
}

fn retained_bytes(
    source_bytes: usize,
    patients: usize,
    endpoints: usize,
    permutations: usize,
) -> Result<usize, BayesCliError> {
    let analysis = source_bytes
        .checked_add(patients.saturating_mul(512 + endpoints.saturating_mul(16)))
        .and_then(|value| value.checked_add(endpoints.saturating_mul(256)))
        .ok_or_else(|| BayesCliError::Input("Max-T analysis memory overflowed".into()))?;
    analysis
        .checked_mul(2)
        .and_then(|value| value.checked_add(permutations.saturating_mul(16)))
        .and_then(|value| value.checked_add(patients.saturating_mul(128)))
        .and_then(|value| value.checked_add(endpoints.saturating_mul(256)))
        .ok_or_else(|| BayesCliError::Input("Max-T retained-memory estimate overflowed".into()))
}

struct CohortMaxTProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_path_json: Vec<u8>,
    input_artifacts: Vec<ArtifactRef>,
    input: MaxTInput,
    analysis: MaxTPermutationSpec,
    step_down: bool,
    requirements: Requirements,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl CohortMaxTProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input_path: PathBuf,
        input_path_json: Vec<u8>,
        input_artifact: ArtifactRef,
        input: MaxTInput,
        analysis: MaxTPermutationSpec,
        step_down: bool,
        requirements: Requirements,
        maximum_patients: usize,
        maximum_endpoints: usize,
        maximum_patient_endpoint_cells: u64,
        maximum_permutation_endpoint_evaluations: u64,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let correction = if step_down {
            "step_down_max_t"
        } else {
            "single_step_max_t"
        };
        let alpha_bits = analysis.alpha.to_bits().to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-max-t-configuration-v1".as_slice(),
            input_path_json.as_slice(),
            analysis.group_a.as_bytes(),
            analysis.group_b.as_bytes(),
            analysis.permutations.to_string().as_bytes(),
            analysis.seed.to_string().as_bytes(),
            alpha_bits.as_bytes(),
            correction.as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_endpoints.to_string().as_bytes(),
            maximum_patient_endpoint_cells.to_string().as_bytes(),
            maximum_permutation_endpoint_evaluations
                .to_string()
                .as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;whole-patient-label-permutation;optional-exact-exchangeability-blocks;complete-endpoint-family;two-sided;correction={correction};permutations={};seed={};alpha_bits={alpha_bits};maximum_patients={maximum_patients};maximum_endpoints={maximum_endpoints};maximum_patient_endpoint_cells={maximum_patient_endpoint_cells};maximum_permutation_endpoint_evaluations={maximum_permutation_endpoint_evaluations};memory_budget_mib={memory_budget_mib}",
            analysis.permutations, analysis.seed
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("cohort-max-t")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "cohort_max_t",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_path_json,
            input_artifacts: vec![input_artifact],
            input,
            analysis,
            step_down,
            requirements,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        let correction = if self.step_down {
            "step_down_max_t"
        } else {
            "single_step_max_t"
        };
        if serde_json::to_vec(&output.input).ok().as_deref()
            != Some(self.input_path_json.as_slice())
            || output.format != "marklab.cohort_max_t"
            || output.version != 1
            || output.design.randomization_unit != "patient"
            || output.design.correction != correction
            || output.design.blocked != self.requirements.blocked.then_some(true)
            || output.design.null_family.as_deref()
                != self
                    .requirements
                    .blocked
                    .then_some("population_independence")
            || output.design.block_count != self.requirements.block_count
            || output.groups.group_a_patients != self.requirements.group_a_count
            || output.groups.group_b_patients != self.requirements.group_b_count
            || output.endpoints.len() != self.requirements.endpoint_names.len()
            || output.alpha.to_bits() != self.analysis.alpha.to_bits()
            || !output.critical_value.is_finite()
            || output.critical_value < 0.0
            || output.permutations.requested != self.analysis.permutations
            || output.permutations.attempted != self.analysis.permutations
            || output.permutations.completed != self.analysis.permutations
            || output.seed != self.analysis.seed
            || self.requirements.patient_endpoint_cells > FIXED_MAXIMUM_PATIENT_ENDPOINT_CELLS
            || self.requirements.permutation_endpoint_evaluations > FIXED_MAXIMUM_MAX_T_EVALUATIONS
        {
            return Err(invalid("decoded cohort Max-T result differs"));
        }
        for (endpoint, expected_name) in output
            .endpoints
            .iter()
            .zip(&self.requirements.endpoint_names)
        {
            if endpoint.endpoint != *expected_name
                || !endpoint.effect_group_a_minus_group_b.is_finite()
                || !endpoint.studentized_statistic.is_finite()
                || !positive_unit_interval(endpoint.adjusted_p_value)
                || !on_permutation_lattice(endpoint.adjusted_p_value, self.analysis.permutations)
            {
                return Err(invalid("decoded cohort Max-T endpoint differs"));
            }
        }
        if self.step_down {
            let mut ordered = output.endpoints.iter().collect::<Vec<_>>();
            ordered.sort_by(|left, right| {
                right
                    .studentized_statistic
                    .abs()
                    .total_cmp(&left.studentized_statistic.abs())
                    .then_with(|| left.endpoint.cmp(&right.endpoint))
            });
            if ordered
                .windows(2)
                .any(|pair| pair[0].adjusted_p_value > pair[1].adjusted_p_value)
            {
                return Err(invalid("decoded step-down Max-T p-values are not monotone"));
            }
        }
        Ok(())
    }
}

impl WorkflowNode for CohortMaxTProjectNode {
    type Output = Output;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = source_artifact(&self.input_path, INPUT_KIND).map_err(NodeError::input)?;
        if self.input_artifacts.first() != Some(&current) {
            return Err(NodeError::input(io::Error::new(
                io::ErrorKind::InvalidData,
                "cohort Max-T input identity changed",
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
        let result = match (&self.input.blocks, self.step_down) {
            (Some(blocks), true) => max_t_multiple_endpoint_blocked_step_down_permutation(
                &self.input.patients,
                blocks,
                &self.analysis,
            ),
            (Some(blocks), false) => max_t_multiple_endpoint_blocked_permutation(
                &self.input.patients,
                blocks,
                &self.analysis,
            ),
            (None, true) => {
                max_t_multiple_endpoint_step_down_permutation(&self.input.patients, &self.analysis)
            }
            (None, false) => {
                max_t_multiple_endpoint_permutation(&self.input.patients, &self.analysis)
            }
        }
        .map_err(NodeError::execution)?;
        Ok(Output::from_result(
            self.input_path.clone(),
            result,
            self.requirements.blocked,
        ))
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: Output = marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    format: String,
    version: u32,
    input: PathBuf,
    design: DesignSummary,
    groups: GroupSummary,
    endpoints: Vec<EndpointOutput>,
    alpha: f64,
    critical_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl Output {
    fn from_result(input: PathBuf, result: MaxTPermutationResult, blocked: bool) -> Self {
        Self {
            format: "marklab.cohort_max_t".into(),
            version: 1,
            input,
            design: DesignSummary {
                randomization_unit: "patient".into(),
                correction: result.correction.as_str().into(),
                blocked: blocked.then_some(true),
                null_family: blocked.then_some("population_independence".into()),
                block_count: blocked.then_some(result.inference_design.block_count()),
            },
            groups: GroupSummary {
                group_a_patients: result.group_a_count,
                group_b_patients: result.group_b_count,
            },
            endpoints: result
                .endpoints
                .into_iter()
                .map(|endpoint| EndpointOutput {
                    endpoint: endpoint.endpoint,
                    effect_group_a_minus_group_b: endpoint.effect_group_a_minus_group_b,
                    studentized_statistic: endpoint.studentized_statistic,
                    adjusted_p_value: endpoint.adjusted_p_value,
                })
                .collect(),
            alpha: result.alpha,
            critical_value: result.critical_value,
            permutations: PermutationSummary {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DesignSummary {
    randomization_unit: String,
    correction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    null_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GroupSummary {
    group_a_patients: usize,
    group_b_patients: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EndpointOutput {
    endpoint: String,
    effect_group_a_minus_group_b: f64,
    studentized_statistic: f64,
    adjusted_p_value: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PermutationSummary {
    requested: usize,
    attempted: usize,
    completed: usize,
}

fn positive_unit_interval(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value <= 1.0
}

fn on_permutation_lattice(value: f64, permutations: usize) -> bool {
    let scaled = value * (permutations + 1) as f64;
    scaled.is_finite() && scaled >= 1.0 - 1e-9 && (scaled - scaled.round()).abs() <= 1e-9
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
