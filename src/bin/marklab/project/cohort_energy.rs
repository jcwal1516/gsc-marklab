use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use marklab_cohort::{
    patient_level_blocked_energy_distance, patient_level_energy_distance, EnergyDistanceResult,
    EnergyDistanceSpec, EnergyMetric, Fingerprint, PatientExchangeabilityBlock,
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

const INPUT_KIND: &str = "application/vnd.marklab.source.patient-fingerprints-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-energy+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-energy-node-v1";
const FIXED_DISTANCE_ELEMENTS: u64 = 25_000_000;
const FIXED_ENERGY_EVALUATIONS: u64 = 100_000_000;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    group_a: String,
    group_b: String,
    metric: String,
    permutations: usize,
    seed: u64,
    maximum_patients: usize,
    maximum_features: usize,
    maximum_distance_elements: u64,
    maximum_energy_evaluations: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_patients < 4
        || maximum_features == 0
        || maximum_distance_elements == 0
        || maximum_distance_elements > FIXED_DISTANCE_ELEMENTS
        || maximum_energy_evaluations == 0
        || maximum_energy_evaluations > FIXED_ENERGY_EVALUATIONS
        || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "cohort energy resource limits must be positive and remain within fixed ceilings"
                .into(),
        ));
    }
    let metric_kind = parse_metric(&metric)?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let prepared_artifact = ArtifactRef::from_bytes(INPUT_KIND, &input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let parsed = read_fingerprints(&input_bytes)?;
    let requirements = preflight(
        &parsed,
        &group_a,
        &group_b,
        permutations,
        maximum_patients,
        maximum_features,
        maximum_distance_elements,
        maximum_energy_evaluations,
    )?;
    let retained = retained_bytes(input_bytes.len(), &parsed, &requirements)?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "cohort energy retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    let current_artifact = source_artifact(&input_path, INPUT_KIND)?;
    if prepared_artifact != current_artifact {
        return Err(BayesCliError::Input(
            "cohort energy source changed while prepared".into(),
        ));
    }

    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        BayesCliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let analysis = EnergyAnalysis {
        input_path,
        input_path_identity,
        input_artifact: prepared_artifact.clone(),
        fingerprints: parsed.fingerprints,
        blocks: parsed.blocks,
        spec: EnergyDistanceSpec {
            group_a,
            group_b,
            metric: metric_kind,
            permutations,
            seed,
        },
        requirements,
    };
    let node = CohortEnergyNode::new(
        analysis,
        metric,
        maximum_patients,
        maximum_features,
        maximum_distance_elements,
        maximum_energy_evaluations,
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
        ArtifactSchema::new("marklab.cohort_energy", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project cohort-energy cache_status={cache_status}");
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
            "cohort energy input must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Debug, Deserialize)]
struct FingerprintCsvRecord {
    patient_id: String,
    group: String,
    feature: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

struct ParsedFingerprints {
    fingerprints: Vec<Fingerprint>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

fn read_fingerprints(bytes: &[u8]) -> Result<ParsedFingerprints, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "feature", "value", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "feature", "value"])
    {
        return Err(BayesCliError::Input(
            "CSV header must be exactly patient_id,group,feature,value or patient_id,group,feature,value,block"
                .into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<FingerprintCsvRecord>() {
        let row = decoded?;
        if blocked && row.block.is_none() {
            return Err(BayesCliError::Input(format!(
                "patient {} is missing its fingerprint block",
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
                "patient {} has conflicting fingerprint blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.feature.clone(), row.value).is_some() {
            return Err(BayesCliError::Input(format!(
                "patient {} has duplicate feature {:?}",
                row.patient_id, row.feature
            )));
        }
    }
    let mut fingerprints = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, features)) in grouped {
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
            );
        }
        fingerprints.push(Fingerprint {
            patient_id,
            group,
            values: features.values().copied().collect(),
            features: features.into_keys().collect(),
        });
    }
    Ok(ParsedFingerprints {
        fingerprints,
        blocks,
    })
}

#[derive(Clone, Copy)]
struct Requirements {
    patient_count: usize,
    feature_count: usize,
    group_a_count: usize,
    group_b_count: usize,
    blocked: bool,
    block_count: usize,
    distance_elements: u64,
    energy_evaluations: u64,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    parsed: &ParsedFingerprints,
    group_a: &str,
    group_b: &str,
    permutations: usize,
    maximum_patients: usize,
    maximum_features: usize,
    maximum_distance_elements: u64,
    maximum_energy_evaluations: u64,
) -> Result<Requirements, BayesCliError> {
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(BayesCliError::Input(
            "energy group labels must be distinct exact nonempty values".into(),
        ));
    }
    if permutations == 0 || permutations > 1_000_000 {
        return Err(BayesCliError::Input(
            "energy permutations must be between 1 and 1000000".into(),
        ));
    }
    let fingerprints = &parsed.fingerprints;
    if fingerprints.len() < 4 || fingerprints.len() > maximum_patients {
        return Err(BayesCliError::Input(format!(
            "fingerprint count is outside 4..=maximum_patients {maximum_patients}"
        )));
    }
    let features = &fingerprints[0].features;
    if features.is_empty()
        || features.len() > maximum_features
        || features.iter().any(|feature| feature.trim().is_empty())
        || features.iter().collect::<HashSet<_>>().len() != features.len()
    {
        return Err(BayesCliError::Input(format!(
            "fingerprint features are invalid or exceed maximum_features {maximum_features}"
        )));
    }
    let mut patient_ids = HashSet::with_capacity(fingerprints.len());
    let mut group_a_count = 0;
    let mut group_b_count = 0;
    for fingerprint in fingerprints {
        if fingerprint.patient_id.is_empty()
            || fingerprint.patient_id.trim() != fingerprint.patient_id
            || !patient_ids.insert(fingerprint.patient_id.as_str())
            || fingerprint.features != *features
            || fingerprint.values.len() != features.len()
            || fingerprint.values.iter().any(|value| !value.is_finite())
        {
            return Err(BayesCliError::Input(
                "fingerprints require unique exact patients and complete finite shared features"
                    .into(),
            ));
        }
        if fingerprint.group == group_a {
            group_a_count += 1;
        } else if fingerprint.group == group_b {
            group_b_count += 1;
        } else {
            return Err(BayesCliError::Input(format!(
                "patient {} has undeclared group {:?}",
                fingerprint.patient_id, fingerprint.group
            )));
        }
    }
    if group_a_count < 2 || group_b_count < 2 {
        return Err(BayesCliError::Input(
            "each energy group requires at least two patient fingerprints".into(),
        ));
    }

    let (blocked, block_count) = match &parsed.blocks {
        Some(blocks) => {
            let by_patient = blocks
                .iter()
                .map(|assignment| (assignment.patient_id(), assignment.block()))
                .collect::<BTreeMap<_, _>>();
            if by_patient.len() != fingerprints.len() {
                return Err(BayesCliError::Input(
                    "blocked energy requires exactly one block per patient".into(),
                ));
            }
            let mut labels = BTreeMap::<&str, (bool, bool)>::new();
            for fingerprint in fingerprints {
                let block = by_patient
                    .get(fingerprint.patient_id.as_str())
                    .ok_or_else(|| {
                        BayesCliError::Input(format!(
                            "missing fingerprint block for patient {}",
                            fingerprint.patient_id
                        ))
                    })?;
                let groups = labels.entry(*block).or_default();
                groups.0 |= fingerprint.group == group_a;
                groups.1 |= fingerprint.group == group_b;
            }
            if !labels.values().any(|(has_a, has_b)| *has_a && *has_b) {
                return Err(BayesCliError::Input(
                    "fingerprint blocks are fully confounded with group".into(),
                ));
            }
            (true, labels.len())
        }
        None => (false, 0),
    };
    let patient_count = fingerprints.len();
    let distance_elements = (patient_count as u64)
        .checked_mul(patient_count as u64)
        .ok_or_else(|| BayesCliError::Input("energy distance element count overflowed".into()))?;
    if distance_elements > maximum_distance_elements {
        return Err(BayesCliError::Input(format!(
            "{distance_elements} energy distance elements exceed maximum_distance_elements {maximum_distance_elements}"
        )));
    }
    let energy_evaluations = distance_elements
        .checked_mul(permutations as u64 + 1)
        .ok_or_else(|| BayesCliError::Input("energy evaluation count overflowed".into()))?;
    if energy_evaluations > maximum_energy_evaluations {
        return Err(BayesCliError::Input(format!(
            "{energy_evaluations} energy evaluations exceed maximum_energy_evaluations {maximum_energy_evaluations}"
        )));
    }
    Ok(Requirements {
        patient_count,
        feature_count: features.len(),
        group_a_count,
        group_b_count,
        blocked,
        block_count,
        distance_elements,
        energy_evaluations,
    })
}

fn retained_bytes(
    source_bytes: usize,
    parsed: &ParsedFingerprints,
    requirements: &Requirements,
) -> Result<usize, BayesCliError> {
    let values = requirements
        .patient_count
        .checked_mul(requirements.feature_count)
        .and_then(|value| value.checked_mul(2 * std::mem::size_of::<f64>()))
        .ok_or_else(|| BayesCliError::Input("energy fingerprint memory overflowed".into()))?;
    let matrix = usize::try_from(requirements.distance_elements)
        .ok()
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| BayesCliError::Input("energy distance memory overflowed".into()))?;
    source_bytes
        .checked_add(values)
        .and_then(|value| value.checked_add(matrix))
        .and_then(|value| value.checked_add(requirements.patient_count.saturating_mul(1024)))
        .and_then(|value| value.checked_add(requirements.feature_count.saturating_mul(256)))
        .and_then(|value| {
            value.checked_add(
                parsed
                    .blocks
                    .as_ref()
                    .map_or(0, |blocks| blocks.len().saturating_mul(256)),
            )
        })
        .and_then(|value| value.checked_add(32 * 1024))
        .ok_or_else(|| BayesCliError::Input("energy retained-memory estimate overflowed".into()))
}

struct EnergyAnalysis {
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    input_artifact: ArtifactRef,
    fingerprints: Vec<Fingerprint>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
    spec: EnergyDistanceSpec,
    requirements: Requirements,
}

struct CohortEnergyNode {
    node_spec: NodeSpec,
    analysis: EnergyAnalysis,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl CohortEnergyNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        analysis: EnergyAnalysis,
        metric: String,
        maximum_patients: usize,
        maximum_features: usize,
        maximum_distance_elements: u64,
        maximum_energy_evaluations: u64,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let blocked = analysis.requirements.blocked.to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-energy-configuration-v1".as_slice(),
            analysis.input_path_identity.as_slice(),
            analysis.spec.group_a.as_bytes(),
            analysis.spec.group_b.as_bytes(),
            metric.as_bytes(),
            analysis.spec.permutations.to_string().as_bytes(),
            analysis.spec.seed.to_string().as_bytes(),
            blocked.as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_features.to_string().as_bytes(),
            maximum_distance_elements.to_string().as_bytes(),
            maximum_energy_evaluations.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;patient-population-unit;group-label-permutation;metric={metric};blocked={blocked};maximum_patients={maximum_patients};maximum_features={maximum_features};maximum_distance_elements={maximum_distance_elements};maximum_energy_evaluations={maximum_energy_evaluations};memory_budget_mib={memory_budget_mib}"
        )
        .into_bytes();
        Ok(Self {
            node_spec: NodeSpec::new(
                NodeId::new("cohort-energy")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "cohort_energy",
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
        let minimum_p = 1.0 / (self.analysis.spec.permutations + 1) as f64;
        let lattice = output.p_value * (self.analysis.spec.permutations + 1) as f64;
        let output_path_identity = serde_json::to_vec(&output.input)
            .map_err(|error| invalid_owned(format!("decoded input path is invalid: {error}")))?;
        if output.format != "marklab.cohort_energy"
            || output.version != 1
            || output_path_identity != self.analysis.input_path_identity
            || output.design.randomization_unit != "patient"
            || output.design.blocked != requirements.blocked
            || output.design.null_family.as_deref()
                != requirements.blocked.then_some("population_independence")
            || output.design.block_count != requirements.blocked.then_some(requirements.block_count)
            || output.groups.group_a_patients != requirements.group_a_count
            || output.groups.group_b_patients != requirements.group_b_count
            || output.feature_count != requirements.feature_count
            || output.metric != metric_name(self.analysis.spec.metric)
            || !output.energy_distance.is_finite()
            || !output.p_value.is_finite()
            || !(minimum_p..=1.0).contains(&output.p_value)
            || (lattice - lattice.round()).abs() > 1e-9
            || output.permutations.requested != self.analysis.spec.permutations
            || output.permutations.attempted != self.analysis.spec.permutations
            || output.permutations.completed != self.analysis.spec.permutations
            || output.seed != self.analysis.spec.seed
            || requirements.energy_evaluations > FIXED_ENERGY_EVALUATIONS
        {
            return Err(invalid("decoded cohort energy result differs"));
        }
        Ok(())
    }
}

impl WorkflowNode for CohortEnergyNode {
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
                "cohort energy source identity changed",
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
        let (result, block_count) = match &self.analysis.blocks {
            Some(blocks) => {
                let blocked = patient_level_blocked_energy_distance(
                    &self.analysis.fingerprints,
                    blocks,
                    &self.analysis.spec,
                )
                .map_err(NodeError::execution)?;
                let block_count = blocked.design().block_count();
                (blocked.into_parts().0, Some(block_count))
            }
            None => (
                patient_level_energy_distance(&self.analysis.fingerprints, &self.analysis.spec)
                    .map_err(NodeError::execution)?,
                None,
            ),
        };
        Ok(Output::from_result(
            self.analysis.input_path.clone(),
            result,
            block_count,
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
    design: PopulationDesignSummary,
    groups: GroupSummary,
    feature_count: usize,
    metric: String,
    energy_distance: f64,
    p_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl Output {
    fn from_result(
        input: PathBuf,
        result: EnergyDistanceResult,
        block_count: Option<usize>,
    ) -> Self {
        Self {
            format: "marklab.cohort_energy".into(),
            version: 1,
            input,
            design: PopulationDesignSummary {
                randomization_unit: "patient".into(),
                blocked: block_count.is_some(),
                null_family: block_count.map(|_| "population_independence".into()),
                block_count,
            },
            groups: GroupSummary {
                group_a_patients: result.group_a_count,
                group_b_patients: result.group_b_count,
            },
            feature_count: result.feature_count,
            metric: metric_name(result.metric).into(),
            energy_distance: result.energy_distance,
            p_value: result.p_value,
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
struct PopulationDesignSummary {
    randomization_unit: String,
    blocked: bool,
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
struct PermutationSummary {
    requested: usize,
    attempted: usize,
    completed: usize,
}

fn parse_metric(value: &str) -> Result<EnergyMetric, BayesCliError> {
    match value {
        "euclidean" => Ok(EnergyMetric::Euclidean),
        _ => Err(BayesCliError::Input(
            "energy metric must be euclidean".into(),
        )),
    }
}

fn metric_name(value: EnergyMetric) -> &'static str {
    match value {
        EnergyMetric::Euclidean => "euclidean",
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
