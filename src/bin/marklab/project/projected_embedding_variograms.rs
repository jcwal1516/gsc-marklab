use std::{io, path::PathBuf};

use marklab_bayes::{EmbeddingDistanceBin, ProjectedSplitCurve, ProjectionArtifact, WorkerBackend};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.projected-embedding-points-csv;version=1";
const BINS_KIND: &str = "application/vnd.marklab.source.distance-bins-csv;version=1";
const LOCK_KIND: &str = "application/vnd.marklab.python-lock;version=1";
const WORKER_KIND: &str = "application/vnd.marklab.python-worker;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.projected-embedding-variograms+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-scipy-projected-embedding-variograms-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input: PathBuf,
    bins: PathBuf,
    components: u32,
    permutations: u32,
    seed: u64,
    maximum_pair_visits: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let repository = marklab::python_backend_assets_root()?;
    let lock = repository.join("workers/python/uv.lock");
    let worker = repository.join("workers/python/marklab_scipy_projected_variograms_worker.py");
    let paths = [input.clone(), bins.clone(), lock, worker];
    let before = source_artifacts(&paths)?;
    let prepared = bayes::embedding_spatial::prepare_projected_variograms(
        input,
        bins,
        components,
        permutations,
        seed,
        maximum_pair_visits,
        timeout_seconds,
    )?;
    let after = source_artifacts(&paths)?;
    if before != after {
        return Err(BayesCliError::Input(
            "projected variogram sources changed while prepared".into(),
        ));
    }
    validate_prepared(&prepared, &after)?;
    let node = ProjectedEmbeddingVariogramsNode::new(paths, after.clone(), prepared)?;
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
    for artifact in &after {
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
        ArtifactSchema::new("marklab.projected_embedding_variograms", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project projected-embedding-variograms cache_status={cache_status}");
    Ok(())
}

fn source_artifacts(paths: &[PathBuf; 4]) -> Result<Vec<ArtifactRef>, BayesCliError> {
    [INPUT_KIND, BINS_KIND, LOCK_KIND, WORKER_KIND]
        .into_iter()
        .zip(paths)
        .map(|(kind, path)| source_artifact(path, kind))
        .collect()
}

fn validate_prepared(
    prepared: &bayes::embedding_spatial::PreparedProjectedVariograms,
    artifacts: &[ArtifactRef],
) -> Result<(), BayesCliError> {
    if artifacts.len() != 4
        || prepared.input_sha256 != artifacts[0].digest().to_string()
        || prepared.bins_sha256 != artifacts[1].digest().to_string()
        || prepared.request.backend.environment_lock_sha256 != artifacts[2].digest().to_string()
        || prepared.request.backend.worker_sha256 != artifacts[3].digest().to_string()
    {
        return Err(BayesCliError::Input(
            "projected variogram parsed/backend identities differ from durable sources".into(),
        ));
    }
    Ok(())
}

struct ProjectedEmbeddingVariogramsNode {
    spec: NodeSpec,
    paths: [PathBuf; 4],
    input_artifacts: Vec<ArtifactRef>,
    prepared: bayes::embedding_spatial::PreparedProjectedVariograms,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl ProjectedEmbeddingVariogramsNode {
    fn new(
        paths: [PathBuf; 4],
        input_artifacts: Vec<ArtifactRef>,
        prepared: bayes::embedding_spatial::PreparedProjectedVariograms,
    ) -> Result<Self, BayesCliError> {
        let request = &prepared.request;
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-projected-embedding-variograms-configuration-v1".as_slice(),
            prepared.request_sha256.as_bytes(),
            request.components.to_string().as_bytes(),
            request.permutations.to_string().as_bytes(),
            request.seed.to_string().as_bytes(),
            request.resources.maximum_pair_visits.to_string().as_bytes(),
            prepared.timeout_seconds.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "bounded-process;backend=scipy@{};python={};training-only-pca;complete-row-within-stratum;single-step-max-t;components={};permutations={};seed={};maximum_pair_visits={};timeout_seconds={}",
            request.backend.version,
            request.backend.python_version,
            request.components,
            request.permutations,
            request.seed,
            request.resources.maximum_pair_visits,
            prepared.timeout_seconds,
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("scipy-projected-embedding-variograms")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "scipy_projected_embedding_variograms",
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

    fn validate(&self, document: &ProjectedEmbeddingVariogramDocument) -> io::Result<()> {
        let request = &self.prepared.request;
        if document.format != "marklab.projected_embedding_variograms"
            || document.version != 1
            || document.backend.name != "scipy"
            || document.backend.version != request.backend.version
            || document.backend.python_version != request.backend.python_version
            || document.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || document.backend.worker_sha256 != request.backend.worker_sha256
            || document.input.input_path != self.prepared.input_path.display().to_string()
            || document.input.input_sha256 != self.prepared.input_sha256
            || document.input.bins_path != self.prepared.bins_path.display().to_string()
            || document.input.bins_sha256 != self.prepared.bins_sha256
            || document.feature_names != request.feature_names
            || document.multiplicity_control != "single_step_max_t"
            || document.permutations != request.permutations
            || document.seed != request.seed
            || document.request_sha256 != self.prepared.request_sha256
            || document.bins.len() != request.bins.len()
            || document.curves.is_empty()
            || document.projection_artifact.components.len() != request.components as usize
        {
            return Err(invalid("decoded projected variogram identity differs"));
        }
        for (actual, expected) in document.bins.iter().zip(&request.bins) {
            if actual.bin_id != expected.bin_id
                || actual.lower_um.to_bits() != expected.lower_um.to_bits()
                || actual.upper_um.to_bits() != expected.upper_um.to_bits()
                || actual.upper_inclusive != expected.upper_inclusive
            {
                return Err(invalid("decoded projected variogram bins differ"));
            }
        }
        if document.curves.iter().any(|curve| {
            curve.rows.len() != (request.components as usize * request.bins.len())
                || curve.rows.iter().any(|row| {
                    row.semivariance.is_some_and(|value| !value.is_finite())
                        || row
                            .max_t_adjusted_p
                            .is_some_and(|value| !(0.0..=1.0).contains(&value))
                })
        }) {
            return Err(invalid("decoded projected variogram curve differs"));
        }
        Ok(())
    }
}

impl WorkflowNode for ProjectedEmbeddingVariogramsNode {
    type Output = ProjectedEmbeddingVariogramDocument;

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
                "projected variogram source identity changed",
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
        let result = bayes::embedding_spatial::execute_projected_variograms(&self.prepared)
            .map_err(NodeError::execution)?;
        serde_json::from_value(serde_json::to_value(result).map_err(NodeError::execution)?)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let document = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate(&document).map_err(NodeError::decode)?;
        Ok(document)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ProjectedEmbeddingVariogramDocument {
    format: String,
    version: u32,
    backend: WorkerBackend,
    input: ProjectedInputIdentityDocument,
    feature_names: Vec<String>,
    projection_artifact: ProjectionArtifact,
    bins: Vec<EmbeddingDistanceBin>,
    multiplicity_control: String,
    permutations: u32,
    seed: u64,
    curves: Vec<ProjectedSplitCurve>,
    request_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ProjectedInputIdentityDocument {
    input_path: String,
    input_sha256: String,
    bins_path: String,
    bins_sha256: String,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
