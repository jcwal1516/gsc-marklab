use std::{borrow::Cow, io};

use marklab_bayes::WorkerBackend;
use marklab_embeddings::analysis::{EmbeddingDistanceBin, ProjectedSplitCurve, ProjectionArtifact};
use marklab_project::{ArtifactRef, ContentDigest};
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::ProjectedEmbeddingVariogramsProjectArgs;
use super::{
    bayes::embedding_spatial::{
        execute_projected_variograms, prepare_projected_variograms, PreparedProjectedVariograms,
    },
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.projected-embedding-points-csv;version=1";
const BINS_KIND: &str = "application/vnd.marklab.source.distance-bins-csv;version=1";
const LOCK_KIND: &str = "application/vnd.marklab.python-lock;version=1";
const WORKER_KIND: &str = "application/vnd.marklab.python-worker;version=1";

pub(super) fn run(args: ProjectedEmbeddingVariogramsProjectArgs) -> Result<(), CliError> {
    let ProjectedEmbeddingVariogramsProjectArgs {
        project: project_path,
        input,
        bins,
        components,
        permutations,
        seed,
        maximum_pair_visits,
        timeout_seconds,
        out: output_path,
    } = args;
    const LABEL: &str = "projected variogram sources";
    let repository = marklab::backend::python_backend_assets_root()?;
    let sources = Source::identify_all(
        [
            input.clone(),
            bins.clone(),
            repository.join("workers/python/uv.lock"),
            repository.join("workers/python/marklab_scipy_projected_variograms_worker.py"),
        ],
        &[INPUT_KIND, BINS_KIND, LOCK_KIND, WORKER_KIND],
    )?;
    let prepared = prepare_projected_variograms(
        input,
        bins,
        components,
        permutations,
        seed,
        maximum_pair_visits,
        timeout_seconds,
    )?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let artifacts: Vec<&ArtifactRef> = sources.iter().map(Source::artifact).collect();
    validate_prepared(&prepared, &artifacts)?;
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
        "bounded-process;backend=scipy@{};python={};training-only-pca;within-biological-unit-pairs;complete-row-within-unit-and-stratum;single-step-max-t;components={};permutations={};seed={};maximum_pair_visits={};timeout_seconds={}",
        request.backend.version,
        request.backend.python_version,
        request.components,
        request.permutations,
        request.seed,
        request.resources.maximum_pair_visits,
        prepared.timeout_seconds,
    );
    let contract = NodeContract {
        id: "scipy-projected-embedding-variograms",
        kind: "scipy_projected_embedding_variograms",
        version: 1,
        configuration_digest,
        execution_policy: Cow::Owned(execution_policy.into_bytes()),
        implementation_identity: "marklab-project-scipy-projected-embedding-variograms-node-v1",
        output_kind: "application/vnd.marklab.projected-embedding-variograms+json;version=1",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| {
            let result = execute_projected_variograms(prepared).map_err(NodeError::execution)?;
            serde_json::from_value(serde_json::to_value(result).map_err(NodeError::execution)?)
                .map_err(NodeError::execution)
        },
        |prepared, document| validate(prepared, document).map_err(NodeError::decode),
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.projected_embedding_variograms", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "projected-embedding-variograms",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn validate_prepared(
    prepared: &PreparedProjectedVariograms,
    artifacts: &[&ArtifactRef],
) -> Result<(), CliError> {
    if artifacts.len() != 4
        || prepared.input_sha256 != artifacts[0].digest().to_string()
        || prepared.bins_sha256 != artifacts[1].digest().to_string()
        || prepared.request.backend.environment_lock_sha256 != artifacts[2].digest().to_string()
        || prepared.request.backend.worker_sha256 != artifacts[3].digest().to_string()
    {
        return Err(CliError::Input(
            "projected variogram parsed/backend identities differ from durable sources".into(),
        ));
    }
    Ok(())
}

fn validate(
    prepared: &PreparedProjectedVariograms,
    document: &ProjectedEmbeddingVariogramDocument,
) -> io::Result<()> {
    let request = &prepared.request;
    if document.format != "marklab.projected_embedding_variograms"
        || document.version != 1
        || document.backend.name != "scipy"
        || document.backend.version != request.backend.version
        || document.backend.python_version != request.backend.python_version
        || document.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
        || document.backend.worker_sha256 != request.backend.worker_sha256
        || document.input.input_path != prepared.input_path.display().to_string()
        || document.input.input_sha256 != prepared.input_sha256
        || document.input.bins_path != prepared.bins_path.display().to_string()
        || document.input.bins_sha256 != prepared.bins_sha256
        || document.feature_names != request.feature_names
        || document.multiplicity_control != "single_step_max_t"
        || document.permutations != request.permutations
        || document.seed != request.seed
        || document.request_sha256 != prepared.request_sha256
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
