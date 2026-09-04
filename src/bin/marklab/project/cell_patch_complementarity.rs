use std::{fs, io, path::PathBuf};

use marklab_bayes::{
    expected_model_features, sha256_hex, CellPatchComplementaritySpec,
    CellPatchComplementarityWorkerRequest, CellPatchComplementarityWorkerResult,
    ComplementarityFeatureNames, ComplementarityModelResult, ComplementarityPatientRow,
    WorkerBackend,
};
use marklab_cohort::{
    paired_patient_permutation_test, PairedPatientEndpoint, PairedPatientPermutationSpec,
    PermutationAlternative,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.cell-patch-complementarity-csv;version=1";
const LOCK_KIND: &str = "application/vnd.marklab.python-lock;version=1";
const WORKER_KIND: &str = "application/vnd.marklab.python-worker;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cell-patch-complementarity+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-scipy-cell-patch-complementarity-node-v1";
const WORKER_NAME: &str = "marklab_scipy_cell_patch_complementarity_worker.py";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    outer_folds: u32,
    inner_folds: u32,
    ridge_alphas: String,
    permutations: usize,
    seed: u64,
    timeout_seconds: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if memory_bytes == 0 {
        return Err(BayesCliError::Input(
            "memory budget must be positive".into(),
        ));
    }
    let repository = marklab::python_backend_assets_root()?;
    let paths = SourcePaths {
        input: input_path,
        lock: repository.join("workers/python/uv.lock"),
        worker: repository.join("workers/python").join(WORKER_NAME),
    };
    let input_bytes = read_bounded(&paths.input, memory_bytes, "input")?;
    let lock_bytes = read_bounded(&paths.lock, memory_bytes, "environment lock")?;
    let worker_bytes = read_bounded(&paths.worker, memory_bytes, "worker")?;
    let source_bytes = input_bytes
        .len()
        .checked_add(lock_bytes.len())
        .and_then(|value| value.checked_add(worker_bytes.len()))
        .ok_or_else(|| {
            BayesCliError::Input("complementarity source-byte total overflowed".into())
        })?;
    if source_bytes > memory_bytes {
        return Err(BayesCliError::Input(
            "complementarity sources exceed the retained-memory budget".into(),
        ));
    }
    let prepared_artifacts = source_artifacts_from_bytes(&input_bytes, &lock_bytes, &worker_bytes)?;
    let (patients, feature_names) = read_patients(&input_bytes)?;
    let alphas = parse_alphas(&ridge_alphas)?;
    let request = CellPatchComplementarityWorkerRequest::new(
        CellPatchComplementaritySpec {
            patients,
            feature_names,
            outer_folds,
            inner_folds,
            ridge_alphas: alphas,
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
        return Err(BayesCliError::Input(format!(
            "cell-patch complementarity retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    let output_estimate = output_bytes_upper_bound(&request)?;
    if output_estimate > MAXIMUM_RESULT_BYTES {
        return Err(BayesCliError::Input(format!(
            "cell-patch complementarity output estimate {output_estimate} exceeds durable result limit {MAXIMUM_RESULT_BYTES}"
        )));
    }
    let peak = retained
        .checked_add(output_estimate)
        .ok_or_else(|| BayesCliError::Input("complementarity peak memory overflowed".into()))?;
    if peak > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "cell-patch complementarity peak-memory estimate {peak} exceeds budget {memory_bytes}"
        )));
    }
    if prepared_artifacts != source_artifacts(&paths)? {
        return Err(BayesCliError::Input(
            "cell-patch complementarity sources changed while prepared".into(),
        ));
    }

    let prepared = Prepared {
        repository,
        request,
        request_bytes,
        request_sha256,
        input_sha256: prepared_artifacts[0].digest().to_string(),
        permutations,
        seed,
    };
    let node = CellPatchComplementarityNode::new(
        paths,
        prepared_artifacts.clone(),
        prepared,
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
    for artifact in &prepared_artifacts {
        project
            .register_reference(artifact.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
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
        ArtifactSchema::new("marklab.cell_patch_complementarity", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project test-cell-patch-complementarity cache_status={cache_status}");
    Ok(())
}

#[derive(Clone)]
struct SourcePaths {
    input: PathBuf,
    lock: PathBuf,
    worker: PathBuf,
}

fn source_artifacts(paths: &SourcePaths) -> Result<Vec<ArtifactRef>, BayesCliError> {
    Ok(vec![
        source_artifact(&paths.input, INPUT_KIND)?,
        source_artifact(&paths.lock, LOCK_KIND)?,
        source_artifact(&paths.worker, WORKER_KIND)?,
    ])
}

fn source_artifacts_from_bytes(
    input: &[u8],
    lock: &[u8],
    worker: &[u8],
) -> Result<Vec<ArtifactRef>, BayesCliError> {
    [
        (INPUT_KIND, input),
        (LOCK_KIND, lock),
        (WORKER_KIND, worker),
    ]
    .into_iter()
    .map(|(kind, bytes)| {
        ArtifactRef::from_bytes(kind, bytes)
            .map_err(|error| BayesCliError::Input(error.to_string()))
    })
    .collect()
}

fn read_bounded(
    path: &std::path::Path,
    memory_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(BayesCliError::Input(format!(
            "complementarity {label} must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

fn read_patients(
    bytes: &[u8],
) -> Result<(Vec<ComplementarityPatientRow>, ComplementarityFeatureNames), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 12
        || headers.iter().take(4).collect::<Vec<_>>()
            != ["patient_id", "outer_fold", "inner_fold", "target"]
    {
        return Err(BayesCliError::Input(
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
        return Err(BayesCliError::Input(
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
                        BayesCliError::Input(format!(
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
                .map_err(|_| BayesCliError::Input("outer fold is invalid".into()))?,
            inner_fold: record[2]
                .parse()
                .map_err(|_| BayesCliError::Input("inner fold is invalid".into()))?,
            target: record[3]
                .parse()
                .map_err(|_| BayesCliError::Input("target is invalid".into()))?,
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

fn parse_alphas(values: &str) -> Result<Vec<f64>, BayesCliError> {
    values
        .split(',')
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| BayesCliError::Input("ridge alpha grid is invalid".into()))
        })
        .collect()
}

fn preflight_permutations(patients: usize, permutations: usize) -> Result<(), BayesCliError> {
    let work = patients
        .checked_mul(permutations)
        .and_then(|value| value.checked_mul(6))
        .ok_or_else(|| {
            BayesCliError::Input("complementarity permutation work overflowed".into())
        })?;
    if permutations == 0 || permutations > 1_000_000 || work > 600_000_000 {
        return Err(BayesCliError::Input(
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
) -> Result<usize, BayesCliError> {
    let values = request
        .patients
        .len()
        .checked_mul(feature_count(&request.feature_names))
        .and_then(|value| value.checked_mul(2 * std::mem::size_of::<f64>()))
        .ok_or_else(|| BayesCliError::Input("complementarity value memory overflowed".into()))?;
    source_bytes
        .checked_add(request_bytes.saturating_mul(2))
        .and_then(|value| value.checked_add(values))
        .and_then(|value| value.checked_add(request.patients.len().saturating_mul(1024)))
        .and_then(|value| value.checked_add(feature_count(&request.feature_names) * 256))
        .ok_or_else(|| BayesCliError::Input("complementarity memory estimate overflowed".into()))
}

fn output_bytes_upper_bound(
    request: &CellPatchComplementarityWorkerRequest,
) -> Result<usize, BayesCliError> {
    let patient_id_bytes = request
        .patients
        .iter()
        .try_fold(0_usize, |total, patient| {
            total.checked_add(patient.patient_id.len())
        })
        .ok_or_else(|| BayesCliError::Input("patient identity size overflowed".into()))?;
    let model_features = expected_model_features(&request.feature_names);
    let model_feature_count = model_features
        .iter()
        .map(|(_, names)| names.len())
        .sum::<usize>();
    let model_feature_bytes = model_features
        .iter()
        .flat_map(|(_, names)| names)
        .try_fold(0_usize, |total, name| total.checked_add(name.len()))
        .ok_or_else(|| BayesCliError::Input("model feature identity size overflowed".into()))?;
    (64 * 1024_usize)
        .checked_add(request.patients.len().saturating_mul(6 * 256))
        .and_then(|value| value.checked_add(patient_id_bytes.saturating_mul(6)))
        .and_then(|value| value.checked_add(request.outer_folds as usize * 6 * 256))
        .and_then(|value| value.checked_add(model_feature_count.saturating_mul(96)))
        .and_then(|value| value.checked_add(model_feature_bytes))
        .ok_or_else(|| BayesCliError::Input("complementarity output estimate overflowed".into()))
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

struct CellPatchComplementarityNode {
    spec: NodeSpec,
    paths: SourcePaths,
    input_artifacts: Vec<ArtifactRef>,
    prepared: Prepared,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl CellPatchComplementarityNode {
    fn new(
        paths: SourcePaths,
        input_artifacts: Vec<ArtifactRef>,
        prepared: Prepared,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != 3
            || prepared.input_sha256 != input_artifacts[0].digest().to_string()
            || prepared.request.backend.environment_lock_sha256
                != input_artifacts[1].digest().to_string()
            || prepared.request.backend.worker_sha256 != input_artifacts[2].digest().to_string()
        {
            return Err(BayesCliError::Input(
                "cell-patch complementarity parsed/backend identities differ from durable sources"
                    .into(),
            ));
        }
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cell-patch-complementarity-configuration-v1".as_slice(),
            prepared.request_sha256.as_bytes(),
            prepared.permutations.to_string().as_bytes(),
            prepared.seed.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "bounded-process;backend=scipy@{};python={};nested-patient-held-out-preprocessing-and-tuning;outer_folds={};inner_folds={};permutations={};seed={};timeout_seconds={};memory_budget_mib={memory_budget_mib}",
            prepared.request.backend.version,
            prepared.request.backend.python_version,
            prepared.request.outer_folds,
            prepared.request.inner_folds,
            prepared.permutations,
            prepared.seed,
            prepared.request.resources.timeout_seconds,
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("scipy-cell-patch-complementarity")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "scipy_cell_patch_complementarity",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            paths,
            input_artifacts,
            prepared,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        let request = &self.prepared.request;
        if output.format != "marklab.cell_patch_complementarity"
            || output.version != 1
            || output.backend.name != "scipy"
            || output.backend.version != request.backend.version
            || output.backend.python_version != request.backend.python_version
            || output.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || output.backend.worker_sha256 != request.backend.worker_sha256
            || output.input_sha256 != self.prepared.input_sha256
            || output.patient_count as usize != request.patients.len()
            || output.outer_folds != request.outer_folds
            || output.inner_folds != request.inner_folds
            || output.split_policy != "nested_patient_held_out_preprocessing_and_tuning"
            || output.claim_status != "experimental_synthetic_predictive_design"
            || output.request_sha256 != self.prepared.request_sha256
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
        let worker: CellPatchComplementarityWorkerResult = serde_json::from_slice(&worker_bytes)
            .map_err(|error| invalid_owned(error.to_string()))?;
        worker
            .validate(request, &self.prepared.request_sha256)
            .map_err(|error| invalid_owned(error.to_string()))?;
        let expected = comparisons(
            &output.models,
            self.prepared.permutations,
            self.prepared.seed,
        )?;
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
                || actual.studentized_statistic.to_bits()
                    != expected.studentized_statistic.to_bits()
                || actual.p_value_two_sided.to_bits() != expected.p_value_two_sided.to_bits()
                || actual.permutations != expected.permutations
                || actual.seed != expected.seed
            {
                return Err(invalid("decoded complementarity comparison differs"));
            }
        }
        Ok(())
    }
}

impl WorkflowNode for CellPatchComplementarityNode {
    type Output = Output;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = source_artifacts(&self.paths).map_err(NodeError::input)?;
        if current != self.input_artifacts {
            return Err(NodeError::input(invalid(
                "cell-patch complementarity source identity changed",
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
        let result_bytes = bayes::run_worker(
            &self.prepared.repository,
            WORKER_NAME,
            &self.prepared.request_bytes,
            self.prepared.request.resources.timeout_seconds,
        )
        .map_err(|error| NodeError::execution(invalid_owned(error.to_string())))?;
        let worker: CellPatchComplementarityWorkerResult =
            serde_json::from_slice(&result_bytes).map_err(NodeError::execution)?;
        worker
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(|error| NodeError::execution(invalid_owned(error.to_string())))?;
        let comparisons = comparisons(
            &worker.models,
            self.prepared.permutations,
            self.prepared.seed,
        )
        .map_err(NodeError::execution)?;
        Ok(Output {
            format: "marklab.cell_patch_complementarity".into(),
            version: 1,
            backend: worker.backend,
            input_sha256: self.prepared.input_sha256.clone(),
            patient_count: self.prepared.request.patients.len() as u32,
            outer_folds: self.prepared.request.outer_folds,
            inner_folds: self.prepared.request.inner_folds,
            split_policy: "nested_patient_held_out_preprocessing_and_tuning".into(),
            claim_status: "experimental_synthetic_predictive_design".into(),
            models: worker.models,
            comparisons,
            request_sha256: worker.request_sha256,
        })
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: Output = marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

fn comparisons(
    models: &[ComplementarityModelResult],
    permutations: usize,
    seed: u64,
) -> Result<Vec<IncrementComparison>, io::Error> {
    [
        ("m1", "m0"),
        ("m2", "m0"),
        ("m3", "m1"),
        ("m3", "m2"),
        ("m4", "m3"),
        ("m5", "m4"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (expanded_id, base_id))| {
        let base = models
            .iter()
            .find(|model| model.model_id == base_id)
            .ok_or_else(|| invalid("complementarity base model is missing"))?;
        let expanded = models
            .iter()
            .find(|model| model.model_id == expanded_id)
            .ok_or_else(|| invalid("complementarity expanded model is missing"))?;
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
        .map_err(|error| invalid_owned(error.to_string()))?;
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IncrementComparison {
    comparison_id: String,
    base_model: String,
    expanded_model: String,
    pair_count: usize,
    mean_absolute_error_effect_expanded_minus_base: f64,
    studentized_statistic: f64,
    p_value_two_sided: f64,
    permutations: usize,
    seed: u64,
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

fn map_bayes_error(error: marklab_bayes::BayesError) -> BayesCliError {
    BayesCliError::Input(error.to_string())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
