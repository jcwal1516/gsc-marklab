use std::{collections::BTreeMap, fs};

use marklab_bayes::{
    json_sha256, sha256_hex, BetaBinomialGroupGenderSlideData,
    BetaBinomialGroupGenderSlideHierarchyInputIdentity, BetaBinomialGroupGenderSlideHierarchySpec,
    BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::{
    BetaBinomialGroupGenderSlideHierarchyArgs, BetaBinomialGroupGenderSlideHierarchyModelArgs,
};
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlideRow {
    slide_id: String,
    patient_id: String,
    group: String,
    gender: String,
    successes: u64,
    trials: u64,
}

pub(super) fn run(args: BetaBinomialGroupGenderSlideHierarchyArgs) -> Result<(), CliError> {
    let BetaBinomialGroupGenderSlideHierarchyArgs { model, out } = args;
    let output_path = out;
    let prepared = prepare(model)?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedBetaBinomialGroupGenderSlideHierarchy {
    pub(crate) request: BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    model: BetaBinomialGroupGenderSlideHierarchyModelArgs,
) -> Result<PreparedBetaBinomialGroupGenderSlideHierarchy, CliError> {
    let BetaBinomialGroupGenderSlideHierarchyModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        reference_gender,
        comparison_gender,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
        patient_log_odds_sd_prior_sd,
        slide_concentration_prior_sd,
        nuts,
        timeout_seconds,
    } = model;
    let sampling = nuts.spec();
    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "beta-binomial group/gender slide input exceeds 16 MiB".into(),
        ));
    }
    let input_bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let slides = reader
        .deserialize::<SlideRow>()
        .map(|row| {
            row.map(|row| BetaBinomialGroupGenderSlideData {
                slide_id: row.slide_id,
                patient_id: row.patient_id,
                group: row.group,
                gender: row.gender,
                successes: row.successes,
                trials: row.trials,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CliError::Input(format!(
                "invalid beta-binomial group/gender slide CSV: {error}"
            ))
        })?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(
        repository,
        "marklab_pymc_beta_binomial_group_gender_slide_hierarchy_worker.py",
    )?;
    let request = BetaBinomialGroupGenderSlideHierarchyWorkerRequest::new(
        BetaBinomialGroupGenderSlideHierarchySpec {
            reference_group,
            comparison_group,
            reference_gender,
            comparison_gender,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            gender_effect_prior_sd,
            patient_log_odds_sd_prior_sd,
            slide_concentration_prior_sd,
            slides,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let mut patient_slides = BTreeMap::<String, (String, String, usize)>::new();
    for slide in &request.slides {
        let patient = patient_slides.entry(slide.patient_id.clone()).or_insert((
            slide.group.clone(),
            slide.gender.clone(),
            0,
        ));
        patient.2 += 1;
    }
    let mut design_cell_patient_counts = BTreeMap::new();
    for (group, gender, _) in patient_slides.values() {
        *design_cell_patient_counts
            .entry(format!("{group}:{gender}"))
            .or_insert(0) += 1;
    }
    let input_identity = BetaBinomialGroupGenderSlideHierarchyInputIdentity {
        path: input_path.display().to_string(),
        slide_data_sha256: json_sha256(&request.slides)?,
        patients: patient_slides.len(),
        slides: request.slides.len(),
        repeated_patients: patient_slides
            .values()
            .filter(|patient| patient.2 >= 2)
            .count(),
        design_cell_patient_counts,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedBetaBinomialGroupGenderSlideHierarchy {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedBetaBinomialGroupGenderSlideHierarchy,
) -> Result<BetaBinomialGroupGenderSlideHierarchyWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_beta_binomial_group_gender_slide_hierarchy_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: BetaBinomialGroupGenderSlideHierarchyWorkerResult =
        serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
