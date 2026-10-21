use std::{borrow::Cow, collections::BTreeMap, io};

use marklab_cohort::{
    cluster_covariate_matrix_freedman_lane, ClusterCovariatePatientRecord, CovariatePermutationSpec,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::super::cohort::{
    cluster_covariate::{read_records_from_bytes, Output},
    CliAlternative,
};
use super::cli_schema::{
    CohortClusterCovariatePermutationLimits, CohortClusterCovariatePermutationProjectArgs,
};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.cluster-covariate-patient-rows-csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.cohort-cluster-covariate-permutation+json;version=2";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-cohort-cluster-covariate-permutation-node-v2";
const FIXED_MAXIMUM_PATIENTS: usize = 1_000_000;
const FIXED_MAXIMUM_CLUSTERS: usize = 1_000_000;
const FIXED_MAXIMUM_COVARIATES: usize = 32;
const FIXED_MAXIMUM_PATIENT_COVARIATE_CELLS: u64 = 32_000_000;
const FIXED_MAXIMUM_OLS_WORK: u64 = 100_000_000;
const EXCHANGEABILITY_ASSUMPTION: &str =
    "reduced-model residuals are exchangeable across independent clusters conditional on the fixed cluster-level nuisance matrix";

pub(super) fn run(args: CohortClusterCovariatePermutationProjectArgs) -> Result<(), CliError> {
    let CohortClusterCovariatePermutationProjectArgs {
        project: project_path,
        input: input_path,
        group_a,
        group_b,
        permutations,
        seed,
        alternative,
        limits,
        out: output_path,
    } = args;
    let CohortClusterCovariatePermutationLimits {
        maximum_patients,
        maximum_clusters,
        maximum_covariates,
        maximum_patient_covariate_cells,
        maximum_ols_work,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "cluster-covariate input";
    validate_declared_limits(
        maximum_patients,
        maximum_clusters,
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
    let records = read_records_from_bytes(&input_bytes)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let requirements = preflight(&records, &group_a, &group_b, permutations, limits)?;
    let retained = retained_bytes(input_bytes.len(), &records, &requirements, permutations)?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "cluster-covariate retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    source.ensure_unchanged(LABEL)?;

    let analysis = CovariatePermutationSpec {
        group_a,
        group_b,
        permutations,
        seed,
        alternative: alternative.into(),
    };
    let (analysis, contract) = CohortClusterCovariate::new(
        input_path_identity,
        records,
        analysis,
        alternative,
        requirements,
        limits,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        analysis,
        CohortClusterCovariate::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_cluster_covariate_permutation", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "cohort-cluster-covariate-permutation",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn validate_declared_limits(
    maximum_patients: usize,
    maximum_clusters: usize,
    maximum_covariates: usize,
    maximum_patient_covariate_cells: u64,
    maximum_ols_work: u64,
    memory_budget_mib: usize,
) -> Result<(), CliError> {
    if maximum_patients == 0
        || maximum_clusters == 0
        || maximum_covariates == 0
        || maximum_patient_covariate_cells == 0
        || maximum_ols_work == 0
        || memory_budget_mib == 0
    {
        return Err(CliError::Input(
            "cluster-covariate resource limits must be positive".into(),
        ));
    }
    if maximum_patients > FIXED_MAXIMUM_PATIENTS
        || maximum_clusters > FIXED_MAXIMUM_CLUSTERS
        || maximum_covariates > FIXED_MAXIMUM_COVARIATES
        || maximum_patient_covariate_cells > FIXED_MAXIMUM_PATIENT_COVARIATE_CELLS
        || maximum_ols_work > FIXED_MAXIMUM_OLS_WORK
    {
        return Err(CliError::Input(
            "cluster-covariate declared resource limits exceed fixed production ceilings".into(),
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
            "cluster-covariate input must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

#[derive(Clone)]
struct Requirements {
    patient_count: usize,
    cluster_count: usize,
    group_a_clusters: usize,
    group_b_clusters: usize,
    covariate_names: Vec<String>,
    patient_covariate_cells: u64,
    ols_work: u64,
}

fn preflight(
    records: &[ClusterCovariatePatientRecord],
    group_a: &str,
    group_b: &str,
    permutations: usize,
    limits: CohortClusterCovariatePermutationLimits,
) -> Result<Requirements, CliError> {
    let CohortClusterCovariatePermutationLimits {
        maximum_patients,
        maximum_clusters,
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
            "cluster-covariate group labels must be non-empty, exact, and distinct".into(),
        ));
    }
    if permutations == 0 || permutations > 1_000_000 {
        return Err(CliError::Input(
            "cluster-covariate permutations must be between 1 and 1000000".into(),
        ));
    }
    if records.is_empty() {
        return Err(CliError::Input(
            "cluster-covariate input has no patient rows".into(),
        ));
    }
    if records.len() > maximum_patients {
        return Err(CliError::Input(format!(
            "cluster-covariate patient count {} exceeds maximum_patients {maximum_patients}",
            records.len()
        )));
    }
    let covariate_names = records[0].covariate_names.clone();
    if covariate_names.len() > maximum_covariates {
        return Err(CliError::Input(format!(
            "cluster-covariate covariate count {} exceeds maximum_covariates {maximum_covariates}",
            covariate_names.len()
        )));
    }
    let mut clusters = BTreeMap::<&str, bool>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || record.cluster_id.is_empty()
            || record.cluster_id.trim() != record.cluster_id
            || !record.outcome.is_finite()
            || record.covariate_names != covariate_names
            || record.covariates.len() != covariate_names.len()
            || record.covariates.iter().any(|value| !value.is_finite())
        {
            return Err(CliError::Input(
                "every cluster-covariate patient requires exact IDs and one complete finite nuisance vector"
                    .into(),
            ));
        }
        let is_group_a = if record.group == group_a {
            true
        } else if record.group == group_b {
            false
        } else {
            return Err(CliError::Input(format!(
                "patient {} has undeclared group {:?}",
                record.patient_id, record.group
            )));
        };
        if clusters
            .insert(record.cluster_id.as_str(), is_group_a)
            .is_some_and(|existing| existing != is_group_a)
        {
            return Err(CliError::Input(format!(
                "cluster {} contains conflicting group labels",
                record.cluster_id
            )));
        }
    }
    if clusters.len() > maximum_clusters {
        return Err(CliError::Input(format!(
            "cluster-covariate cluster count {} exceeds maximum_clusters {maximum_clusters}",
            clusters.len()
        )));
    }
    let group_a_clusters = clusters.values().filter(|value| **value).count();
    let group_b_clusters = clusters.len() - group_a_clusters;
    if group_a_clusters < 2 || group_b_clusters < 2 {
        return Err(CliError::Input(
            "each group must contain at least two independent clusters".into(),
        ));
    }
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
            "cluster-covariate {patient_covariate_cells} patient-covariate cells exceed maximum_patient_covariate_cells {maximum_patient_covariate_cells}"
        )));
    }
    let cluster_count = u64::try_from(clusters.len())
        .map_err(|_| CliError::Input("cluster count overflowed".into()))?;
    let full_columns = u64::try_from(covariate_names.len() + 2)
        .map_err(|_| CliError::Input("design column count overflowed".into()))?;
    let per_fit = cluster_count
        .checked_mul(full_columns)
        .and_then(|value| value.checked_mul(full_columns))
        .and_then(|value| value.checked_add(full_columns.pow(3)))
        .ok_or_else(|| CliError::Input("cluster-covariate OLS work overflowed".into()))?;
    let ols_work = per_fit
        .checked_mul(permutations as u64 + 2)
        .ok_or_else(|| CliError::Input("cluster-covariate OLS work overflowed".into()))?;
    if ols_work > maximum_ols_work {
        return Err(CliError::Input(format!(
            "cluster-covariate {ols_work} OLS work units exceed maximum_ols_work {maximum_ols_work}"
        )));
    }
    Ok(Requirements {
        patient_count: records.len(),
        cluster_count: clusters.len(),
        group_a_clusters,
        group_b_clusters,
        covariate_names,
        patient_covariate_cells,
        ols_work,
    })
}

fn retained_bytes(
    source_bytes: usize,
    records: &[ClusterCovariatePatientRecord],
    requirements: &Requirements,
    permutations: usize,
) -> Result<usize, CliError> {
    let patient_bytes = records
        .len()
        .checked_mul(512 + requirements.covariate_names.len().saturating_mul(32))
        .ok_or_else(|| CliError::Input("cluster-covariate memory estimate overflowed".into()))?;
    let cluster_bytes = requirements
        .cluster_count
        .checked_mul(1024 + requirements.covariate_names.len().saturating_mul(64))
        .ok_or_else(|| CliError::Input("cluster-covariate memory estimate overflowed".into()))?;
    source_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(patient_bytes))
        .and_then(|value| value.checked_add(cluster_bytes))
        .and_then(|value| value.checked_add(permutations.saturating_mul(16)))
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or_else(|| CliError::Input("cluster-covariate memory estimate overflowed".into()))
}

struct CohortClusterCovariate {
    records: Vec<ClusterCovariatePatientRecord>,
    analysis: CovariatePermutationSpec,
    alternative: CliAlternative,
    requirements: Requirements,
}

impl CohortClusterCovariate {
    fn new(
        input_path_identity: Vec<u8>,
        records: Vec<ClusterCovariatePatientRecord>,
        analysis: CovariatePermutationSpec,
        alternative: CliAlternative,
        requirements: Requirements,
        limits: CohortClusterCovariatePermutationLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let CohortClusterCovariatePermutationLimits {
            maximum_patients,
            maximum_clusters,
            maximum_covariates,
            maximum_patient_covariate_cells,
            maximum_ols_work,
            memory_budget_mib,
        } = limits;
        let alternative_name = alternative_name(alternative);
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-cluster-covariate-configuration-v1".as_slice(),
            input_path_identity.as_slice(),
            analysis.group_a.as_bytes(),
            analysis.group_b.as_bytes(),
            analysis.permutations.to_string().as_bytes(),
            analysis.seed.to_string().as_bytes(),
            alternative_name.as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_clusters.to_string().as_bytes(),
            maximum_covariates.to_string().as_bytes(),
            maximum_patient_covariate_cells.to_string().as_bytes(),
            maximum_ols_work.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;equal-weight-patient-cluster-summaries;complete-cluster-residual-permutation;alternative={alternative_name};permutations={};seed={};maximum_patients={maximum_patients};maximum_clusters={maximum_clusters};maximum_covariates={maximum_covariates};maximum_patient_covariate_cells={maximum_patient_covariate_cells};maximum_ols_work={maximum_ols_work};memory_budget_mib={memory_budget_mib}",
            analysis.permutations, analysis.seed
        )
        .into_bytes();
        Ok((
            Self {
                records,
                analysis,
                alternative,
                requirements,
            },
            NodeContract {
                id: "cohort-cluster-covariate",
                kind: "cohort_cluster_covariate_permutation",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::ExactFloatJson,
            },
        ))
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        let expected_residual_df = self
            .requirements
            .cluster_count
            .saturating_sub(self.requirements.covariate_names.len() + 2);
        if output.format != "marklab.cohort_cluster_covariate_permutation"
            || output.version != 1
            || output.design.analysis_level != "cluster"
            || output.design.null_family != "cluster_covariate_residual_permutation"
            || output.design.permutation_unit != "complete_cluster_residual"
            || output.design.cluster_summary != "equal_weight_patient_mean"
            || output.design.exchangeability_assumption != EXCHANGEABILITY_ASSUMPTION
            || output.design.nuisance_columns != self.requirements.covariate_names.len()
            || output.design.reduced_model_columns != self.requirements.covariate_names.len() + 1
            || output.design.full_model_columns != self.requirements.covariate_names.len() + 2
            || output.design.residual_degrees_of_freedom != expected_residual_df
            || output.patients != self.requirements.patient_count
            || output.clusters.total != self.requirements.cluster_count
            || output.clusters.group_a != self.requirements.group_a_clusters
            || output.clusters.group_b != self.requirements.group_b_clusters
            || output.groups.group_a != self.analysis.group_a
            || output.groups.group_b != self.analysis.group_b
            || output.covariates.names != self.requirements.covariate_names
            || output.covariates.centers.len() != self.requirements.covariate_names.len()
            || output.covariates.scales.len() != self.requirements.covariate_names.len()
            || output
                .covariates
                .centers
                .iter()
                .any(|value| !value.is_finite())
            || output
                .covariates
                .scales
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || !output.effect_group_a_minus_group_b.is_finite()
            || !output.target_standard_error.is_finite()
            || output.target_standard_error <= 0.0
            || !output.studentized_statistic.is_finite()
            || !output.p_value.is_finite()
            || output.p_value <= 0.0
            || output.p_value > 1.0
            || !p_value_is_on_lattice(output.p_value, self.analysis.permutations, self.alternative)
            || output.permutations.requested != self.analysis.permutations
            || output.permutations.seed != self.analysis.seed
            || output.permutations.alternative != self.alternative
            || output.claim_status != "experimental_cluster_residual_exchangeability_required"
            || self.requirements.patient_covariate_cells > FIXED_MAXIMUM_PATIENT_COVARIATE_CELLS
            || self.requirements.ols_work > FIXED_MAXIMUM_OLS_WORK
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "decoded cluster-covariate result differs from the prepared analysis",
            ));
        }
        Ok(())
    }

    fn execute(&self) -> Result<Output, NodeError> {
        let result = cluster_covariate_matrix_freedman_lane(&self.records, &self.analysis)
            .map_err(NodeError::execution)?;
        Ok(Output::from_result(result, self.alternative))
    }
}

fn alternative_name(alternative: CliAlternative) -> &'static str {
    match alternative {
        CliAlternative::Less => "less",
        CliAlternative::Greater => "greater",
        CliAlternative::TwoSided => "two-sided",
    }
}

fn p_value_is_on_lattice(p_value: f64, permutations: usize, alternative: CliAlternative) -> bool {
    if p_value == 1.0 {
        return true;
    }
    let scaled = p_value * (permutations + 1) as f64;
    let lattice_value = match alternative {
        CliAlternative::Less | CliAlternative::Greater => scaled,
        CliAlternative::TwoSided => scaled / 2.0,
    };
    (lattice_value - lattice_value.round()).abs() <= 1e-9
}
