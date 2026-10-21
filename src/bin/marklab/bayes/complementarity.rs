use std::fs;

use marklab_bayes::{sha256_hex, WorkerBackend};
use marklab_cohort::{
    paired_patient_permutation_test, PairedPatientEndpoint, PairedPatientPermutationSpec,
    PermutationAlternative,
};
use marklab_embeddings::analysis::{
    CellPatchComplementaritySpec, CellPatchComplementarityWorkerRequest,
    CellPatchComplementarityWorkerResult, ComplementarityFeatureNames, ComplementarityModelResult,
    ComplementarityPatientRow,
};
use serde::{Deserialize, Serialize};

use super::cli_schema::TestCellPatchComplementarityArgs;
use super::{embedding_spatial, publish_json, run_worker, CliError};

pub(super) fn run(args: TestCellPatchComplementarityArgs) -> Result<(), CliError> {
    run_with_output(
        args,
        "marklab.cell_patch_complementarity",
        "experimental_synthetic_predictive_design",
    )
}

pub(crate) fn run_multimodal_comparison(
    args: TestCellPatchComplementarityArgs,
) -> Result<(), CliError> {
    run_with_output(
        args,
        "marklab.multimodal_model_comparison",
        "experimental_synthetic_multimodal_comparison",
    )
}

fn run_with_output(
    args: TestCellPatchComplementarityArgs,
    output_format: &'static str,
    claim_status: &'static str,
) -> Result<(), CliError> {
    let TestCellPatchComplementarityArgs {
        input,
        outer_folds,
        inner_folds,
        ridge_alphas,
        permutations,
        seed,
        timeout_seconds,
        out,
    } = args;

    let input_bytes = embedding_spatial::read(&input)?;
    let (patients, feature_names) = read_patients(&input_bytes)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| CliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_name = "marklab_scipy_cell_patch_complementarity_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let request = CellPatchComplementarityWorkerRequest::new(
        CellPatchComplementaritySpec {
            patients,
            feature_names,
            outer_folds,
            inner_folds,
            ridge_alphas,
            timeout_seconds,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let worker: CellPatchComplementarityWorkerResult = serde_json::from_slice(&result_bytes)?;
    worker.validate(&request, &request_sha256)?;
    let comparisons = comparisons(&worker.models, permutations, seed).map_err(CliError::Backend)?;
    publish_json(
        &out,
        &Output {
            format: output_format,
            version: 1,
            backend: worker.backend,
            input_sha256: sha256_hex(&input_bytes),
            patient_count: request.patients.len() as u32,
            outer_folds: request.outer_folds,
            inner_folds: request.inner_folds,
            split_policy: "nested_patient_held_out_preprocessing_and_tuning",
            claim_status,
            models: worker.models,
            comparisons,
            request_sha256: worker.request_sha256,
        },
    )
}

pub(crate) fn read_patients(
    bytes: &[u8],
) -> Result<(Vec<ComplementarityPatientRow>, ComplementarityFeatureNames), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 12
        || headers.iter().take(4).collect::<Vec<_>>()
            != ["patient_id", "outer_fold", "inner_fold", "target"]
    {
        return Err(CliError::Input(
            "complementarity input requires patient/fold/target prefix and all feature groups"
                .into(),
        ));
    }
    let prefixes = [
        "technical_",
        "clinical_",
        "compartment_",
        "acquisition_",
        "cell_",
        "patch_",
        "neighbor_",
        "measured_",
    ];
    let indices = prefixes.map(|prefix| {
        headers
            .iter()
            .enumerate()
            .skip(4)
            .filter_map(|(index, name)| name.starts_with(prefix).then_some(index))
            .collect::<Vec<_>>()
    });
    if indices.iter().any(Vec::is_empty) || indices.iter().flatten().count() != headers.len() - 4 {
        return Err(CliError::Input(
            "every complementarity feature must belong to one required prefix group".into(),
        ));
    }
    let names = |group: usize| {
        indices[group]
            .iter()
            .map(|index| headers[*index].to_owned())
            .collect::<Vec<_>>()
    };
    let feature_names = ComplementarityFeatureNames {
        technical: names(0),
        clinical: names(1),
        compartment: names(2),
        acquisition: names(3),
        cell: names(4),
        patch: names(5),
        neighbor: names(6),
        measured: names(7),
    };
    let mut patients = Vec::new();
    for record in reader.records() {
        let record = record?;
        let values = |group: usize| {
            indices[group]
                .iter()
                .map(|index| {
                    record[*index].parse::<f64>().map_err(|_| {
                        CliError::Input(format!(
                            "complementarity feature {} is invalid",
                            &headers[*index]
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        };
        patients.push(ComplementarityPatientRow {
            patient_id: record[0].to_owned(),
            outer_fold: record[1]
                .parse()
                .map_err(|_| CliError::Input("outer fold is invalid".into()))?,
            inner_fold: record[2]
                .parse()
                .map_err(|_| CliError::Input("inner fold is invalid".into()))?,
            target: record[3]
                .parse()
                .map_err(|_| CliError::Input("target is invalid".into()))?,
            technical: values(0)?,
            clinical: values(1)?,
            compartment: values(2)?,
            acquisition: values(3)?,
            cell: values(4)?,
            patch: values(5)?,
            neighbor: values(6)?,
            measured: values(7)?,
        });
    }
    Ok((patients, feature_names))
}

/// One nested-model increment and its paired permutation test.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IncrementComparison {
    pub(crate) comparison_id: String,
    pub(crate) base_model: String,
    pub(crate) expanded_model: String,
    pub(crate) pair_count: usize,
    pub(crate) mean_absolute_error_effect_expanded_minus_base: f64,
    pub(crate) studentized_statistic: f64,
    pub(crate) p_value_two_sided: f64,
    pub(crate) permutations: usize,
    pub(crate) seed: u64,
}

/// The six nested-model increments, each compared by a paired patient permutation test of
/// absolute prediction error, with a per-comparison seed derived from `seed`.
pub(crate) fn comparisons(
    models: &[ComplementarityModelResult],
    permutations: usize,
    seed: u64,
) -> Result<Vec<IncrementComparison>, String> {
    [
        ("baseline_plus_cell", "baseline"),
        ("baseline_plus_patch", "baseline"),
        ("baseline_plus_cell_patch", "baseline_plus_cell"),
        ("baseline_plus_cell_patch", "baseline_plus_patch"),
        (
            "baseline_plus_cell_patch_neighbor",
            "baseline_plus_cell_patch",
        ),
        (
            "baseline_plus_cell_patch_neighbor_measured",
            "baseline_plus_cell_patch_neighbor",
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (expanded_id, base_id))| {
        let base = models
            .iter()
            .find(|model| model.model_id == base_id)
            .ok_or("complementarity base model is missing")?;
        let expanded = models
            .iter()
            .find(|model| model.model_id == expanded_id)
            .ok_or("complementarity expanded model is missing")?;
        let mut records = Vec::with_capacity(base.predictions.len() * 2);
        for (base_prediction, expanded_prediction) in
            base.predictions.iter().zip(&expanded.predictions)
        {
            records.push(PairedPatientEndpoint {
                patient_id: base_prediction.patient_id.clone(),
                condition: base_id.into(),
                endpoint: (base_prediction.predicted - base_prediction.observed).abs(),
            });
            records.push(PairedPatientEndpoint {
                patient_id: expanded_prediction.patient_id.clone(),
                condition: expanded_id.into(),
                endpoint: (expanded_prediction.predicted - expanded_prediction.observed).abs(),
            });
        }
        let comparison_seed = seed.wrapping_add(index as u64);
        let result = paired_patient_permutation_test(
            &records,
            &PairedPatientPermutationSpec {
                condition_a: base_id.into(),
                condition_b: expanded_id.into(),
                permutations,
                seed: comparison_seed,
                alternative: PermutationAlternative::TwoSided,
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(IncrementComparison {
            comparison_id: format!("{expanded_id}_minus_{base_id}"),
            base_model: base_id.into(),
            expanded_model: expanded_id.into(),
            pair_count: result.pair_count,
            mean_absolute_error_effect_expanded_minus_base: result
                .effect_condition_b_minus_condition_a,
            studentized_statistic: result.studentized_statistic,
            p_value_two_sided: result.p_value,
            permutations,
            seed: comparison_seed,
        })
    })
    .collect()
}

#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    backend: WorkerBackend,
    input_sha256: String,
    patient_count: u32,
    outer_folds: u32,
    inner_folds: u32,
    split_policy: &'static str,
    claim_status: &'static str,
    models: Vec<ComplementarityModelResult>,
    comparisons: Vec<IncrementComparison>,
    request_sha256: String,
}
