use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    io,
    path::PathBuf,
};

use marklab_cohort::{hierarchical_bootstrap, HierarchicalBootstrapSpec, HierarchicalScalarRecord};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::super::cohort::hierarchical_bootstrap::{
    read_records_from_bytes, HierarchicalBootstrapOutput,
};
use super::cli_schema::{
    CohortHierarchicalBootstrapLimits, CohortHierarchicalBootstrapProjectArgs,
};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.hierarchical-scalar-specimens-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-hierarchical-bootstrap+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-hierarchical-bootstrap-node-v1";
const FIXED_MAXIMUM_PATIENTS: usize = 1_000_000;
const FIXED_MAXIMUM_SPECIMENS: usize = 1_000_000;
const FIXED_MAXIMUM_BOOTSTRAP_DRAWS: u64 = 100_000_000;

pub(super) fn run(args: CohortHierarchicalBootstrapProjectArgs) -> Result<(), CliError> {
    let CohortHierarchicalBootstrapProjectArgs {
        project: project_path,
        input: input_path,
        replicates,
        seed,
        alpha,
        limits,
        out: output_path,
    } = args;
    let CohortHierarchicalBootstrapLimits {
        maximum_patients,
        maximum_specimens,
        maximum_bootstrap_draws,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "hierarchical-bootstrap input";
    validate_declared_limits(
        maximum_patients,
        maximum_specimens,
        maximum_bootstrap_draws,
        memory_budget_mib,
    )?;
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        CliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let source = Source::from_bytes(input_path.clone(), INPUT_KIND, &input_bytes)?;
    let records = read_records_from_bytes(&input_bytes)
        .map_err(|error| CliError::Input(error.to_string()))?;
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
        return Err(CliError::Input(format!(
            "hierarchical-bootstrap retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    source.ensure_unchanged(LABEL)?;

    let analysis = HierarchicalBootstrapSpec {
        replicates,
        seed,
        alpha,
    };
    let (analysis, contract) = CohortHierarchicalBootstrap::new(
        input_path,
        input_path_identity,
        records,
        analysis,
        requirements,
        limits,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        analysis,
        CohortHierarchicalBootstrap::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_hierarchical_bootstrap", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "cohort-hierarchical-bootstrap",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn validate_declared_limits(
    maximum_patients: usize,
    maximum_specimens: usize,
    maximum_bootstrap_draws: u64,
    memory_budget_mib: usize,
) -> Result<(), CliError> {
    if maximum_patients == 0
        || maximum_specimens == 0
        || maximum_bootstrap_draws == 0
        || memory_budget_mib == 0
    {
        return Err(CliError::Input(
            "hierarchical-bootstrap resource limits must be positive".into(),
        ));
    }
    if maximum_patients > FIXED_MAXIMUM_PATIENTS
        || maximum_specimens > FIXED_MAXIMUM_SPECIMENS
        || maximum_bootstrap_draws > FIXED_MAXIMUM_BOOTSTRAP_DRAWS
    {
        return Err(CliError::Input(
            "hierarchical-bootstrap declared resource limits exceed fixed production ceilings"
                .into(),
        ));
    }
    Ok(())
}

fn read_bounded(path: &std::path::Path, memory_bytes: usize) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "hierarchical-bootstrap input must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
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
) -> Result<Requirements, CliError> {
    if replicates == 0 || replicates > 1_000_000 {
        return Err(CliError::Input(
            "bootstrap replicates must be between 1 and 1000000".into(),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 0.5 {
        return Err(CliError::Input(
            "bootstrap alpha must be finite and strictly between zero and one half".into(),
        ));
    }
    if records.len() < 2 {
        return Err(CliError::Input(
            "hierarchical bootstrap requires at least two specimen rows".into(),
        ));
    }
    if records.len() > maximum_specimens {
        return Err(CliError::Input(format!(
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
            return Err(CliError::Input(
                "hierarchical-bootstrap records require exact patient/specimen IDs and finite endpoints"
                    .into(),
            ));
        }
        if !specimen_ids.insert(record.specimen_id.as_str()) {
            return Err(CliError::Input(format!(
                "duplicate specimen_id: {}",
                record.specimen_id
            )));
        }
        *patients.entry(record.patient_id.as_str()).or_default() += 1;
    }
    if patients.len() < 2 {
        return Err(CliError::Input(
            "hierarchical bootstrap requires at least two patients".into(),
        ));
    }
    if patients.len() > maximum_patients {
        return Err(CliError::Input(format!(
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
        .ok_or_else(|| CliError::Input("hierarchical-bootstrap draw count overflowed".into()))?;
    if draw_count > maximum_bootstrap_draws {
        return Err(CliError::Input(format!(
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
) -> Result<usize, CliError> {
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
        .ok_or_else(|| CliError::Input("hierarchical-bootstrap memory estimate overflowed".into()))
}

struct CohortHierarchicalBootstrap {
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    records: Vec<HierarchicalScalarRecord>,
    analysis: HierarchicalBootstrapSpec,
    requirements: Requirements,
}

impl CohortHierarchicalBootstrap {
    fn new(
        input_path: PathBuf,
        input_path_identity: Vec<u8>,
        records: Vec<HierarchicalScalarRecord>,
        analysis: HierarchicalBootstrapSpec,
        requirements: Requirements,
        limits: CohortHierarchicalBootstrapLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let CohortHierarchicalBootstrapLimits {
            maximum_patients,
            maximum_specimens,
            maximum_bootstrap_draws,
            memory_budget_mib,
        } = limits;
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
        Ok((
            Self {
                input_path,
                input_path_identity,
                records,
                analysis,
                requirements,
            },
            NodeContract {
                id: "cohort-hierarchical-bootstrap",
                kind: "cohort_hierarchical_bootstrap",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::ExactFloatJson,
            },
        ))
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

    fn execute(&self) -> Result<HierarchicalBootstrapOutput, NodeError> {
        let result =
            hierarchical_bootstrap(&self.records, &self.analysis).map_err(NodeError::execution)?;
        Ok(HierarchicalBootstrapOutput::from_result(
            self.input_path.clone(),
            result,
        ))
    }
}
