use std::path::{Path, PathBuf};

use marklab_bayes::{
    sha256_hex, NegativeBinomialHierarchyResult, NegativeBinomialHierarchySpec,
    NegativeBinomialHierarchyWorkerRequest,
};
use marklab_project::ArtifactRef;

use super::cli_schema::NegativeBinomialHierarchyArgs;
use super::{publish_json, run_worker, MAXIMUM_INPUT_BYTES};
use crate::cli_error::CliError;
use crate::input_file;

const WORKER: &str = "marklab_numpyro_negative_binomial_hierarchy_worker.py";
const SOURCE_KIND: &str =
    "application/vnd.marklab.source.negative-binomial-hierarchy+json;version=1";
const LOCK_KIND: &str = "application/vnd.marklab.python-lock+text;version=1";
const WORKER_KIND: &str = "application/vnd.marklab.python-worker+source;version=1";

pub(super) fn run(args: NegativeBinomialHierarchyArgs) -> Result<(), CliError> {
    let NegativeBinomialHierarchyArgs { input, out } = args;
    let output = out;
    let prepared = prepare(&input)?;
    let result = execute(&prepared)?;
    publish_json(&output, &result)
}

pub(crate) struct Prepared {
    pub request: NegativeBinomialHierarchyWorkerRequest,
    pub request_bytes: Vec<u8>,
    pub request_sha256: String,
    pub artifacts: Vec<ArtifactRef>,
    repository: PathBuf,
}

pub(crate) fn prepare(input: &Path) -> Result<Prepared, CliError> {
    let source = read(input)?;
    let spec: NegativeBinomialHierarchySpec = serde_json::from_slice(&source)?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read(&directory.join("uv.lock"))?;
    let worker = read(&directory.join(WORKER))?;
    let artifacts: Vec<_> = [
        (SOURCE_KIND, source.as_slice()),
        (LOCK_KIND, lock.as_slice()),
        (WORKER_KIND, worker.as_slice()),
    ]
    .into_iter()
    .map(|(kind, bytes)| {
        ArtifactRef::from_bytes(kind, bytes).map_err(|error| CliError::Input(error.to_string()))
    })
    .collect::<Result<_, _>>()?;
    let request = NegativeBinomialHierarchyWorkerRequest::new(
        spec,
        artifacts[1].digest().to_string(),
        artifacts[2].digest().to_string(),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        request,
        request_bytes,
        request_sha256,
        artifacts,
        repository,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<NegativeBinomialHierarchyResult, CliError> {
    let bytes = run_worker(
        &prepared.repository,
        WORKER,
        &prepared.request_bytes,
        prepared.request.spec.timeout_seconds,
    )?;
    let result: NegativeBinomialHierarchyResult = serde_json::from_slice(&bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}

fn read(path: &Path) -> Result<Vec<u8>, CliError> {
    input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "negative-binomial hierarchy input/asset must be a regular file within 16 MiB",
    )
}
