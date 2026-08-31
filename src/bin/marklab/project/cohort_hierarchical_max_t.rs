use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use marklab_cohort::{
    hierarchical_gatekeeping_max_t, HierarchicalMaxTResult, MaxTCorrection, MaxTPermutationSpec,
    OrderedEndpointFamily, PatientEndpointVector,
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

const INPUT_KIND: &str =
    "application/vnd.marklab.source.hierarchical-max-t-patient-endpoints-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-hierarchical-max-t+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-hierarchical-max-t-node-v1";
const FIXED_EVALUATION_CEILING: u64 = 100_000_000;

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
    maximum_families: usize,
    maximum_endpoints: usize,
    maximum_cells: u64,
    maximum_permutation_endpoint_evaluations: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_patients < 4
        || maximum_families < 2
        || maximum_endpoints < 2
        || maximum_cells == 0
        || maximum_permutation_endpoint_evaluations == 0
        || maximum_permutation_endpoint_evaluations > FIXED_EVALUATION_CEILING
        || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "hierarchical Max-T resource limits must be positive and remain within fixed ceilings"
                .into(),
        ));
    }
    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        BayesCliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let prepared_artifact = ArtifactRef::from_bytes(INPUT_KIND, &input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let input = read_input(&input_bytes)?;
    let requirements = preflight(
        &input,
        &group_a,
        &group_b,
        permutations,
        alpha,
        maximum_patients,
        maximum_families,
        maximum_endpoints,
        maximum_cells,
        maximum_permutation_endpoint_evaluations,
    )?;
    let retained = retained_bytes(input_bytes.len(), &input, &requirements, permutations)?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "hierarchical Max-T retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    let output_estimate = output_bytes_upper_bound(&input)?;
    if output_estimate > MAXIMUM_RESULT_BYTES {
        return Err(BayesCliError::Input(format!(
            "hierarchical Max-T output estimate {output_estimate} exceeds durable result limit {MAXIMUM_RESULT_BYTES}"
        )));
    }
    let peak = retained
        .checked_add(output_estimate)
        .ok_or_else(|| BayesCliError::Input("hierarchical Max-T peak memory overflowed".into()))?;
    if peak > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "hierarchical Max-T peak-memory estimate {peak} exceeds budget {memory_bytes}"
        )));
    }
    if prepared_artifact != source_artifact(&input_path, INPUT_KIND)? {
        return Err(BayesCliError::Input(
            "hierarchical Max-T source changed while prepared".into(),
        ));
    }

    let correction = if step_down {
        MaxTCorrection::StepDown
    } else {
        MaxTCorrection::SingleStep
    };
    let analysis = Analysis {
        input_path,
        input_path_identity,
        input_artifact: prepared_artifact.clone(),
        patients: input.patients,
        families: input.families,
        spec: MaxTPermutationSpec {
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
        },
        correction,
        requirements,
    };
    let node = CohortHierarchicalMaxTNode::new(
        analysis,
        maximum_patients,
        maximum_families,
        maximum_endpoints,
        maximum_cells,
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
        .register_reference(prepared_artifact)
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
        ArtifactSchema::new("marklab.cohort_hierarchical_max_t", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project cohort-hierarchical-max-t cache_status={cache_status}");
    Ok(())
}

fn read_bounded(path: &Path, memory_bytes: usize) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(BayesCliError::Input(format!(
            "hierarchical Max-T input must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    patient_id: String,
    group: String,
    family_order: usize,
    family: String,
    endpoint: String,
    value: f64,
}

struct Input {
    patients: Vec<PatientEndpointVector>,
    families: Vec<OrderedEndpointFamily>,
}

type FamilyKey = (usize, String, String);
type PatientRows = (String, BTreeMap<FamilyKey, f64>);

fn read_input(bytes: &[u8]) -> Result<Input, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if !reader.headers()?.iter().eq([
        "patient_id",
        "group",
        "family_order",
        "family",
        "endpoint",
        "value",
    ]) {
        return Err(BayesCliError::Input(
            "CSV header must be exactly patient_id,group,family_order,family,endpoint,value".into(),
        ));
    }
    let mut family_names = BTreeMap::<usize, String>::new();
    let mut endpoint_keys = BTreeMap::<String, FamilyKey>::new();
    let mut patients = BTreeMap::<String, PatientRows>::new();
    for row in reader.deserialize::<CsvRow>() {
        let row = row?;
        if row.family_order == 0 {
            return Err(BayesCliError::Input(
                "family_order must be a contiguous one-based integer".into(),
            ));
        }
        if family_names
            .insert(row.family_order, row.family.clone())
            .is_some_and(|existing| existing != row.family)
        {
            return Err(BayesCliError::Input(format!(
                "family order {} has conflicting names",
                row.family_order
            )));
        }
        let key = (row.family_order, row.family.clone(), row.endpoint.clone());
        if endpoint_keys
            .insert(row.endpoint.clone(), key.clone())
            .is_some_and(|existing| existing != key)
        {
            return Err(BayesCliError::Input(format!(
                "endpoint {:?} belongs to conflicting families",
                row.endpoint
            )));
        }
        let patient = patients
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), BTreeMap::new()));
        if patient.0 != row.group {
            return Err(BayesCliError::Input(format!(
                "patient {} has conflicting group values",
                row.patient_id
            )));
        }
        if patient.1.insert(key, row.value).is_some() {
            return Err(BayesCliError::Input(format!(
                "patient {} has a duplicate hierarchical endpoint",
                row.patient_id
            )));
        }
    }
    if family_names.keys().copied().ne(1..=family_names.len()) {
        return Err(BayesCliError::Input(
            "family_order must be contiguous from one".into(),
        ));
    }
    let mut families = Vec::with_capacity(family_names.len());
    for (order, family) in family_names {
        let endpoints = endpoint_keys
            .values()
            .filter(|(endpoint_order, endpoint_family, _)| {
                *endpoint_order == order && endpoint_family == &family
            })
            .map(|(_, _, endpoint)| endpoint.clone())
            .collect::<Vec<_>>();
        families.push(OrderedEndpointFamily { family, endpoints });
    }
    let expected_keys = families
        .iter()
        .enumerate()
        .flat_map(|(order, family)| {
            family
                .endpoints
                .iter()
                .map(move |endpoint| (order + 1, family.family.clone(), endpoint.clone()))
        })
        .collect::<Vec<_>>();
    let patients = patients
        .into_iter()
        .map(|(patient_id, (group, values))| {
            if values.keys().ne(expected_keys.iter()) {
                return Err(BayesCliError::Input(format!(
                    "patient {patient_id} does not have the exact complete ordered endpoint families"
                )));
            }
            Ok(PatientEndpointVector {
                patient_id,
                group,
                endpoints: expected_keys
                    .iter()
                    .map(|(_, _, endpoint)| endpoint.clone())
                    .collect(),
                values: values.into_values().collect(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Input { patients, families })
}

#[derive(Clone, Copy)]
struct Requirements {
    patient_count: usize,
    family_count: usize,
    endpoint_count: usize,
    group_a_count: usize,
    group_b_count: usize,
    cells: u64,
    evaluations: u64,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    input: &Input,
    group_a: &str,
    group_b: &str,
    permutations: usize,
    alpha: f64,
    maximum_patients: usize,
    maximum_families: usize,
    maximum_endpoints: usize,
    maximum_cells: u64,
    maximum_evaluations: u64,
) -> Result<Requirements, BayesCliError> {
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(BayesCliError::Input(
            "hierarchical Max-T group labels must be distinct exact nonempty values".into(),
        ));
    }
    if permutations == 0 || permutations > 1_000_000 {
        return Err(BayesCliError::Input(
            "hierarchical Max-T permutations must be between 1 and 1000000".into(),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(BayesCliError::Input(
            "hierarchical Max-T alpha must be finite and strictly between zero and one".into(),
        ));
    }
    if input.patients.len() < 4 || input.patients.len() > maximum_patients {
        return Err(BayesCliError::Input(format!(
            "patient count is outside 4..=maximum_patients {maximum_patients}"
        )));
    }
    if input.families.len() < 2 || input.families.len() > maximum_families {
        return Err(BayesCliError::Input(format!(
            "family count is outside 2..=maximum_families {maximum_families}"
        )));
    }
    let endpoints = &input.patients[0].endpoints;
    if endpoints.len() < 2
        || endpoints.len() > maximum_endpoints
        || endpoints.iter().any(|endpoint| endpoint.trim().is_empty())
        || endpoints.iter().collect::<HashSet<_>>().len() != endpoints.len()
    {
        return Err(BayesCliError::Input(format!(
            "endpoint count or identities exceed maximum_endpoints {maximum_endpoints}"
        )));
    }
    let mut family_names = HashSet::new();
    if input.families.iter().any(|family| {
        family.family.is_empty()
            || family.family.trim() != family.family
            || !family_names.insert(family.family.as_str())
            || family.endpoints.is_empty()
    }) {
        return Err(BayesCliError::Input(
            "ordered endpoint families require exact unique names and nonempty members".into(),
        ));
    }
    let flattened = input
        .families
        .iter()
        .flat_map(|family| family.endpoints.iter())
        .collect::<Vec<_>>();
    if flattened.iter().copied().ne(endpoints.iter()) {
        return Err(BayesCliError::Input(
            "ordered endpoint families must exactly partition the complete endpoint vector".into(),
        ));
    }
    let mut patient_ids = HashSet::new();
    let mut group_a_count = 0;
    let mut group_b_count = 0;
    for patient in &input.patients {
        if patient.patient_id.is_empty()
            || patient.patient_id.trim() != patient.patient_id
            || !patient_ids.insert(patient.patient_id.as_str())
            || patient.endpoints != *endpoints
            || patient.values.len() != endpoints.len()
            || patient.values.iter().any(|value| !value.is_finite())
        {
            return Err(BayesCliError::Input(
                "hierarchical Max-T requires unique exact patients and complete finite endpoint vectors"
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
            "each hierarchical Max-T group requires at least two patients".into(),
        ));
    }
    let cells = (input.patients.len() as u64)
        .checked_mul(endpoints.len() as u64)
        .ok_or_else(|| BayesCliError::Input("hierarchical Max-T cell count overflowed".into()))?;
    if cells > maximum_cells {
        return Err(BayesCliError::Input(format!(
            "{cells} patient-endpoint cells exceed maximum_cells {maximum_cells}"
        )));
    }
    let evaluations = cells
        .checked_mul(permutations as u64 + 1)
        .ok_or_else(|| BayesCliError::Input("hierarchical Max-T work overflowed".into()))?;
    if evaluations > maximum_evaluations {
        return Err(BayesCliError::Input(format!(
            "{evaluations} patient-endpoint-permutation evaluations exceed maximum_permutation_endpoint_evaluations {maximum_evaluations}"
        )));
    }
    Ok(Requirements {
        patient_count: input.patients.len(),
        family_count: input.families.len(),
        endpoint_count: endpoints.len(),
        group_a_count,
        group_b_count,
        cells,
        evaluations,
    })
}

fn retained_bytes(
    source_bytes: usize,
    input: &Input,
    requirements: &Requirements,
    permutations: usize,
) -> Result<usize, BayesCliError> {
    let cell_bytes = usize::try_from(requirements.cells)
        .ok()
        .and_then(|cells| cells.checked_mul(3 * std::mem::size_of::<f64>()))
        .ok_or_else(|| BayesCliError::Input("hierarchical Max-T cell memory overflowed".into()))?;
    source_bytes
        .checked_add(cell_bytes)
        .and_then(|value| value.checked_add(requirements.patient_count.saturating_mul(1024)))
        .and_then(|value| value.checked_add(requirements.endpoint_count.saturating_mul(1024)))
        .and_then(|value| value.checked_add(requirements.family_count.saturating_mul(1024)))
        .and_then(|value| value.checked_add(permutations.saturating_mul(16)))
        .and_then(|value| {
            value.checked_add(
                input
                    .families
                    .iter()
                    .map(|family| family.endpoints.len())
                    .max()
                    .unwrap_or(0)
                    .saturating_mul(requirements.patient_count)
                    .saturating_mul(32),
            )
        })
        .ok_or_else(|| BayesCliError::Input("hierarchical Max-T memory estimate overflowed".into()))
}

fn output_bytes_upper_bound(input: &Input) -> Result<usize, BayesCliError> {
    let family_name_bytes = input
        .families
        .iter()
        .try_fold(0_usize, |total, family| {
            total.checked_add(family.family.len())
        })
        .ok_or_else(|| BayesCliError::Input("family identity size overflowed".into()))?;
    let endpoint_name_bytes = input
        .families
        .iter()
        .try_fold(0_usize, |total, family| {
            family
                .endpoints
                .iter()
                .try_fold(total, |total, endpoint| total.checked_add(endpoint.len()))
        })
        .ok_or_else(|| BayesCliError::Input("endpoint identity size overflowed".into()))?;
    (32 * 1024_usize)
        .checked_add(input.families.len().saturating_mul(512))
        .and_then(|value| value.checked_add(family_name_bytes))
        .and_then(|value| {
            value.checked_add(
                input
                    .families
                    .iter()
                    .map(|family| family.endpoints.len())
                    .sum::<usize>()
                    .saturating_mul(384),
            )
        })
        .and_then(|value| value.checked_add(endpoint_name_bytes))
        .ok_or_else(|| BayesCliError::Input("hierarchical Max-T output estimate overflowed".into()))
}

struct Analysis {
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    input_artifact: ArtifactRef,
    patients: Vec<PatientEndpointVector>,
    families: Vec<OrderedEndpointFamily>,
    spec: MaxTPermutationSpec,
    correction: MaxTCorrection,
    requirements: Requirements,
}

struct CohortHierarchicalMaxTNode {
    node_spec: NodeSpec,
    analysis: Analysis,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl CohortHierarchicalMaxTNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        analysis: Analysis,
        maximum_patients: usize,
        maximum_families: usize,
        maximum_endpoints: usize,
        maximum_cells: u64,
        maximum_evaluations: u64,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-hierarchical-max-t-configuration-v1".as_slice(),
            analysis.input_path_identity.as_slice(),
            analysis.spec.group_a.as_bytes(),
            analysis.spec.group_b.as_bytes(),
            analysis.spec.permutations.to_string().as_bytes(),
            analysis.spec.seed.to_string().as_bytes(),
            analysis.spec.alpha.to_bits().to_string().as_bytes(),
            correction_name(analysis.correction).as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_families.to_string().as_bytes(),
            maximum_endpoints.to_string().as_bytes(),
            maximum_cells.to_string().as_bytes(),
            maximum_evaluations.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;patient-label-permutation;ordered-family-gatekeeping-max-t;correction={};maximum_patients={maximum_patients};maximum_families={maximum_families};maximum_endpoints={maximum_endpoints};maximum_cells={maximum_cells};maximum_permutation_endpoint_evaluations={maximum_evaluations};memory_budget_mib={memory_budget_mib}",
            correction_name(analysis.correction)
        )
        .into_bytes();
        Ok(Self {
            node_spec: NodeSpec::new(
                NodeId::new("cohort-hierarchical-max-t")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "cohort_hierarchical_max_t",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            analysis,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.node_spec
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        let requirements = &self.analysis.requirements;
        if output.format != "marklab.cohort_hierarchical_max_t"
            || output.version != 1
            || output.design.analysis_level != "patient"
            || output.design.null_family != "population_independence"
            || output.design.permutation_unit != "patient_label"
            || output.design.multiplicity != "ordered_family_gatekeeping_max_t"
            || output.design.correction != correction_name(self.analysis.correction)
            || output.design.opening_rule
                != "open the next family only when every endpoint in the current opened family is rejected"
            || output.patients.total != requirements.patient_count
            || output.patients.group_a != requirements.group_a_count
            || output.patients.group_b != requirements.group_b_count
            || output.families.len() != requirements.family_count
            || output.opened_family_count
                != output.families.iter().filter(|family| family.opened).count()
            || output.alpha.to_bits() != self.analysis.spec.alpha.to_bits()
            || output.permutations.requested != self.analysis.spec.permutations
            || output.permutations.attempted != self.analysis.spec.permutations
            || output.permutations.completed != self.analysis.spec.permutations
            || output.permutations.seed != self.analysis.spec.seed
            || output.claim_status != "strong_fwer_prespecified_serial_gatekeeping"
            || requirements.evaluations > FIXED_EVALUATION_CEILING
        {
            return Err(invalid("decoded hierarchical Max-T identity differs"));
        }
        let minimum_p = 1.0 / (self.analysis.spec.permutations + 1) as f64;
        let mut should_open = true;
        for (order, (family, expected)) in output
            .families
            .iter()
            .zip(&self.analysis.families)
            .enumerate()
        {
            if family.order != order
                || family.family != expected.family
                || family.opened != should_open
                || !family.critical_value.is_finite()
                || family.critical_value < 0.0
                || family.endpoints.len() != expected.endpoints.len()
            {
                return Err(invalid("decoded hierarchical Max-T family differs"));
            }
            for (endpoint, expected_name) in family.endpoints.iter().zip(&expected.endpoints) {
                let lattice =
                    endpoint.local_adjusted_p_value * (self.analysis.spec.permutations + 1) as f64;
                if &endpoint.endpoint != expected_name
                    || !endpoint.effect_group_a_minus_group_b.is_finite()
                    || !endpoint.studentized_statistic.is_finite()
                    || !(minimum_p..=1.0).contains(&endpoint.local_adjusted_p_value)
                    || (lattice - lattice.round()).abs() > 1e-9
                    || endpoint.rejected
                        != (family.opened
                            && endpoint.local_adjusted_p_value <= self.analysis.spec.alpha)
                {
                    return Err(invalid("decoded hierarchical Max-T endpoint differs"));
                }
            }
            let all_rejected =
                family.opened && family.endpoints.iter().all(|endpoint| endpoint.rejected);
            if family.all_endpoints_rejected != all_rejected {
                return Err(invalid("decoded hierarchical Max-T gate state differs"));
            }
            should_open = all_rejected;
        }
        Ok(())
    }
}

impl WorkflowNode for CohortHierarchicalMaxTNode {
    type Output = Output;

    fn spec(&self) -> &NodeSpec {
        &self.node_spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        std::slice::from_ref(&self.analysis.input_artifact)
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current =
            source_artifact(&self.analysis.input_path, INPUT_KIND).map_err(NodeError::input)?;
        if current != self.analysis.input_artifact {
            return Err(NodeError::input(invalid(
                "hierarchical Max-T source identity changed",
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
        let result = hierarchical_gatekeeping_max_t(
            &self.analysis.patients,
            &self.analysis.families,
            &self.analysis.spec,
            self.analysis.correction,
        )
        .map_err(NodeError::execution)?;
        Ok(Output::from_result(result))
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
    design: Design,
    patients: PatientCounts,
    families: Vec<FamilyOutput>,
    opened_family_count: usize,
    alpha: f64,
    permutations: Permutations,
    claim_status: String,
}

impl Output {
    fn from_result(result: HierarchicalMaxTResult) -> Self {
        Self {
            format: "marklab.cohort_hierarchical_max_t".into(),
            version: 1,
            design: Design {
                analysis_level: "patient".into(),
                null_family: "population_independence".into(),
                permutation_unit: "patient_label".into(),
                multiplicity: "ordered_family_gatekeeping_max_t".into(),
                correction: result.correction.as_str().into(),
                opening_rule: "open the next family only when every endpoint in the current opened family is rejected".into(),
            },
            patients: PatientCounts {
                total: result.group_a_count + result.group_b_count,
                group_a: result.group_a_count,
                group_b: result.group_b_count,
            },
            families: result
                .families
                .into_iter()
                .map(|family| FamilyOutput {
                    order: family.order,
                    family: family.family,
                    opened: family.opened,
                    all_endpoints_rejected: family.all_endpoints_rejected,
                    critical_value: family.critical_value,
                    endpoints: family
                        .endpoints
                        .into_iter()
                        .map(|endpoint| EndpointOutput {
                            endpoint: endpoint.endpoint,
                            effect_group_a_minus_group_b: endpoint.effect_group_a_minus_group_b,
                            studentized_statistic: endpoint.studentized_statistic,
                            local_adjusted_p_value: endpoint.local_adjusted_p_value,
                            rejected: endpoint.rejected,
                        })
                        .collect(),
                })
                .collect(),
            opened_family_count: result.opened_family_count,
            alpha: result.alpha,
            permutations: Permutations {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
                seed: result.seed,
            },
            claim_status: "strong_fwer_prespecified_serial_gatekeeping".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Design {
    analysis_level: String,
    null_family: String,
    permutation_unit: String,
    multiplicity: String,
    correction: String,
    opening_rule: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatientCounts {
    total: usize,
    group_a: usize,
    group_b: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyOutput {
    order: usize,
    family: String,
    opened: bool,
    all_endpoints_rejected: bool,
    critical_value: f64,
    endpoints: Vec<EndpointOutput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EndpointOutput {
    endpoint: String,
    effect_group_a_minus_group_b: f64,
    studentized_statistic: f64,
    local_adjusted_p_value: f64,
    rejected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Permutations {
    requested: usize,
    attempted: usize,
    completed: usize,
    seed: u64,
}

fn correction_name(value: MaxTCorrection) -> &'static str {
    value.as_str()
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
