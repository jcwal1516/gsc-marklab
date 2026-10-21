use std::{borrow::Cow, collections::BTreeMap, io, path::PathBuf};

use marklab_cohort::{
    multisite_covariate_patient_contrast, MultisiteCovariatePatientRecord, MultisiteEffectModel,
    MultisiteInferenceSpec,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::super::cohort::{
    multisite::{read_covariate_records_from_bytes, CovariateContrastOutput},
    CliMultisiteModel,
};
use super::cli_schema::{
    CohortMultisiteCovariateContrastLimits, CohortMultisiteCovariateContrastProjectArgs,
};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
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

pub(super) fn run(args: CohortMultisiteCovariateContrastProjectArgs) -> Result<(), CliError> {
    let CohortMultisiteCovariateContrastProjectArgs {
        project: project_path,
        input: input_path,
        group_a,
        group_b,
        model,
        alpha,
        limits,
        out: output_path,
    } = args;
    let CohortMultisiteCovariateContrastLimits {
        maximum_patients,
        maximum_sites,
        maximum_covariates,
        maximum_patient_covariate_cells,
        maximum_ols_work,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "multisite-covariate input";
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
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        CliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let source = Source::from_bytes(input_path.clone(), INPUT_KIND, &input_bytes)?;
    let records = read_covariate_records_from_bytes(&input_bytes)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let requirements = preflight(&records, &group_a, &group_b, alpha, limits)?;
    let retained = retained_bytes(input_bytes.len(), &requirements)?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "multisite-covariate retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    source.ensure_unchanged(LABEL)?;

    let analysis = Analysis {
        group_a,
        group_b,
        inference: MultisiteInferenceSpec {
            model: model.into(),
            alpha,
        },
        cli_model: model,
    };
    let (analysis, contract) = CohortMultisiteCovariate::new(
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
        CohortMultisiteCovariate::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_multisite_covariate_contrast", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "cohort-multisite-covariate-contrast",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn validate_declared_limits(
    maximum_patients: usize,
    maximum_sites: usize,
    maximum_covariates: usize,
    maximum_patient_covariate_cells: u64,
    maximum_ols_work: u64,
    memory_budget_mib: usize,
) -> Result<(), CliError> {
    if maximum_patients == 0
        || maximum_sites == 0
        || maximum_covariates == 0
        || maximum_patient_covariate_cells == 0
        || maximum_ols_work == 0
        || memory_budget_mib == 0
    {
        return Err(CliError::Input(
            "multisite-covariate resource limits must be positive".into(),
        ));
    }
    if maximum_patients > FIXED_MAXIMUM_PATIENTS
        || maximum_sites > FIXED_MAXIMUM_SITES
        || maximum_covariates > FIXED_MAXIMUM_COVARIATES
        || maximum_patient_covariate_cells > FIXED_MAXIMUM_PATIENT_COVARIATE_CELLS
        || maximum_ols_work > FIXED_MAXIMUM_OLS_WORK
    {
        return Err(CliError::Input(
            "multisite-covariate declared resource limits exceed fixed production ceilings".into(),
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
            "multisite-covariate input must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
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

fn preflight(
    records: &[MultisiteCovariatePatientRecord],
    group_a: &str,
    group_b: &str,
    alpha: f64,
    limits: CohortMultisiteCovariateContrastLimits,
) -> Result<Requirements, CliError> {
    let CohortMultisiteCovariateContrastLimits {
        maximum_patients,
        maximum_sites,
        maximum_covariates,
        maximum_patient_covariate_cells,
        maximum_ols_work,
        ..
    } = limits;
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(CliError::Input(
            "multisite-covariate groups must be distinct exact non-empty labels".into(),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 0.5 {
        return Err(CliError::Input(
            "multisite alpha must be in (0, 0.5)".into(),
        ));
    }
    if records.is_empty() {
        return Err(CliError::Input(
            "multisite-covariate input has no patient rows".into(),
        ));
    }
    if records.len() > maximum_patients {
        return Err(CliError::Input(format!(
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
        return Err(CliError::Input(
            "multisite nuisance names must be 1-32 exact unique bounded names".into(),
        ));
    }
    if covariate_names.len() > maximum_covariates {
        return Err(CliError::Input(format!(
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
            return Err(CliError::Input(
                "multisite-covariate rows require exact IDs and one complete finite nuisance vector"
                    .into(),
            ));
        }
        let group_index = if record.group == group_a {
            1
        } else if record.group == group_b {
            2
        } else {
            return Err(CliError::Input(format!(
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
        return Err(CliError::Input(format!(
            "multisite-covariate site count {} exceeds maximum_sites {maximum_sites}",
            sites.len()
        )));
    }
    if !(3..=FIXED_MAXIMUM_SITES).contains(&sites.len()) {
        return Err(CliError::Input(
            "multisite inference requires 3-10000 sites".into(),
        ));
    }
    let sites = sites
        .into_iter()
        .map(|(site_id, (patient_count, group_a_count, group_b_count))| {
            if group_a_count < 2 || group_b_count < 2 {
                return Err(CliError::Input(format!(
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
        .ok_or_else(|| CliError::Input("patient-covariate cell count overflowed".into()))?;
    if patient_covariate_cells > maximum_patient_covariate_cells {
        return Err(CliError::Input(format!(
            "multisite-covariate {patient_covariate_cells} patient-covariate cells exceed maximum_patient_covariate_cells {maximum_patient_covariate_cells}"
        )));
    }
    let full_columns = u64::try_from(covariate_names.len() + 2)
        .map_err(|_| CliError::Input("design column count overflowed".into()))?;
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
        .ok_or_else(|| CliError::Input("multisite-covariate OLS work overflowed".into()))?;
    if ols_work > maximum_ols_work {
        return Err(CliError::Input(format!(
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

fn retained_bytes(source_bytes: usize, requirements: &Requirements) -> Result<usize, CliError> {
    source_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(requirements.patient_count.saturating_mul(768)))
        .and_then(|value| value.checked_add(requirements.sites.len().saturating_mul(2048)))
        .and_then(|value| value.checked_add(128 * 1024))
        .ok_or_else(|| CliError::Input("multisite-covariate memory estimate overflowed".into()))
}

struct Analysis {
    group_a: String,
    group_b: String,
    inference: MultisiteInferenceSpec,
    cli_model: CliMultisiteModel,
}

struct CohortMultisiteCovariate {
    input_path: PathBuf,
    input_path_identity: Vec<u8>,
    records: Vec<MultisiteCovariatePatientRecord>,
    analysis: Analysis,
    requirements: Requirements,
}

impl CohortMultisiteCovariate {
    fn new(
        input_path: PathBuf,
        input_path_identity: Vec<u8>,
        records: Vec<MultisiteCovariatePatientRecord>,
        analysis: Analysis,
        requirements: Requirements,
        limits: CohortMultisiteCovariateContrastLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let CohortMultisiteCovariateContrastLimits {
            maximum_patients,
            maximum_sites,
            maximum_covariates,
            maximum_patient_covariate_cells,
            maximum_ols_work,
            memory_budget_mib,
        } = limits;
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
        Ok((
            Self {
                input_path,
                input_path_identity,
                records,
                analysis,
                requirements,
            },
            NodeContract {
                id: "cohort-multisite-covariate",
                kind: "cohort_multisite_covariate_contrast",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::ExactFloatJson,
            },
        ))
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

    fn execute(&self) -> Result<CovariateContrastOutput, NodeError> {
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
