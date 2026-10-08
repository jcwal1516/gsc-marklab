use std::{collections::BTreeMap, fs, io, path::PathBuf};

use marklab_cohort::{
    multisite_covariate_patient_contrast, MultisiteCovariatePatientRecord, MultisiteEffectModel,
    MultisiteInferenceSpec,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::super::cohort::{
    multisite::{read_covariate_records_from_bytes, CovariateContrastOutput},
    CliMultisiteModel,
};
use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.multisite-covariate-patient-rows-csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.cohort-multisite-covariate-contrast+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-multisite-covariate-contrast-node-v1";
const FIXED_MAXIMUM_PATIENTS: usize = 1_000_000;
const FIXED_MAXIMUM_SITES: usize = 10_000;
const FIXED_MAXIMUM_COVARIATES: usize = 32;
const FIXED_MAXIMUM_PATIENT_COVARIATE_CELLS: u64 = 32_000_000;
const FIXED_MAXIMUM_OLS_WORK: u64 = 100_000_000;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    group_a: String,
    group_b: String,
    model: CliMultisiteModel,
    alpha: f64,
    maximum_patients: usize,
    maximum_sites: usize,
    maximum_covariates: usize,
    maximum_patient_covariate_cells: u64,
    maximum_ols_work: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    validate_declared_limits(
        maximum_patients,
        maximum_sites,
        maximum_covariates,
        maximum_patient_covariate_cells,
        maximum_ols_work,
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
    let records = read_covariate_records_from_bytes(&input_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let requirements = preflight(
        &records,
        &group_a,
        &group_b,
        alpha,
        maximum_patients,
        maximum_sites,
        maximum_covariates,
        maximum_patient_covariate_cells,
        maximum_ols_work,
    )?;
    let retained = retained_bytes(input_bytes.len(), &requirements)?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "multisite-covariate retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    if source_artifact(&input_path, INPUT_KIND)? != input_artifact {
        return Err(BayesCliError::Input(
            "multisite-covariate input changed while prepared".into(),
        ));
    }

    let analysis = Analysis {
        group_a,
        group_b,
        inference: MultisiteInferenceSpec {
            model: model.into(),
            alpha,
        },
        cli_model: model,
    };
    let node = CohortMultisiteCovariateNode::new(
        input_path,
        input_path_identity,
        input_artifact.clone(),
        records,
        analysis,
        requirements,
        maximum_patients,
        maximum_sites,
        maximum_covariates,
        maximum_patient_covariate_cells,
        maximum_ols_work,
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
        ArtifactSchema::new("marklab.cohort_multisite_covariate_contrast", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project cohort-multisite-covariate-contrast cache_status={cache_status}");
    Ok(())
}

fn validate_declared_limits(
    maximum_patients: usize,
    maximum_sites: usize,
    maximum_covariates: usize,
    maximum_patient_covariate_cells: u64,
    maximum_ols_work: u64,
    memory_budget_mib: usize,
) -> Result<(), BayesCliError> {
    if maximum_patients == 0
        || maximum_sites == 0
        || maximum_covariates == 0
        || maximum_patient_covariate_cells == 0
        || maximum_ols_work == 0
        || memory_budget_mib == 0
    {
        return Err(BayesCliError::Input(
            "multisite-covariate resource limits must be positive".into(),
        ));
    }
    if maximum_patients > FIXED_MAXIMUM_PATIENTS
        || maximum_sites > FIXED_MAXIMUM_SITES
        || maximum_covariates > FIXED_MAXIMUM_COVARIATES
        || maximum_patient_covariate_cells > FIXED_MAXIMUM_PATIENT_COVARIATE_CELLS
        || maximum_ols_work > FIXED_MAXIMUM_OLS_WORK
    {
        return Err(BayesCliError::Input(
            "multisite-covariate declared resource limits exceed fixed production ceilings".into(),
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
            "multisite-covariate input must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Clone)]
struct SiteRequirements {
    site_id: String,
    patient_count: usize,
    group_a_count: usize,
    group_b_count: usize,
}

#[derive(Clone)]
struct Requirements {
    patient_count: usize,
    covariate_names: Vec<String>,
    sites: Vec<SiteRequirements>,
    patient_covariate_cells: u64,
    ols_work: u64,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    records: &[MultisiteCovariatePatientRecord],
    group_a: &str,
    group_b: &str,
    alpha: f64,
    maximum_patients: usize,
    maximum_sites: usize,
    maximum_covariates: usize,
    maximum_patient_covariate_cells: u64,
    maximum_ols_work: u64,
) -> Result<Requirements, BayesCliError> {
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(BayesCliError::Input(
            "multisite-covariate groups must be distinct exact non-empty labels".into(),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 0.5 {
        return Err(BayesCliError::Input(
            "multisite alpha must be in (0, 0.5)".into(),
        ));
    }
    if records.is_empty() {
        return Err(BayesCliError::Input(
            "multisite-covariate input has no patient rows".into(),
        ));
    }
    if records.len() > maximum_patients {
        return Err(BayesCliError::Input(format!(
            "multisite-covariate patient count {} exceeds maximum_patients {maximum_patients}",
            records.len()
        )));
    }
    let covariate_names = records[0].covariate_names.clone();
    if covariate_names.is_empty()
        || covariate_names.iter().any(|name| {
            name.is_empty()
                || name.len() > 128
                || name.trim() != name
                || name.chars().any(char::is_control)
        })
    {
        return Err(BayesCliError::Input(
            "multisite nuisance names must be 1-32 exact unique bounded names".into(),
        ));
    }
    if covariate_names.len() > maximum_covariates {
        return Err(BayesCliError::Input(format!(
            "multisite-covariate covariate count {} exceeds maximum_covariates {maximum_covariates}",
            covariate_names.len()
        )));
    }
    let mut sites = BTreeMap::<&str, (usize, usize, usize)>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || record.site_id.is_empty()
            || record.site_id.trim() != record.site_id
            || !record.endpoint.is_finite()
            || record.covariate_names != covariate_names
            || record.covariates.len() != covariate_names.len()
            || record.covariates.iter().any(|value| !value.is_finite())
        {
            return Err(BayesCliError::Input(
                "multisite-covariate rows require exact IDs and one complete finite nuisance vector"
                    .into(),
            ));
        }
        let group_index = if record.group == group_a {
            1
        } else if record.group == group_b {
            2
        } else {
            return Err(BayesCliError::Input(format!(
                "patient {} has undeclared group {:?}",
                record.patient_id, record.group
            )));
        };
        let counts = sites.entry(record.site_id.as_str()).or_default();
        counts.0 += 1;
        if group_index == 1 {
            counts.1 += 1;
        } else {
            counts.2 += 1;
        }
    }
    if sites.len() > maximum_sites {
        return Err(BayesCliError::Input(format!(
            "multisite-covariate site count {} exceeds maximum_sites {maximum_sites}",
            sites.len()
        )));
    }
    if !(3..=FIXED_MAXIMUM_SITES).contains(&sites.len()) {
        return Err(BayesCliError::Input(
            "multisite inference requires 3-10000 sites".into(),
        ));
    }
    let sites = sites
        .into_iter()
        .map(|(site_id, (patient_count, group_a_count, group_b_count))| {
            if group_a_count < 2 || group_b_count < 2 {
                return Err(BayesCliError::Input(format!(
                    "site {site_id}: each group must contain at least two patients"
                )));
            }
            Ok(SiteRequirements {
                site_id: site_id.to_owned(),
                patient_count,
                group_a_count,
                group_b_count,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let patient_covariate_cells = u64::try_from(records.len())
        .ok()
        .and_then(|patients| {
            u64::try_from(covariate_names.len())
                .ok()
                .and_then(|covariates| patients.checked_mul(covariates))
        })
        .ok_or_else(|| BayesCliError::Input("patient-covariate cell count overflowed".into()))?;
    if patient_covariate_cells > maximum_patient_covariate_cells {
        return Err(BayesCliError::Input(format!(
            "multisite-covariate {patient_covariate_cells} patient-covariate cells exceed maximum_patient_covariate_cells {maximum_patient_covariate_cells}"
        )));
    }
    let full_columns = u64::try_from(covariate_names.len() + 2)
        .map_err(|_| BayesCliError::Input("design column count overflowed".into()))?;
    let ols_work = u64::try_from(records.len())
        .ok()
        .and_then(|patients| patients.checked_mul(full_columns))
        .and_then(|value| value.checked_mul(full_columns))
        .and_then(|value| {
            u64::try_from(sites.len())
                .ok()
                .and_then(|site_count| site_count.checked_mul(full_columns.pow(3)))
                .and_then(|solver| value.checked_add(solver))
        })
        .ok_or_else(|| BayesCliError::Input("multisite-covariate OLS work overflowed".into()))?;
    if ols_work > maximum_ols_work {
        return Err(BayesCliError::Input(format!(
            "multisite-covariate {ols_work} OLS work units exceed maximum_ols_work {maximum_ols_work}"
        )));
    }
    Ok(Requirements {
        patient_count: records.len(),
        covariate_names,
        sites,
        patient_covariate_cells,
        ols_work,
    })
}

fn retained_bytes(
    source_bytes: usize,
    requirements: &Requirements,
) -> Result<usize, BayesCliError> {
    source_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(requirements.patient_count.saturating_mul(768)))
        .and_then(|value| value.checked_add(requirements.sites.len().saturating_mul(2048)))
        .and_then(|value| value.checked_add(128 * 1024))
        .ok_or_else(|| {
            BayesCliError::Input("multisite-covariate memory estimate overflowed".into())
        })
}

struct Analysis {
    group_a: String,
    group_b: String,
    inference: MultisiteInferenceSpec,
    cli_model: CliMultisiteModel,
}

struct CohortMultisiteCovariateNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    input_artifacts: Vec<ArtifactRef>,
    records: Vec<MultisiteCovariatePatientRecord>,
    analysis: Analysis,
    requirements: Requirements,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl CohortMultisiteCovariateNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input_path: PathBuf,
        input_path_identity: Vec<u8>,
        input_artifact: ArtifactRef,
        records: Vec<MultisiteCovariatePatientRecord>,
        analysis: Analysis,
        requirements: Requirements,
        maximum_patients: usize,
        maximum_sites: usize,
        maximum_covariates: usize,
        maximum_patient_covariate_cells: u64,
        maximum_ols_work: u64,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        let model_name = model_name(analysis.cli_model);
        let alpha_bits = analysis.inference.alpha.to_bits().to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-multisite-covariate-configuration-v1".as_slice(),
            input_path_identity.as_slice(),
            analysis.group_a.as_bytes(),
            analysis.group_b.as_bytes(),
            model_name.as_bytes(),
            alpha_bits.as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_sites.to_string().as_bytes(),
            maximum_covariates.to_string().as_bytes(),
            maximum_patient_covariate_cells.to_string().as_bytes(),
            maximum_ols_work.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;patient-unit;within-site-adjusted-ols;inverse-variance-pooling;model={model_name};alpha_bits={alpha_bits};maximum_patients={maximum_patients};maximum_sites={maximum_sites};maximum_covariates={maximum_covariates};maximum_patient_covariate_cells={maximum_patient_covariate_cells};maximum_ols_work={maximum_ols_work};memory_budget_mib={memory_budget_mib}"
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("cohort-multisite-covariate")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "cohort_multisite_covariate_contrast",
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

    fn validate(&self, output: &CovariateContrastOutput) -> io::Result<()> {
        let model = model_name(self.analysis.cli_model).replace('-', "_");
        if serde_json::to_vec(&output.input).ok().as_deref()
            != Some(self.input_path_identity.as_slice())
            || output.format != "marklab.cohort_multisite_covariate_contrast"
            || output.version != 1
            || output.design.population_unit != "patient"
            || output.design.site_effect != "adjusted_group_a_indicator"
            || output.design.standard_error != "within_site_ols"
            || output.design.nuisance_transform
                != "within_site_center_and_max_absolute_deviation_scale"
            || output.groups.group_a != self.analysis.group_a
            || output.groups.group_b != self.analysis.group_b
            || output.covariates.names != self.requirements.covariate_names
            || output.sites.len() != self.requirements.sites.len()
            || output.pooled.format != "marklab.cohort_multisite_inference"
            || output.pooled.version != 1
            || output.pooled.model != model
            || output.pooled.site_count != self.requirements.sites.len()
            || output.pooled.total_patient_count != self.requirements.patient_count
            || !finite_positive(output.pooled.pooled_standard_error)
            || !output.pooled.pooled_effect.is_finite()
            || !finite_interval(output.pooled.confidence_interval)
            || output.pooled.prediction_interval.is_some()
                != (self.analysis.inference.model == MultisiteEffectModel::RandomEffectsReml)
            || output
                .pooled
                .prediction_interval
                .is_some_and(|interval| !finite_interval(interval))
            || !output.pooled.heterogeneity.tau_squared.is_finite()
            || output.pooled.heterogeneity.tau_squared < 0.0
            || (self.analysis.inference.model == MultisiteEffectModel::FixedEffect
                && output.pooled.heterogeneity.tau_squared.to_bits() != 0.0_f64.to_bits())
            || !output.pooled.heterogeneity.q.is_finite()
            || output.pooled.heterogeneity.q < 0.0
            || output.pooled.heterogeneity.degrees_of_freedom + 1 != self.requirements.sites.len()
            || !unit_interval(output.pooled.heterogeneity.p_value)
            || output.pooled.leave_one_site_out.len() != self.requirements.sites.len()
            || output.pooled.alpha.to_bits() != self.analysis.inference.alpha.to_bits()
            || output.pooled.claim_status != "experimental_site_summary_meta_analysis"
            || self.requirements.patient_covariate_cells > FIXED_MAXIMUM_PATIENT_COVARIATE_CELLS
            || self.requirements.ols_work > FIXED_MAXIMUM_OLS_WORK
        {
            return Err(invalid());
        }
        for (site, expected) in output.sites.iter().zip(&self.requirements.sites) {
            if site.site_id != expected.site_id
                || site.group_a_patients != expected.group_a_count
                || site.group_b_patients != expected.group_b_count
                || site.residual_degrees_of_freedom
                    != expected
                        .patient_count
                        .saturating_sub(self.requirements.covariate_names.len() + 2)
                || !site.effect.is_finite()
                || !finite_positive(site.standard_error)
                || site.covariate_centers.len() != self.requirements.covariate_names.len()
                || site.covariate_scales.len() != self.requirements.covariate_names.len()
                || site
                    .covariate_centers
                    .iter()
                    .any(|value| !value.is_finite())
                || site
                    .covariate_scales
                    .iter()
                    .any(|value| !finite_positive(*value))
            {
                return Err(invalid());
            }
        }
        for (sensitivity, expected) in output
            .pooled
            .leave_one_site_out
            .iter()
            .zip(&self.requirements.sites)
        {
            if sensitivity.omitted_site_id != expected.site_id
                || !sensitivity.pooled_effect.is_finite()
                || !finite_positive(sensitivity.pooled_standard_error)
                || !sensitivity.tau_squared.is_finite()
                || sensitivity.tau_squared < 0.0
                || (self.analysis.inference.model == MultisiteEffectModel::FixedEffect
                    && sensitivity.tau_squared.to_bits() != 0.0_f64.to_bits())
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

impl WorkflowNode for CohortMultisiteCovariateNode {
    type Output = CovariateContrastOutput;

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
                "multisite-covariate input identity changed",
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
        let result = multisite_covariate_patient_contrast(
            &self.records,
            &self.analysis.group_a,
            &self.analysis.group_b,
            &self.analysis.inference,
        )
        .map_err(NodeError::execution)?;
        Ok(CovariateContrastOutput::from_result(
            self.input_path.clone(),
            result,
        ))
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: CovariateContrastOutput =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

fn model_name(model: CliMultisiteModel) -> &'static str {
    match model {
        CliMultisiteModel::FixedEffect => "fixed-effect",
        CliMultisiteModel::RandomEffectsReml => "random-effects-reml",
    }
}

fn finite_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn finite_interval(interval: [f64; 2]) -> bool {
    interval[0].is_finite() && interval[1].is_finite() && interval[0] <= interval[1]
}

fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "decoded multisite-covariate result differs from the prepared analysis",
    )
}
