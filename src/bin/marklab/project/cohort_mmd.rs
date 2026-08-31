use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::PathBuf,
};

use marklab_cohort::{
    patient_level_blocked_mmd, patient_level_mmd, Fingerprint, MmdEstimator, MmdKernel,
    MmdPermutationResult, MmdPermutationSpec, PatientExchangeabilityBlock,
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
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-mmd+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-mmd-node-v1";
const FIXED_MAXIMUM_KERNEL_ELEMENTS: u64 = 25_000_000;
const FIXED_MAXIMUM_MMD_EVALUATIONS: u64 = 100_000_000;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    group_a: String,
    group_b: String,
    kernel: String,
    bandwidth: Option<f64>,
    estimator: String,
    permutations: usize,
    seed: u64,
    maximum_patients: usize,
    maximum_features: usize,
    maximum_kernel_elements: u64,
    maximum_mmd_evaluations: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_patients == 0
        || maximum_features == 0
        || maximum_kernel_elements == 0
        || maximum_mmd_evaluations == 0
        || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "cohort MMD patient, feature, kernel-work, permutation-work, and memory limits must be positive"
                .into(),
        ));
    }
    if maximum_kernel_elements > FIXED_MAXIMUM_KERNEL_ELEMENTS
        || maximum_mmd_evaluations > FIXED_MAXIMUM_MMD_EVALUATIONS
    {
        return Err(BayesCliError::Input(
            "cohort MMD declared work limits exceed the fixed production ceilings".into(),
        ));
    }
    let kernel_value = parse_kernel(&kernel, bandwidth)?;
    let estimator_value = parse_estimator(&estimator)?;
    let input_path_json = serde_json::to_vec(&input_path).map_err(|error| {
        BayesCliError::Input(format!("input path is not serializable: {error}"))
    })?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let artifact = ArtifactRef::from_bytes(INPUT_KIND, &input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let parsed = read_fingerprints(&input_bytes)?;
    let requirements = preflight(
        &parsed,
        &group_a,
        &group_b,
        kernel_value,
        permutations,
        maximum_patients,
        maximum_features,
        maximum_kernel_elements,
        maximum_mmd_evaluations,
    )?;
    let retained = retained_bytes(
        input_bytes.len(),
        requirements.patient_count,
        requirements.feature_count,
        requirements.kernel_elements,
    )?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "cohort MMD retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    if source_artifact(&input_path, INPUT_KIND)? != artifact {
        return Err(BayesCliError::Input(
            "cohort MMD input changed while prepared".into(),
        ));
    }
    drop(input_bytes);

    let node = CohortMmdProjectNode::new(
        input_path,
        input_path_json,
        artifact.clone(),
        parsed,
        MmdPermutationSpec {
            group_a,
            group_b,
            kernel: kernel_value,
            estimator: estimator_value,
            permutations,
            seed,
        },
        requirements,
        kernel,
        estimator,
        maximum_patients,
        maximum_features,
        maximum_kernel_elements,
        maximum_mmd_evaluations,
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
        ArtifactSchema::new("marklab.cohort_mmd", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project cohort-mmd cache_status={cache_status}");
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
            "cohort MMD input must be a regular file within {maximum} bytes: {}",
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

#[derive(Clone)]
struct FingerprintInput {
    fingerprints: Vec<Fingerprint>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

fn read_fingerprints(bytes: &[u8]) -> Result<FingerprintInput, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
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
        let row = decoded.map_err(|error| BayesCliError::Input(error.to_string()))?;
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
    Ok(FingerprintInput {
        fingerprints,
        blocks,
    })
}

#[derive(Clone)]
struct Requirements {
    patient_count: usize,
    feature_count: usize,
    group_a_count: usize,
    group_b_count: usize,
    blocked: bool,
    block_count: Option<usize>,
    kernel_elements: u64,
    mmd_evaluations: u64,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    input: &FingerprintInput,
    group_a: &str,
    group_b: &str,
    kernel: MmdKernel,
    permutations: usize,
    maximum_patients: usize,
    maximum_features: usize,
    maximum_kernel_elements: u64,
    maximum_mmd_evaluations: u64,
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
    if let MmdKernel::Rbf { bandwidth } = kernel {
        if !bandwidth.is_finite() || bandwidth <= 0.0 {
            return Err(BayesCliError::Input(
                "RBF bandwidth must be finite and positive".into(),
            ));
        }
    }
    let fingerprints = &input.fingerprints;
    if fingerprints.is_empty() || fingerprints.len() > 1_000_000 {
        return Err(BayesCliError::Input(
            "MMD requires 1 to 1000000 patient fingerprints".into(),
        ));
    }
    if fingerprints.len() > maximum_patients {
        return Err(BayesCliError::Input(format!(
            "cohort MMD patient count exceeds maximum_patients {maximum_patients}"
        )));
    }
    let features = &fingerprints[0].features;
    if features.is_empty()
        || features.len() > maximum_features
        || features.iter().any(|feature| feature.trim().is_empty())
        || features.iter().collect::<HashSet<_>>().len() != features.len()
    {
        return Err(BayesCliError::Input(format!(
            "cohort MMD features must be unique, complete, nonempty, and within maximum_features {maximum_features}"
        )));
    }
    let mut patient_ids = HashSet::new();
    let mut group_a_count = 0;
    let mut group_b_count = 0;
    for fingerprint in fingerprints {
        if fingerprint.patient_id.trim().is_empty()
            || fingerprint.patient_id.trim() != fingerprint.patient_id
            || !patient_ids.insert(fingerprint.patient_id.as_str())
            || fingerprint.features != *features
            || fingerprint.values.len() != features.len()
            || fingerprint.values.iter().any(|value| !value.is_finite())
        {
            return Err(BayesCliError::Input(
                "cohort MMD fingerprints require unique exact patient IDs and complete finite feature vectors"
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
            "each group must contain at least two patient fingerprints".into(),
        ));
    }
    let block_count = if let Some(assignments) = &input.blocks {
        if assignments.len() != fingerprints.len() {
            return Err(BayesCliError::Input(
                "blocked fingerprint design requires exactly one assignment per patient".into(),
            ));
        }
        let by_patient = assignments
            .iter()
            .map(|assignment| (assignment.patient_id(), assignment.block()))
            .collect::<BTreeMap<_, _>>();
        if by_patient.len() != fingerprints.len()
            || fingerprints
                .iter()
                .any(|row| !by_patient.contains_key(row.patient_id.as_str()))
        {
            return Err(BayesCliError::Input(
                "blocked fingerprint assignments do not match exact patients".into(),
            ));
        }
        let mut groups_by_block = BTreeMap::<&str, (bool, bool)>::new();
        for fingerprint in fingerprints {
            let block = by_patient[fingerprint.patient_id.as_str()];
            let entry = groups_by_block.entry(block).or_default();
            if fingerprint.group == group_a {
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
    let kernel_elements = (fingerprints.len() as u64)
        .checked_mul(fingerprints.len() as u64)
        .ok_or_else(|| BayesCliError::Input("MMD kernel matrix size overflowed".into()))?;
    if kernel_elements > maximum_kernel_elements {
        return Err(BayesCliError::Input(format!(
            "MMD kernel matrix has {kernel_elements} elements, exceeding maximum_kernel_elements {maximum_kernel_elements}"
        )));
    }
    let mmd_evaluations = kernel_elements
        .checked_mul(permutations as u64 + 1)
        .ok_or_else(|| BayesCliError::Input("MMD permutation work overflowed".into()))?;
    if mmd_evaluations > maximum_mmd_evaluations {
        return Err(BayesCliError::Input(format!(
            "MMD requires {mmd_evaluations} kernel-by-permutation evaluations, exceeding maximum_mmd_evaluations {maximum_mmd_evaluations}"
        )));
    }
    Ok(Requirements {
        patient_count: fingerprints.len(),
        feature_count: features.len(),
        group_a_count,
        group_b_count,
        blocked: input.blocks.is_some(),
        block_count,
        kernel_elements,
        mmd_evaluations,
    })
}

fn retained_bytes(
    source_bytes: usize,
    patients: usize,
    features: usize,
    kernel_elements: u64,
) -> Result<usize, BayesCliError> {
    let matrix = usize::try_from(kernel_elements)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| BayesCliError::Input("MMD kernel memory overflowed".into()))?;
    let analysis = source_bytes
        .checked_add(patients.saturating_mul(512 + features.saturating_mul(16)))
        .and_then(|value| value.checked_add(features.saturating_mul(256)))
        .ok_or_else(|| BayesCliError::Input("MMD analysis memory overflowed".into()))?;
    analysis
        .checked_mul(2)
        .and_then(|value| value.checked_add(matrix))
        .and_then(|value| value.checked_add(patients.saturating_mul(128)))
        .ok_or_else(|| BayesCliError::Input("MMD retained-memory estimate overflowed".into()))
}

struct CohortMmdProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_path_json: Vec<u8>,
    input_artifacts: Vec<ArtifactRef>,
    input: FingerprintInput,
    analysis: MmdPermutationSpec,
    requirements: Requirements,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl CohortMmdProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input_path: PathBuf,
        input_path_json: Vec<u8>,
        input_artifact: ArtifactRef,
        input: FingerprintInput,
        analysis: MmdPermutationSpec,
        requirements: Requirements,
        kernel: String,
        estimator: String,
        maximum_patients: usize,
        maximum_features: usize,
        maximum_kernel_elements: u64,
        maximum_mmd_evaluations: u64,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let bandwidth_identity = match analysis.kernel {
            MmdKernel::Linear => "none".into(),
            MmdKernel::Rbf { bandwidth } => bandwidth.to_bits().to_string(),
        };
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-mmd-configuration-v1".as_slice(),
            input_path_json.as_slice(),
            analysis.group_a.as_bytes(),
            analysis.group_b.as_bytes(),
            kernel.as_bytes(),
            bandwidth_identity.as_bytes(),
            estimator.as_bytes(),
            analysis.permutations.to_string().as_bytes(),
            analysis.seed.to_string().as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_features.to_string().as_bytes(),
            maximum_kernel_elements.to_string().as_bytes(),
            maximum_mmd_evaluations.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;patient-label-permutation;optional-exact-exchangeability-blocks;inclusive-plus-one-high-tail;kernel={kernel};bandwidth_bits={bandwidth_identity};estimator={estimator};permutations={};seed={};maximum_patients={maximum_patients};maximum_features={maximum_features};maximum_kernel_elements={maximum_kernel_elements};maximum_mmd_evaluations={maximum_mmd_evaluations};memory_budget_mib={memory_budget_mib}",
            analysis.permutations, analysis.seed
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("cohort-mmd")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "cohort_mmd",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_path_json,
            input_artifacts: vec![input_artifact],
            input,
            analysis,
            requirements,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        if serde_json::to_vec(&output.input).ok().as_deref()
            != Some(self.input_path_json.as_slice())
            || output.format != "marklab.cohort_mmd"
            || output.version != 1
            || output.design.randomization_unit != "patient"
            || output.design.blocked != self.requirements.blocked
            || output.design.null_family.as_deref()
                != self
                    .requirements
                    .blocked
                    .then_some("population_independence")
            || output.design.block_count != self.requirements.block_count
            || output.groups.group_a_patients != self.requirements.group_a_count
            || output.groups.group_b_patients != self.requirements.group_b_count
            || output.feature_count != self.requirements.feature_count
            || output.kernel.kind != kernel_name(self.analysis.kernel)
            || !same_optional_float(
                output.kernel.bandwidth,
                kernel_bandwidth(self.analysis.kernel),
            )
            || output.estimator != estimator_name(self.analysis.estimator)
            || !output.mmd_squared.is_finite()
            || !output.p_value.is_finite()
            || output.p_value <= 0.0
            || output.p_value > 1.0
            || output.permutations.requested != self.analysis.permutations
            || output.permutations.attempted != self.analysis.permutations
            || output.permutations.completed != self.analysis.permutations
            || output.seed != self.analysis.seed
            || self.requirements.kernel_elements > FIXED_MAXIMUM_KERNEL_ELEMENTS
            || self.requirements.mmd_evaluations > FIXED_MAXIMUM_MMD_EVALUATIONS
        {
            return Err(invalid("decoded cohort MMD result differs"));
        }
        let lattice = output.p_value * (self.analysis.permutations + 1) as f64;
        if !lattice.is_finite() || lattice < 1.0 - 1e-9 || (lattice - lattice.round()).abs() > 1e-9
        {
            return Err(invalid("decoded cohort MMD p-value differs"));
        }
        Ok(())
    }
}

impl WorkflowNode for CohortMmdProjectNode {
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
                "cohort MMD input identity changed",
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
        let (result, block_count) = if let Some(blocks) = &self.input.blocks {
            let blocked =
                patient_level_blocked_mmd(&self.input.fingerprints, blocks, &self.analysis)
                    .map_err(NodeError::execution)?;
            let (result, design) = blocked.into_parts();
            (result, Some(design.block_count()))
        } else {
            (
                patient_level_mmd(&self.input.fingerprints, &self.analysis)
                    .map_err(NodeError::execution)?,
                None,
            )
        };
        Ok(Output::from_result(
            self.input_path.clone(),
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
    design: DesignSummary,
    groups: GroupSummary,
    feature_count: usize,
    kernel: KernelSummary,
    estimator: String,
    mmd_squared: f64,
    p_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl Output {
    fn from_result(
        input: PathBuf,
        result: MmdPermutationResult,
        block_count: Option<usize>,
    ) -> Self {
        Self {
            format: "marklab.cohort_mmd".into(),
            version: 1,
            input,
            design: DesignSummary {
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
            kernel: KernelSummary {
                kind: kernel_name(result.kernel).into(),
                bandwidth: kernel_bandwidth(result.kernel),
            },
            estimator: estimator_name(result.estimator).into(),
            mmd_squared: result.mmd_squared,
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
struct DesignSummary {
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
struct KernelSummary {
    kind: String,
    bandwidth: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PermutationSummary {
    requested: usize,
    attempted: usize,
    completed: usize,
}

fn parse_kernel(value: &str, bandwidth: Option<f64>) -> Result<MmdKernel, BayesCliError> {
    match (value, bandwidth) {
        ("linear", None) => Ok(MmdKernel::Linear),
        ("linear", Some(_)) => Err(BayesCliError::Input(
            "linear MMD forbids --bandwidth".into(),
        )),
        ("rbf", Some(bandwidth)) => Ok(MmdKernel::Rbf { bandwidth }),
        ("rbf", None) => Err(BayesCliError::Input("RBF MMD requires --bandwidth".into())),
        _ => Err(BayesCliError::Input(
            "MMD kernel must be linear or rbf".into(),
        )),
    }
}

fn parse_estimator(value: &str) -> Result<MmdEstimator, BayesCliError> {
    match value {
        "unbiased" => Ok(MmdEstimator::Unbiased),
        "biased" => Ok(MmdEstimator::Biased),
        _ => Err(BayesCliError::Input(
            "MMD estimator must be unbiased or biased".into(),
        )),
    }
}

fn kernel_name(value: MmdKernel) -> &'static str {
    match value {
        MmdKernel::Linear => "linear",
        MmdKernel::Rbf { .. } => "rbf",
    }
}

fn kernel_bandwidth(value: MmdKernel) -> Option<f64> {
    match value {
        MmdKernel::Linear => None,
        MmdKernel::Rbf { bandwidth } => Some(bandwidth),
    }
}

fn estimator_name(value: MmdEstimator) -> &'static str {
    match value {
        MmdEstimator::Unbiased => "unbiased",
        MmdEstimator::Biased => "biased",
    }
}

fn same_optional_float(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.to_bits() == right.to_bits(),
        (None, None) => true,
        _ => false,
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
