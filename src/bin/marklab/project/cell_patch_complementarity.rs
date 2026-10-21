use std::{borrow::Cow, io, path::PathBuf};

use marklab_bayes::{sha256_hex, WorkerBackend};
use marklab_embeddings::analysis::{
    expected_model_features, CellPatchComplementaritySpec, CellPatchComplementarityWorkerRequest,
    CellPatchComplementarityWorkerResult, ComplementarityFeatureNames, ComplementarityModelResult,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::TestCellPatchComplementarityProjectArgs;
use super::{
    bayes,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};
use crate::bayes::complementarity::{self, IncrementComparison};

const INPUT_KIND: &str = "application/vnd.marklab.source.cell-patch-complementarity-csv;version=1";
const LOCK_KIND: &str = "application/vnd.marklab.python-lock;version=1";
const WORKER_KIND: &str = "application/vnd.marklab.python-worker;version=1";
const WORKER_NAME: &str = "marklab_scipy_cell_patch_complementarity_worker.py";

pub(super) fn run(args: TestCellPatchComplementarityProjectArgs) -> Result<(), CliError> {
    let TestCellPatchComplementarityProjectArgs {
        project: project_path,
        input: input_path,
        outer_folds,
        inner_folds,
        ridge_alphas,
        permutations,
        seed,
        timeout_seconds,
        memory_budget_mib,
        out: output_path,
    } = args;
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if memory_bytes == 0 {
        return Err(CliError::Input("memory budget must be positive".into()));
    }
    const LABEL: &str = "cell-patch complementarity sources";
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python").join(WORKER_NAME);
    let input_bytes = read_bounded(&input_path, memory_bytes, "input")?;
    let lock_bytes = read_bounded(&lock_path, memory_bytes, "environment lock")?;
    let worker_bytes = read_bounded(&worker_path, memory_bytes, "worker")?;
    let source_bytes = input_bytes
        .len()
        .checked_add(lock_bytes.len())
        .and_then(|value| value.checked_add(worker_bytes.len()))
        .ok_or_else(|| CliError::Input("complementarity source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(CliError::Input(
            "complementarity sources exceed the retained-memory budget".into(),
        ));
    }
    let sources = vec![
        Source::from_bytes(input_path, INPUT_KIND, &input_bytes)?,
        Source::from_bytes(lock_path, LOCK_KIND, &lock_bytes)?,
        Source::from_bytes(worker_path, WORKER_KIND, &worker_bytes)?,
    ];
    let (patients, feature_names) = complementarity::read_patients(&input_bytes)?;
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
    )
    .map_err(map_bayes_error)?;
    preflight_permutations(request.patients.len(), permutations)?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let retained = retained_bytes(source_bytes, &request, request_bytes.len())?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "cell-patch complementarity retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    let output_estimate = output_bytes_upper_bound(&request)?;
    if output_estimate > MAXIMUM_RESULT_BYTES {
        return Err(CliError::Input(format!(
            "cell-patch complementarity output estimate {output_estimate} exceeds durable result limit {MAXIMUM_RESULT_BYTES}"
        )));
    }
    let peak = retained
        .checked_add(output_estimate)
        .ok_or_else(|| CliError::Input("complementarity peak memory overflowed".into()))?;
    if peak > memory_bytes {
        return Err(CliError::Input(format!(
            "cell-patch complementarity peak-memory estimate {peak} exceeds budget {memory_bytes}"
        )));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-cell-patch-complementarity-configuration-v1".as_slice(),
        request_sha256.as_bytes(),
        permutations.to_string().as_bytes(),
        seed.to_string().as_bytes(),
        memory_budget_mib.to_string().as_bytes(),
    ]);
    let execution_policy = format!(
        "bounded-process;backend=scipy@{};python={};nested-patient-held-out-preprocessing-and-tuning;outer_folds={};inner_folds={};permutations={permutations};seed={seed};timeout_seconds={};memory_budget_mib={memory_budget_mib}",
        request.backend.version,
        request.backend.python_version,
        request.outer_folds,
        request.inner_folds,
        request.resources.timeout_seconds,
    );
    let prepared = Prepared {
        repository,
        request,
        request_bytes,
        request_sha256,
        input_sha256: sha256_hex(&input_bytes),
        permutations,
        seed,
    };
    let contract = NodeContract {
        id: "scipy-cell-patch-complementarity",
        kind: "scipy_cell_patch_complementarity",
        version: 1,
        configuration_digest,
        execution_policy: Cow::Owned(execution_policy.into_bytes()),
        implementation_identity: "marklab-project-scipy-cell-patch-complementarity-node-v1",
        output_kind: "application/vnd.marklab.cell-patch-complementarity+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        execute,
        |prepared, output| validate(prepared, output).map_err(NodeError::decode),
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.cell_patch_complementarity", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "test-cell-patch-complementarity",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn read_bounded(
    path: &std::path::Path,
    memory_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "complementarity {label} must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

fn preflight_permutations(patients: usize, permutations: usize) -> Result<(), CliError> {
    let work = patients
        .checked_mul(permutations)
        .and_then(|value| value.checked_mul(6))
        .ok_or_else(|| CliError::Input("complementarity permutation work overflowed".into()))?;
    if permutations == 0 || permutations > 1_000_000 || work > 600_000_000 {
        return Err(CliError::Input(
            "complementarity permutations exceed the paired-patient resource bound".into(),
        ));
    }
    Ok(())
}

fn feature_count(names: &ComplementarityFeatureNames) -> usize {
    [
        names.technical.len(),
        names.clinical.len(),
        names.compartment.len(),
        names.acquisition.len(),
        names.cell.len(),
        names.patch.len(),
        names.neighbor.len(),
        names.measured.len(),
    ]
    .into_iter()
    .sum()
}

fn retained_bytes(
    source_bytes: usize,
    request: &CellPatchComplementarityWorkerRequest,
    request_bytes: usize,
) -> Result<usize, CliError> {
    let values = request
        .patients
        .len()
        .checked_mul(feature_count(&request.feature_names))
        .and_then(|value| value.checked_mul(2 * std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("complementarity value memory overflowed".into()))?;
    source_bytes
        .checked_add(request_bytes.saturating_mul(2))
        .and_then(|value| value.checked_add(values))
        .and_then(|value| value.checked_add(request.patients.len().saturating_mul(1024)))
        .and_then(|value| value.checked_add(feature_count(&request.feature_names) * 256))
        .ok_or_else(|| CliError::Input("complementarity memory estimate overflowed".into()))
}

fn output_bytes_upper_bound(
    request: &CellPatchComplementarityWorkerRequest,
) -> Result<usize, CliError> {
    let patient_id_bytes = request
        .patients
        .iter()
        .try_fold(0_usize, |total, patient| {
            total.checked_add(patient.patient_id.len())
        })
        .ok_or_else(|| CliError::Input("patient identity size overflowed".into()))?;
    let model_features = expected_model_features(&request.feature_names);
    let model_feature_count = model_features
        .iter()
        .map(|(_, names)| names.len())
        .sum::<usize>();
    let model_feature_bytes = model_features
        .iter()
        .flat_map(|(_, names)| names)
        .try_fold(0_usize, |total, name| total.checked_add(name.len()))
        .ok_or_else(|| CliError::Input("model feature identity size overflowed".into()))?;
    (64 * 1024_usize)
        .checked_add(request.patients.len().saturating_mul(6 * 256))
        .and_then(|value| value.checked_add(patient_id_bytes.saturating_mul(6)))
        .and_then(|value| value.checked_add(request.outer_folds as usize * 6 * 256))
        .and_then(|value| value.checked_add(model_feature_count.saturating_mul(96)))
        .and_then(|value| value.checked_add(model_feature_bytes))
        .ok_or_else(|| CliError::Input("complementarity output estimate overflowed".into()))
}

struct Prepared {
    repository: PathBuf,
    request: CellPatchComplementarityWorkerRequest,
    request_bytes: Vec<u8>,
    request_sha256: String,
    input_sha256: String,
    permutations: usize,
    seed: u64,
}

fn execute(prepared: &Prepared) -> Result<Output, NodeError> {
    let result_bytes = bayes::run_worker(
        &prepared.repository,
        WORKER_NAME,
        &prepared.request_bytes,
        prepared.request.resources.timeout_seconds,
    )
    .map_err(|error| NodeError::execution(invalid_owned(error.to_string())))?;
    let worker: CellPatchComplementarityWorkerResult =
        serde_json::from_slice(&result_bytes).map_err(NodeError::execution)?;
    worker
        .validate(&prepared.request, &prepared.request_sha256)
        .map_err(|error| NodeError::execution(invalid_owned(error.to_string())))?;
    let comparisons =
        complementarity::comparisons(&worker.models, prepared.permutations, prepared.seed)
            .map_err(|error| NodeError::execution(invalid_owned(error)))?;
    Ok(Output {
        format: "marklab.cell_patch_complementarity".into(),
        version: 1,
        backend: worker.backend,
        input_sha256: prepared.input_sha256.clone(),
        patient_count: prepared.request.patients.len() as u32,
        outer_folds: prepared.request.outer_folds,
        inner_folds: prepared.request.inner_folds,
        split_policy: "nested_patient_held_out_preprocessing_and_tuning".into(),
        claim_status: "experimental_synthetic_predictive_design".into(),
        models: worker.models,
        comparisons,
        request_sha256: worker.request_sha256,
    })
}

fn validate(prepared: &Prepared, output: &Output) -> io::Result<()> {
    let request = &prepared.request;
    if output.format != "marklab.cell_patch_complementarity"
        || output.version != 1
        || output.backend.name != "scipy"
        || output.backend.version != request.backend.version
        || output.backend.python_version != request.backend.python_version
        || output.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
        || output.backend.worker_sha256 != request.backend.worker_sha256
        || output.input_sha256 != prepared.input_sha256
        || output.patient_count as usize != request.patients.len()
        || output.outer_folds != request.outer_folds
        || output.inner_folds != request.inner_folds
        || output.split_policy != "nested_patient_held_out_preprocessing_and_tuning"
        || output.claim_status != "experimental_synthetic_predictive_design"
        || output.request_sha256 != prepared.request_sha256
        || output.models.len() != 6
        || output.comparisons.len() != 6
    {
        return Err(invalid(
            "decoded cell-patch complementarity identity differs",
        ));
    }
    let worker_bytes = serde_json::to_vec(&WorkerValidationView {
        format: "marklab.scipy_cell_patch_complementarity_worker_result",
        version: 1,
        backend: &output.backend,
        request_sha256: &output.request_sha256,
        models: &output.models,
    })
    .map_err(|error| invalid_owned(error.to_string()))?;
    let worker: CellPatchComplementarityWorkerResult =
        serde_json::from_slice(&worker_bytes).map_err(|error| invalid_owned(error.to_string()))?;
    worker
        .validate(request, &prepared.request_sha256)
        .map_err(|error| invalid_owned(error.to_string()))?;
    let expected =
        complementarity::comparisons(&output.models, prepared.permutations, prepared.seed)
            .map_err(invalid_owned)?;
    for (actual, expected) in output.comparisons.iter().zip(expected) {
        if actual.comparison_id != expected.comparison_id
            || actual.base_model != expected.base_model
            || actual.expanded_model != expected.expanded_model
            || actual.pair_count != expected.pair_count
            || actual
                .mean_absolute_error_effect_expanded_minus_base
                .to_bits()
                != expected
                    .mean_absolute_error_effect_expanded_minus_base
                    .to_bits()
            || actual.studentized_statistic.to_bits() != expected.studentized_statistic.to_bits()
            || actual.p_value_two_sided.to_bits() != expected.p_value_two_sided.to_bits()
            || actual.permutations != expected.permutations
            || actual.seed != expected.seed
        {
            return Err(invalid("decoded complementarity comparison differs"));
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct WorkerValidationView<'a> {
    format: &'static str,
    version: u32,
    backend: &'a WorkerBackend,
    request_sha256: &'a str,
    models: &'a [ComplementarityModelResult],
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    format: String,
    version: u32,
    backend: WorkerBackend,
    input_sha256: String,
    patient_count: u32,
    outer_folds: u32,
    inner_folds: u32,
    split_policy: String,
    claim_status: String,
    models: Vec<ComplementarityModelResult>,
    comparisons: Vec<IncrementComparison>,
    request_sha256: String,
}

fn map_bayes_error(error: marklab_bayes::BayesError) -> CliError {
    CliError::Input(error.to_string())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
