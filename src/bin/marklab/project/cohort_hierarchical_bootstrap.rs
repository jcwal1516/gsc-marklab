use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::PathBuf,
};

use marklab_cohort::{hierarchical_bootstrap, HierarchicalBootstrapSpec, HierarchicalScalarRecord};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::super::cohort::hierarchical_bootstrap::{
    read_records_from_bytes, HierarchicalBootstrapOutput,
};
use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.hierarchical-scalar-specimens-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-hierarchical-bootstrap+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-hierarchical-bootstrap-node-v1";
const FIXED_MAXIMUM_PATIENTS: usize = 1_000_000;
const FIXED_MAXIMUM_SPECIMENS: usize = 1_000_000;
const FIXED_MAXIMUM_BOOTSTRAP_DRAWS: u64 = 100_000_000;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    replicates: usize,
    seed: u64,
    alpha: f64,
    maximum_patients: usize,
    maximum_specimens: usize,
    maximum_bootstrap_draws: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    validate_declared_limits(
        maximum_patients,
        maximum_specimens,
        maximum_bootstrap_draws,
        memory_budget_mib,
    )?;
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        BayesCliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let input_artifact = ArtifactRef::from_bytes(INPUT_KIND, &input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let records = read_records_from_bytes(&input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let requirements = preflight(
        &records,
        replicates,
        alpha,
        maximum_patients,
        maximum_specimens,
        maximum_bootstrap_draws,
    )?;
    let retained = retained_bytes(input_bytes.len(), &requirements, replicates)?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "hierarchical-bootstrap retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    if source_artifact(&input_path, INPUT_KIND)? != input_artifact {
        return Err(BayesCliError::Input(
            "hierarchical-bootstrap input changed while prepared".into(),
        ));
    }

    let analysis = HierarchicalBootstrapSpec {
        replicates,
        seed,
        alpha,
    };
    let node = CohortHierarchicalBootstrapNode::new(
        input_path,
        input_path_identity,
        input_artifact.clone(),
        records,
        analysis,
        requirements,
        maximum_patients,
        maximum_specimens,
        maximum_bootstrap_draws,
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
        ArtifactSchema::new("marklab.cohort_hierarchical_bootstrap", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project cohort-hierarchical-bootstrap cache_status={cache_status}");
    Ok(())
}

fn validate_declared_limits(
    maximum_patients: usize,
    maximum_specimens: usize,
    maximum_bootstrap_draws: u64,
    memory_budget_mib: usize,
) -> Result<(), BayesCliError> {
    if maximum_patients == 0
        || maximum_specimens == 0
        || maximum_bootstrap_draws == 0
        || memory_budget_mib == 0
    {
        return Err(BayesCliError::Input(
            "hierarchical-bootstrap resource limits must be positive".into(),
        ));
    }
    if maximum_patients > FIXED_MAXIMUM_PATIENTS
        || maximum_specimens > FIXED_MAXIMUM_SPECIMENS
        || maximum_bootstrap_draws > FIXED_MAXIMUM_BOOTSTRAP_DRAWS
    {
        return Err(BayesCliError::Input(
            "hierarchical-bootstrap declared resource limits exceed fixed production ceilings"
                .into(),
        ));
    }
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
            "hierarchical-bootstrap input must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Clone)]
struct Requirements {
    patient_count: usize,
    specimen_count: usize,
    maximum_specimens_per_patient: usize,
    maximum_bootstrap_draws: u64,
}

fn preflight(
    records: &[HierarchicalScalarRecord],
    replicates: usize,
    alpha: f64,
    maximum_patients: usize,
    maximum_specimens: usize,
    maximum_bootstrap_draws: u64,
) -> Result<Requirements, BayesCliError> {
    if replicates == 0 || replicates > 1_000_000 {
        return Err(BayesCliError::Input(
            "bootstrap replicates must be between 1 and 1000000".into(),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 0.5 {
        return Err(BayesCliError::Input(
            "bootstrap alpha must be finite and strictly between zero and one half".into(),
        ));
    }
    if records.len() < 2 {
        return Err(BayesCliError::Input(
            "hierarchical bootstrap requires at least two specimen rows".into(),
        ));
    }
    if records.len() > maximum_specimens {
        return Err(BayesCliError::Input(format!(
            "hierarchical-bootstrap specimen count {} exceeds maximum_specimens {maximum_specimens}",
            records.len()
        )));
    }
    let mut specimen_ids = HashSet::with_capacity(records.len());
    let mut patients = BTreeMap::<&str, usize>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || record.specimen_id.is_empty()
            || record.specimen_id.trim() != record.specimen_id
            || !record.endpoint.is_finite()
        {
            return Err(BayesCliError::Input(
                "hierarchical-bootstrap records require exact patient/specimen IDs and finite endpoints"
                    .into(),
            ));
        }
        if !specimen_ids.insert(record.specimen_id.as_str()) {
            return Err(BayesCliError::Input(format!(
                "duplicate specimen_id: {}",
                record.specimen_id
            )));
        }
        *patients.entry(record.patient_id.as_str()).or_default() += 1;
    }
    if patients.len() < 2 {
        return Err(BayesCliError::Input(
            "hierarchical bootstrap requires at least two patients".into(),
        ));
    }
    if patients.len() > maximum_patients {
        return Err(BayesCliError::Input(format!(
            "hierarchical-bootstrap patient count {} exceeds maximum_patients {maximum_patients}",
            patients.len()
        )));
    }
    let maximum_specimens_per_patient = patients.values().copied().max().unwrap_or(0);
    let draw_count = u64::try_from(patients.len())
        .ok()
        .and_then(|patients| {
            u64::try_from(maximum_specimens_per_patient)
                .ok()
                .and_then(|specimens| patients.checked_mul(specimens))
        })
        .and_then(|per_replicate| per_replicate.checked_mul(replicates as u64))
        .ok_or_else(|| {
            BayesCliError::Input("hierarchical-bootstrap draw count overflowed".into())
        })?;
    if draw_count > maximum_bootstrap_draws {
        return Err(BayesCliError::Input(format!(
            "hierarchical-bootstrap {draw_count} bootstrap draws exceed maximum_bootstrap_draws {maximum_bootstrap_draws}"
        )));
    }
    Ok(Requirements {
        patient_count: patients.len(),
        specimen_count: records.len(),
        maximum_specimens_per_patient,
        maximum_bootstrap_draws: draw_count,
    })
}

fn retained_bytes(
    source_bytes: usize,
    requirements: &Requirements,
    replicates: usize,
) -> Result<usize, BayesCliError> {
    source_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(requirements.specimen_count.saturating_mul(512)))
        .and_then(|value| value.checked_add(requirements.patient_count.saturating_mul(256)))
        .and_then(|value| value.checked_add(replicates.saturating_mul(16)))
        .and_then(|value| {
            value.checked_add(
                requirements
                    .patient_count
                    .saturating_mul(requirements.maximum_specimens_per_patient)
                    .saturating_mul(16),
            )
        })
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or_else(|| {
            BayesCliError::Input("hierarchical-bootstrap memory estimate overflowed".into())
        })
}

struct CohortHierarchicalBootstrapNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    input_artifacts: Vec<ArtifactRef>,
    records: Vec<HierarchicalScalarRecord>,
    analysis: HierarchicalBootstrapSpec,
    requirements: Requirements,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl CohortHierarchicalBootstrapNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input_path: PathBuf,
        input_path_identity: Vec<u8>,
        input_artifact: ArtifactRef,
        records: Vec<HierarchicalScalarRecord>,
        analysis: HierarchicalBootstrapSpec,
        requirements: Requirements,
        maximum_patients: usize,
        maximum_specimens: usize,
        maximum_bootstrap_draws: u64,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let alpha_bits = analysis.alpha.to_bits().to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-hierarchical-bootstrap-configuration-v1".as_slice(),
            input_path_identity.as_slice(),
            analysis.replicates.to_string().as_bytes(),
            analysis.seed.to_string().as_bytes(),
            alpha_bits.as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_specimens.to_string().as_bytes(),
            maximum_bootstrap_draws.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;patient-then-nested-specimen-bootstrap;nearest-rank-percentile;replicates={};seed={};alpha_bits={alpha_bits};maximum_patients={maximum_patients};maximum_specimens={maximum_specimens};maximum_bootstrap_draws={maximum_bootstrap_draws};memory_budget_mib={memory_budget_mib}",
            analysis.replicates, analysis.seed
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("cohort-hierarchical-bootstrap")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "cohort_hierarchical_bootstrap",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_path_identity,
            input_artifacts: vec![input_artifact],
            records,
            analysis,
            requirements,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate(&self, output: &HierarchicalBootstrapOutput) -> io::Result<()> {
        if serde_json::to_vec(&output.input).ok().as_deref()
            != Some(self.input_path_identity.as_slice())
            || output.format != "marklab.cohort_hierarchical_bootstrap"
            || output.version != 1
            || output.design.levels != ["patient", "specimen"]
            || output.design.null_family != "hierarchical_bootstrap"
            || output.design.permutation_unit != "patient_then_nested_specimen"
            || output.design.statistic != "specimen_row_mean"
            || output.design.interval_method != "nearest_rank_percentile"
            || output.patients != self.requirements.patient_count
            || output.specimens != self.requirements.specimen_count
            || !output.observed_mean.is_finite()
            || !output.interval.lower.is_finite()
            || !output.interval.upper.is_finite()
            || output.interval.lower > output.interval.upper
            || output.interval.level.to_bits() != (1.0 - self.analysis.alpha).to_bits()
            || output.replicates.requested != self.analysis.replicates
            || output.replicates.attempted != self.analysis.replicates
            || output.replicates.completed != self.analysis.replicates
            || output.seed != self.analysis.seed
            || output.alpha.to_bits() != self.analysis.alpha.to_bits()
            || self.requirements.maximum_bootstrap_draws > FIXED_MAXIMUM_BOOTSTRAP_DRAWS
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "decoded hierarchical-bootstrap result differs from the prepared analysis",
            ));
        }
        Ok(())
    }
}

impl WorkflowNode for CohortHierarchicalBootstrapNode {
    type Output = HierarchicalBootstrapOutput;

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
                "hierarchical-bootstrap input identity changed",
            )));
        }
        if serde_json::to_vec(&self.input_path).ok().as_deref()
            != Some(self.input_path_identity.as_slice())
        {
            return Err(NodeError::input(io::Error::new(
                io::ErrorKind::InvalidData,
                "hierarchical-bootstrap input path identity changed",
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
        let result =
            hierarchical_bootstrap(&self.records, &self.analysis).map_err(NodeError::execution)?;
        Ok(HierarchicalBootstrapOutput::from_result(
            self.input_path.clone(),
            result,
        ))
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: HierarchicalBootstrapOutput =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
