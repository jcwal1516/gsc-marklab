use super::NonproportionalOrdinalGroupSiteModelArgs;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};
use clap::Args;
use marklab_bayes::{
    json_sha256, sha256_hex, NonproportionalOrdinalGroupSiteInputIdentity,
    NonproportionalOrdinalGroupSiteSpec, NonproportionalOrdinalGroupSiteWorkerRequest,
    NonproportionalOrdinalGroupSiteWorkerResult, OrdinalGroupSitePatientData,
};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Debug, Args)]
pub(crate) struct NonproportionalOrdinalGroupSiteArgs {
    #[command(flatten)]
    pub(super) model: NonproportionalOrdinalGroupSiteModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    patient_id: String,
    site_id: String,
    group: String,
    outcome: String,
}

pub(super) fn run(a: NonproportionalOrdinalGroupSiteArgs) -> Result<(), CliError> {
    let prepared = prepare(a.model)?;
    let result = execute(&prepared)?;
    publish_json(
        &a.out,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedNonproportionalOrdinalGroupSite {
    pub(crate) request: NonproportionalOrdinalGroupSiteWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: NonproportionalOrdinalGroupSiteInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    model: NonproportionalOrdinalGroupSiteModelArgs,
) -> Result<PreparedNonproportionalOrdinalGroupSite, CliError> {
    let NonproportionalOrdinalGroupSiteModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        ordered_levels,
        cutpoint_prior_sd,
        site_intercept_sd_prior_sd,
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
            "nonproportional ordinal input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let codes = ordered_levels
        .iter()
        .enumerate()
        .map(|(i, x)| (x.as_str(), i as u32))
        .collect::<BTreeMap<_, _>>();
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
    let patients = reader
        .deserialize::<Row>()
        .map(|row| -> Result<_, CliError> {
            let row = row?;
            let outcome_code = codes.get(row.outcome.as_str()).copied().ok_or_else(|| {
                CliError::Input(format!("outcome {:?} is not ordered", row.outcome))
            })?;
            Ok(OrdinalGroupSitePatientData {
                patient_id: row.patient_id,
                site_id: row.site_id,
                group: row.group,
                outcome_code,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root = &marklab::backend::python_backend_assets_root()?;
    let directory = root.join("workers/python");
    let lock_path = directory.join("uv.lock");
    let lock = fs::read(&lock_path).map_err(|source| CliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = directory.join("marklab_pymc_nonproportional_ordinal_group_site_worker.py");
    let worker = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NonproportionalOrdinalGroupSiteWorkerRequest::new(
        NonproportionalOrdinalGroupSiteSpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            ordered_levels: ordered_levels.clone(),
            cutpoint_prior_sd,
            site_intercept_sd_prior_sd,
            patients,
        },
        sampling,
        sha256_hex(&lock),
        sha256_hex(&worker),
        timeout_seconds,
    )?;
    let identity = NonproportionalOrdinalGroupSiteInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: json_sha256(&request.patients)?,
        patient_count: request.patients.len(),
        site_count: request.site_ids.len(),
        level_count: ordered_levels.len(),
        reference_patients: request
            .patients
            .iter()
            .filter(|p| p.group == reference_group)
            .count(),
        comparison_patients: request
            .patients
            .iter()
            .filter(|p| p.group == comparison_group)
            .count(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedNonproportionalOrdinalGroupSite {
        request,
        request_bytes,
        request_sha256,
        input_identity: identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedNonproportionalOrdinalGroupSite,
) -> Result<NonproportionalOrdinalGroupSiteWorkerResult, CliError> {
    let bytes = run_worker(
        &marklab::backend::python_backend_assets_root()?,
        "marklab_pymc_nonproportional_ordinal_group_site_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: NonproportionalOrdinalGroupSiteWorkerResult = serde_json::from_slice(&bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
