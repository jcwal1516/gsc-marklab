use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::Args;
use marklab_bayes::{
    json_sha256, sha256_hex, OrdinalGroupSiteHierarchyInputIdentity, OrdinalGroupSiteHierarchySpec,
    OrdinalGroupSiteHierarchyWorkerRequest, OrdinalGroupSiteHierarchyWorkerResult,
    OrdinalGroupSitePatientData,
};
use serde::Deserialize;

use super::OrdinalGroupSiteHierarchyModelArgs;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Args)]
pub(crate) struct OrdinalGroupSiteHierarchyArgs {
    #[command(flatten)]
    pub(super) model: OrdinalGroupSiteHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    site_id: String,
    group: String,
    outcome: String,
}

pub(super) fn run(args: OrdinalGroupSiteHierarchyArgs) -> Result<(), CliError> {
    let prepared = prepare(args.model)?;
    let result = execute(&prepared)?;
    publish_json(
        &args.out,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedOrdinalGroupSiteHierarchy {
    pub(crate) request: OrdinalGroupSiteHierarchyWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: OrdinalGroupSiteHierarchyInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    model: OrdinalGroupSiteHierarchyModelArgs,
) -> Result<PreparedOrdinalGroupSiteHierarchy, CliError> {
    let OrdinalGroupSiteHierarchyModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        ordered_levels,
        cutpoint_prior_sd,
        group_effect_prior_sd,
        site_intercept_sd_prior_sd,
        site_group_slope_sd_prior_sd,
        timeout_seconds,
        nuts,
    } = model;
    let sampling = nuts.spec();

    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "ordinal site hierarchy input exceeds 16 MiB".into(),
        ));
    }
    let input_bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let level_codes = ordered_levels
        .iter()
        .enumerate()
        .map(|(index, level)| (level.as_str(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let patients = reader
        .deserialize::<PatientRow>()
        .map(|row| -> Result<OrdinalGroupSitePatientData, CliError> {
            let row = row?;
            let outcome_code = level_codes
                .get(row.outcome.as_str())
                .copied()
                .ok_or_else(|| {
                    CliError::Input(format!(
                        "ordinal site hierarchy outcome {:?} is not in --ordered-levels",
                        row.outcome
                    ))
                })?;
            Ok(OrdinalGroupSitePatientData {
                patient_id: row.patient_id,
                site_id: row.site_id,
                group: row.group,
                outcome_code,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_bytes = fs::read(worker_directory.join("uv.lock")).map_err(|source| CliError::Io {
        path: worker_directory.join("uv.lock"),
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_ordinal_group_site_hierarchy_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let request = OrdinalGroupSiteHierarchyWorkerRequest::new(
        OrdinalGroupSiteHierarchySpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            ordered_levels: ordered_levels.clone(),
            cutpoint_prior_sd,
            group_effect_prior_sd,
            site_intercept_sd_prior_sd,
            site_group_slope_sd_prior_sd,
            patients,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input_identity = OrdinalGroupSiteHierarchyInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: json_sha256(&request.patients)?,
        patient_count: request.patients.len(),
        site_count: request.site_ids.len(),
        level_count: ordered_levels.len(),
        reference_patients: request
            .patients
            .iter()
            .filter(|patient| patient.group == reference_group)
            .count(),
        comparison_patients: request
            .patients
            .iter()
            .filter(|patient| patient.group == comparison_group)
            .count(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedOrdinalGroupSiteHierarchy {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedOrdinalGroupSiteHierarchy,
) -> Result<OrdinalGroupSiteHierarchyWorkerResult, CliError> {
    let result_bytes = run_worker(
        &marklab::backend::python_backend_assets_root()?,
        "marklab_pymc_ordinal_group_site_hierarchy_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: OrdinalGroupSiteHierarchyWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
