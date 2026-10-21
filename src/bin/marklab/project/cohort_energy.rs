use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    io,
    path::{Path, PathBuf},
};

use marklab_cohort::{
    patient_level_blocked_energy_distance, patient_level_energy_distance, EnergyDistanceResult,
    EnergyDistanceSpec, EnergyMetric, Fingerprint, PatientExchangeabilityBlock,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::{CohortEnergyLimits, CohortEnergyProjectArgs};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.patient-fingerprints-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-energy+json;version=2";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-energy-node-v2";
const FIXED_DISTANCE_ELEMENTS: u64 = 25_000_000;
const FIXED_ENERGY_EVALUATIONS: u64 = 100_000_000;

pub(super) fn run(args: CohortEnergyProjectArgs) -> Result<(), CliError> {
    let CohortEnergyProjectArgs {
        project: project_path,
        input: input_path,
        group_a,
        group_b,
        metric,
        permutations,
        seed,
        limits,
        out: output_path,
    } = args;
    let CohortEnergyLimits {
        maximum_patients,
        maximum_features,
        maximum_distance_elements,
        maximum_energy_evaluations,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "cohort energy source";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_patients < 4
        || maximum_features == 0
        || maximum_distance_elements == 0
        || maximum_distance_elements > FIXED_DISTANCE_ELEMENTS
        || maximum_energy_evaluations == 0
        || maximum_energy_evaluations > FIXED_ENERGY_EVALUATIONS
        || memory_bytes == 0
    {
        return Err(CliError::Input(
            "cohort energy resource limits must be positive and remain within fixed ceilings"
                .into(),
        ));
    }
    let metric_kind = metric.into();
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let source = Source::from_bytes(input_path.clone(), INPUT_KIND, &input_bytes)?;
    let parsed = read_fingerprints(&input_bytes)?;
    let requirements = preflight(&parsed, &group_a, &group_b, permutations, limits)?;
    let retained = retained_bytes(input_bytes.len(), &parsed, &requirements)?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "cohort energy retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    source.ensure_unchanged(LABEL)?;

    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        CliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let analysis = EnergyAnalysis {
        input_path,
        input_path_identity,
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
    let contract = analysis.contract(
        maximum_patients,
        maximum_features,
        maximum_distance_elements,
        maximum_energy_evaluations,
        memory_budget_mib,
    );
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        analysis,
        EnergyAnalysis::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_energy", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish("cohort-energy", &output_path, &run.output, run.cache_status)
}

fn read_bounded(path: &Path, memory_bytes: usize) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "cohort energy input must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
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

fn read_fingerprints(bytes: &[u8]) -> Result<ParsedFingerprints, CliError> {
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
        return Err(CliError::Input(
            "CSV header must be exactly patient_id,group,feature,value or patient_id,group,feature,value,block"
                .into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<FingerprintCsvRecord>() {
        let row = decoded?;
        if blocked && row.block.is_none() {
            return Err(CliError::Input(format!(
                "patient {} is missing its fingerprint block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), BTreeMap::new()));
        if entry.0 != row.group {
            return Err(CliError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CliError::Input(format!(
                "patient {} has conflicting fingerprint blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.feature.clone(), row.value).is_some() {
            return Err(CliError::Input(format!(
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
                    .map_err(|error| CliError::Input(error.to_string()))?,
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

fn preflight(
    parsed: &ParsedFingerprints,
    group_a: &str,
    group_b: &str,
    permutations: usize,
    limits: CohortEnergyLimits,
) -> Result<Requirements, CliError> {
    let CohortEnergyLimits {
        maximum_patients,
        maximum_features,
        maximum_distance_elements,
        maximum_energy_evaluations,
        ..
    } = limits;
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(CliError::Input(
            "energy group labels must be distinct exact nonempty values".into(),
        ));
    }
    if permutations == 0 || permutations > 1_000_000 {
        return Err(CliError::Input(
            "energy permutations must be between 1 and 1000000".into(),
        ));
    }
    let fingerprints = &parsed.fingerprints;
    if fingerprints.len() < 4 || fingerprints.len() > maximum_patients {
        return Err(CliError::Input(format!(
            "fingerprint count is outside 4..=maximum_patients {maximum_patients}"
        )));
    }
    let features = &fingerprints[0].features;
    if features.is_empty()
        || features.len() > maximum_features
        || features.iter().any(|feature| feature.trim().is_empty())
        || features.iter().collect::<HashSet<_>>().len() != features.len()
    {
        return Err(CliError::Input(format!(
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
            return Err(CliError::Input(
                "fingerprints require unique exact patients and complete finite shared features"
                    .into(),
            ));
        }
        if fingerprint.group == group_a {
            group_a_count += 1;
        } else if fingerprint.group == group_b {
            group_b_count += 1;
        } else {
            return Err(CliError::Input(format!(
                "patient {} has undeclared group {:?}",
                fingerprint.patient_id, fingerprint.group
            )));
        }
    }
    if group_a_count < 2 || group_b_count < 2 {
        return Err(CliError::Input(
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
                return Err(CliError::Input(
                    "blocked energy requires exactly one block per patient".into(),
                ));
            }
            let mut labels = BTreeMap::<&str, (bool, bool)>::new();
            for fingerprint in fingerprints {
                let block = by_patient
                    .get(fingerprint.patient_id.as_str())
                    .ok_or_else(|| {
                        CliError::Input(format!(
                            "missing fingerprint block for patient {}",
                            fingerprint.patient_id
                        ))
                    })?;
                let groups = labels.entry(*block).or_default();
                groups.0 |= fingerprint.group == group_a;
                groups.1 |= fingerprint.group == group_b;
            }
            if !labels.values().any(|(has_a, has_b)| *has_a && *has_b) {
                return Err(CliError::Input(
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
        .ok_or_else(|| CliError::Input("energy distance element count overflowed".into()))?;
    if distance_elements > maximum_distance_elements {
        return Err(CliError::Input(format!(
            "{distance_elements} energy distance elements exceed maximum_distance_elements {maximum_distance_elements}"
        )));
    }
    let energy_evaluations = distance_elements
        .checked_mul(permutations as u64 + 1)
        .ok_or_else(|| CliError::Input("energy evaluation count overflowed".into()))?;
    if energy_evaluations > maximum_energy_evaluations {
        return Err(CliError::Input(format!(
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
) -> Result<usize, CliError> {
    let values = requirements
        .patient_count
        .checked_mul(requirements.feature_count)
        .and_then(|value| value.checked_mul(2 * std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("energy fingerprint memory overflowed".into()))?;
    let matrix = usize::try_from(requirements.distance_elements)
        .ok()
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("energy distance memory overflowed".into()))?;
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
        .ok_or_else(|| CliError::Input("energy retained-memory estimate overflowed".into()))
}

struct EnergyAnalysis {
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    fingerprints: Vec<Fingerprint>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
    spec: EnergyDistanceSpec,
    requirements: Requirements,
}

impl EnergyAnalysis {
    fn contract(
        &self,
        maximum_patients: usize,
        maximum_features: usize,
        maximum_distance_elements: u64,
        maximum_energy_evaluations: u64,
        memory_budget_mib: usize,
    ) -> NodeContract {
        let metric = metric_name(self.spec.metric);
        let blocked = self.requirements.blocked.to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-energy-configuration-v1".as_slice(),
            self.input_path_identity.as_slice(),
            self.spec.group_a.as_bytes(),
            self.spec.group_b.as_bytes(),
            metric.as_bytes(),
            self.spec.permutations.to_string().as_bytes(),
            self.spec.seed.to_string().as_bytes(),
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
        NodeContract {
            id: "cohort-energy",
            kind: "cohort_energy",
            version: 1,
            configuration_digest,
            execution_policy: Cow::Owned(execution_policy),
            implementation_identity: IMPLEMENTATION_IDENTITY,
            output_kind: OUTPUT_KIND,
            codec: Codec::ExactFloatJson,
        }
    }

    fn execute(&self) -> Result<Output, NodeError> {
        let (result, block_count) = match &self.blocks {
            Some(blocks) => {
                let blocked =
                    patient_level_blocked_energy_distance(&self.fingerprints, blocks, &self.spec)
                        .map_err(NodeError::execution)?;
                let block_count = blocked.design().block_count();
                (blocked.into_parts().0, Some(block_count))
            }
            None => (
                patient_level_energy_distance(&self.fingerprints, &self.spec)
                    .map_err(NodeError::execution)?,
                None,
            ),
        };
        Ok(Output::from_result(
            self.input_path.clone(),
            result,
            block_count,
        ))
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        let requirements = &self.requirements;
        let minimum_p = 1.0 / (self.spec.permutations + 1) as f64;
        let lattice = output.p_value * (self.spec.permutations + 1) as f64;
        let output_path_identity = serde_json::to_vec(&output.input)
            .map_err(|error| invalid_owned(format!("decoded input path is invalid: {error}")))?;
        if output.format != "marklab.cohort_energy"
            || output.version != 1
            || output_path_identity != self.input_path_identity
            || output.design.randomization_unit != "patient"
            || output.design.blocked != requirements.blocked
            || output.design.null_family.as_deref()
                != requirements.blocked.then_some("population_independence")
            || output.design.block_count != requirements.blocked.then_some(requirements.block_count)
            || output.groups.group_a_patients != requirements.group_a_count
            || output.groups.group_b_patients != requirements.group_b_count
            || output.feature_count != requirements.feature_count
            || output.metric != metric_name(self.spec.metric)
            || !output.energy_distance.is_finite()
            || !output.p_value.is_finite()
            || !(minimum_p..=1.0).contains(&output.p_value)
            || (lattice - lattice.round()).abs() > 1e-9
            || output.permutations != self.spec.permutations
            || output.seed != self.spec.seed
            || requirements.energy_evaluations > FIXED_ENERGY_EVALUATIONS
        {
            return Err(invalid("decoded cohort energy result differs"));
        }
        Ok(())
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
    permutations: usize,
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
            permutations: result.permutations_requested,
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
