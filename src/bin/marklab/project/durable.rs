//! Durable execution shared by the `marklab project` commands.
//!
//! A command prepares its request from content-identified [`Source`] files, wraps the
//! analysis in a [`SourceNode`], runs it with [`execute_node`] (which opens the project,
//! catalogs the inputs and replays or executes the node) and writes the output with
//! [`publish`].

use std::{
    borrow::Cow,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use marklab_project::{
    ArtifactId, ArtifactRef, ArtifactSchema, ContentDigest, DurableProject, DurableProjectLimits,
    DurableRecoveryAction, MarklabProject, NativeRuntimeProvenance,
};
use marklab_workflow::{
    execute_algorithm, CacheKeyMaterial, CacheStatus, LocalScheduler, NodeError, NodeId, NodeRun,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{de::DeserializeOwned, Serialize};

use super::{bayes, CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES};

const MAXIMUM_EXECUTABLE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAXIMUM_PYTHON_SUPPORT_ENTRIES: usize = 1024;
const MAXIMUM_PYTHON_SUPPORT_FILES: usize = 256;
const PYTHON_SUPPORT_KIND: &str = "application/vnd.marklab.python-worker-support+source;version=1";
const SOURCE_NODE_JSON_POLICY: &str = "marklab-source-node-json-f64-roundtrip-v1";

/// Canonical byte encoding of a node output.
#[derive(Clone, Copy)]
pub(super) enum Codec {
    /// [`marklab::exact_float_json`], which preserves every `f64` bit pattern.
    ExactFloatJson,
    /// Compact `serde_json`, the format Python workers emit.
    Json,
    /// Pretty-printed `serde_json`, retained for existing durable node codecs.
    PrettyJson,
}

/// Graph identity, cache key material and output contract of a node.
pub(super) struct NodeContract {
    pub(super) id: &'static str,
    pub(super) kind: &'static str,
    pub(super) version: u32,
    pub(super) configuration_digest: ContentDigest,
    pub(super) execution_policy: Cow<'static, [u8]>,
    pub(super) implementation_identity: &'static str,
    pub(super) output_kind: &'static str,
    pub(super) codec: Codec,
}

impl NodeContract {
    fn node_spec(&self) -> Result<NodeSpec, CliError> {
        let id = NodeId::new(self.id).map_err(|error| CliError::Input(error.to_string()))?;
        NodeSpec::new(id, self.kind, self.version, Vec::new())
            .map_err(|error| CliError::Input(error.to_string()))
    }
}

/// A source file and the content identity recorded when its request was prepared.
pub(super) struct Source {
    path: PathBuf,
    kind: &'static str,
    artifact: ArtifactRef,
}

impl Source {
    /// Identify `path` by hashing its current content.
    pub(super) fn identify(path: PathBuf, kind: &'static str) -> Result<Self, CliError> {
        let artifact = source_artifact(&path, kind)?;
        Ok(Self {
            path,
            kind,
            artifact,
        })
    }

    /// Identify `path` by bytes that were already read from it.
    pub(super) fn from_bytes(
        path: PathBuf,
        kind: &'static str,
        bytes: &[u8],
    ) -> Result<Self, CliError> {
        let artifact = ArtifactRef::from_bytes(kind, bytes)
            .map_err(|error| CliError::Input(error.to_string()))?;
        Ok(Self {
            path,
            kind,
            artifact,
        })
    }

    /// Identify each path with the kind at the same position.
    pub(super) fn identify_all<const N: usize>(
        paths: [PathBuf; N],
        kinds: &[&'static str],
    ) -> Result<Vec<Self>, CliError> {
        assert_eq!(N, kinds.len(), "one source kind per path");
        paths
            .into_iter()
            .zip(kinds)
            .map(|(path, kind)| Self::identify(path, kind))
            .collect()
    }

    /// [`Source::ensure_unchanged`] for every source.
    pub(super) fn ensure_all_unchanged(sources: &[Self], label: &str) -> Result<(), CliError> {
        sources
            .iter()
            .try_for_each(|source| source.ensure_unchanged(label))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    /// Fail if the file no longer has the identity recorded for it, e.g. because it was
    /// replaced while the request was being prepared.
    pub(super) fn ensure_unchanged(&self, label: &str) -> Result<(), CliError> {
        if source_artifact(&self.path, self.kind)? != self.artifact {
            return Err(CliError::Input(format!(
                "{label} changed after it was recorded in the project"
            )));
        }
        Ok(())
    }
}

/// Node-specific re-derivation of inputs that are not plain source files.
type InputCheck<P> = fn(&P) -> Result<(), NodeError>;

/// A workflow node that re-verifies its source files before every cache lookup and
/// otherwise delegates to plain functions over a prepared payload.
pub(super) struct SourceNode<P, O> {
    spec: NodeSpec,
    contract: NodeContract,
    implementation_identity: String,
    label: &'static str,
    sources: Vec<(PathBuf, &'static str)>,
    input_artifacts: Vec<ArtifactRef>,
    payload: P,
    check: Option<InputCheck<P>>,
    execute: fn(&P) -> Result<O, NodeError>,
    validate: fn(&P, &O) -> Result<(), NodeError>,
}

impl<P, O> SourceNode<P, O> {
    /// `label` names the inputs in error messages; `validate` checks a decoded output
    /// (fresh or cached) against the payload it must have been computed from.
    pub(super) fn new(
        contract: NodeContract,
        label: &'static str,
        sources: Vec<Source>,
        payload: P,
        execute: fn(&P) -> Result<O, NodeError>,
        validate: fn(&P, &O) -> Result<(), NodeError>,
    ) -> Result<Self, CliError> {
        let implementation_identity = format!(
            "{};{SOURCE_NODE_JSON_POLICY}",
            contract.implementation_identity
        );
        let (sources, input_artifacts) = sources
            .into_iter()
            .map(|source| ((source.path, source.kind), source.artifact))
            .unzip();
        Ok(Self {
            spec: contract.node_spec()?,
            contract,
            implementation_identity,
            label,
            sources,
            input_artifacts,
            payload,
            check: None,
            execute,
            validate,
        })
    }

    /// A node whose inputs are not plain source files (for example a worker script and
    /// lock file resolved at run time); `check` re-derives them and fails on any change.
    pub(super) fn with_input_check(
        contract: NodeContract,
        input_artifacts: Vec<ArtifactRef>,
        payload: P,
        check: InputCheck<P>,
        execute: fn(&P) -> Result<O, NodeError>,
        validate: fn(&P, &O) -> Result<(), NodeError>,
    ) -> Result<Self, CliError> {
        let implementation_identity = format!(
            "{};{SOURCE_NODE_JSON_POLICY}",
            contract.implementation_identity
        );
        Ok(Self {
            spec: contract.node_spec()?,
            contract,
            implementation_identity,
            label: "",
            sources: Vec::new(),
            input_artifacts,
            payload,
            check: Some(check),
            execute,
            validate,
        })
    }

    pub(super) fn payload(&self) -> &P {
        &self.payload
    }
}

impl<P, O: Serialize + DeserializeOwned> WorkflowNode for SourceNode<P, O> {
    type Output = O;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        for ((path, kind), recorded) in self.sources.iter().zip(&self.input_artifacts) {
            let observed = source_artifact(path, kind).map_err(NodeError::input)?;
            if &observed != recorded {
                return Err(NodeError::input(CliError::Input(format!(
                    "{} changed after it was recorded in the project",
                    self.label
                ))));
            }
        }
        match self.check {
            Some(check) => check(&self.payload),
            None => Ok(()),
        }
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.contract.configuration_digest,
            execution_policy: &self.contract.execution_policy,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<O, NodeError> {
        (self.execute)(&self.payload)
    }

    fn encode_output(&self, output: &O) -> Result<Box<[u8]>, NodeError> {
        match self.contract.codec {
            Codec::ExactFloatJson => {
                marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
            }
            Codec::Json => serde_json::to_vec(output)
                .map(Vec::into_boxed_slice)
                .map_err(NodeError::encoding),
            Codec::PrettyJson => serde_json::to_vec_pretty(output)
                .map(Vec::into_boxed_slice)
                .map_err(NodeError::encoding),
        }
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<O, NodeError> {
        let output: O = match self.contract.codec {
            Codec::ExactFloatJson => {
                marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?
            }
            Codec::Json => serde_json::from_slice(bytes).map_err(NodeError::decode)?,
            Codec::PrettyJson => serde_json::from_slice(bytes).map_err(NodeError::decode)?,
        };
        (self.validate)(&self.payload, &output)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.contract.output_kind
    }
}

/// An open durable project with one command's inputs cataloged.
pub(super) struct Session {
    durable: DurableProject,
    project: MarklabProject,
    scheduler: LocalScheduler,
    runtime: NativeRuntimeProvenance,
}

impl Session {
    /// Open (or create) the durable project at `project_path`, recovering it if needed,
    /// and catalog `inputs` so nodes may read them.
    pub(super) fn open(
        project_path: &Path,
        inputs: &[ArtifactRef],
        maximum_result_bytes: usize,
    ) -> Result<Self, CliError> {
        let runtime = native_runtime_provenance()?;
        let limits =
            DurableProjectLimits::for_object_bytes(maximum_result_bytes).map_err(invalid)?;
        let durable = DurableProject::open_or_create(project_path, limits)
            .map_err(|error| CliError::Backend(error.to_string()))?;
        report_recovery(&durable);
        let mut project =
            MarklabProject::with_inline_artifact_limit(maximum_result_bytes).map_err(invalid)?;
        for artifact in inputs {
            project
                .register_reference(artifact.clone())
                .map_err(invalid)?;
        }
        let scheduler = LocalScheduler::new(SchedulerLimits {
            max_inline_output_bytes: maximum_result_bytes,
        })
        .map_err(invalid)?;
        Ok(Self {
            durable,
            project,
            scheduler,
            runtime,
        })
    }

    /// Replay a verified cached output of `node` or execute it and commit the result.
    pub(super) fn run<N: WorkflowNode>(
        &mut self,
        graph: &WorkflowGraph,
        node: &N,
        (schema_id, schema_version): (&str, u32),
    ) -> Result<NodeRun<N::Output>, CliError> {
        let schema = ArtifactSchema::new(schema_id, schema_version).map_err(invalid)?;
        execute_algorithm(
            &mut self.durable,
            &mut self.project,
            graph,
            node,
            &self.scheduler,
            schema,
            self.runtime.clone(),
        )
        .map_err(|error| CliError::Backend(error.to_string()))
    }
}

/// Run a single node in the durable project at `project_path`.
pub(super) fn execute_node<N: WorkflowNode>(
    project_path: &Path,
    node: &N,
    result_schema: (&str, u32),
    maximum_result_bytes: usize,
) -> Result<NodeRun<N::Output>, CliError> {
    let graph = WorkflowGraph::new([node.spec().clone()]).map_err(invalid)?;
    Session::open(project_path, node.input_artifacts(), maximum_result_bytes)?.run(
        &graph,
        node,
        result_schema,
    )
}

/// Run a Python-backed node while binding its durable identity to the imported support package.
pub(super) fn execute_python_node<N: WorkflowNode>(
    project_path: &Path,
    node: &N,
    result_schema: (&str, u32),
    maximum_result_bytes: usize,
) -> Result<NodeRun<N::Output>, CliError> {
    let node = PythonSupportNode::new(node)?;
    execute_node(project_path, &node, result_schema, maximum_result_bytes)
}

struct PythonSupportNode<'a, N> {
    node: &'a N,
    support_directory: PathBuf,
    support_artifact: ArtifactRef,
    input_artifacts: Vec<ArtifactRef>,
}

impl<'a, N: WorkflowNode> PythonSupportNode<'a, N> {
    fn new(node: &'a N) -> Result<Self, CliError> {
        let support_directory = marklab::backend::python_backend_assets_root()?
            .join("workers/python/marklab_worker_support");
        let support_artifact = python_support_artifact(&support_directory)?;
        let mut input_artifacts = Vec::with_capacity(node.input_artifacts().len() + 1);
        input_artifacts.extend_from_slice(node.input_artifacts());
        input_artifacts.push(support_artifact.clone());
        Ok(Self {
            node,
            support_directory,
            support_artifact,
            input_artifacts,
        })
    }
}

impl<N: WorkflowNode> WorkflowNode for PythonSupportNode<'_, N> {
    type Output = N::Output;

    fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn semantic_input_artifacts(&self) -> &[ArtifactId] {
        self.node.semantic_input_artifacts()
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        self.node.verify_input_content()?;
        let observed =
            python_support_artifact(&self.support_directory).map_err(NodeError::input)?;
        if observed != self.support_artifact {
            return Err(NodeError::input(CliError::Input(
                "Python worker support package changed after preparation".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        self.node.cache_key_material()
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        self.node.execute()
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.node.encode_output(output)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        self.node.decode_output(bytes)
    }

    fn output_kind(&self) -> &'static str {
        self.node.output_kind()
    }
}

fn python_support_artifact(directory: &Path) -> Result<ArtifactRef, CliError> {
    python_support_artifact_with_limit(directory, MAXIMUM_INPUT_BYTES)
}

fn python_support_artifact_with_limit(
    directory: &Path,
    maximum_bytes: u64,
) -> Result<ArtifactRef, CliError> {
    let mut paths = Vec::new();
    let entries = fs::read_dir(directory).map_err(|source| CliError::Io {
        path: directory.to_owned(),
        source,
    })?;
    for (index, entry) in entries.enumerate() {
        if index == MAXIMUM_PYTHON_SUPPORT_ENTRIES {
            return Err(CliError::Input(format!(
                "Python worker support package exceeds {MAXIMUM_PYTHON_SUPPORT_ENTRIES} entries"
            )));
        }
        let entry = entry.map_err(|source| CliError::Io {
            path: directory.to_owned(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| CliError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_symlink() {
            return Err(CliError::Input(format!(
                "Python worker support package contains a symbolic link: {}",
                path.display()
            )));
        }
        if file_type.is_dir() {
            if entry.file_name() != "__pycache__" {
                return Err(CliError::Input(format!(
                    "Python worker support package contains an unsupported subdirectory: {}",
                    path.display()
                )));
            }
            continue;
        }
        if !file_type.is_file() || path.extension().and_then(|value| value.to_str()) != Some("py") {
            continue;
        }
        if paths.len() == MAXIMUM_PYTHON_SUPPORT_FILES {
            return Err(CliError::Input(format!(
                "Python worker support package exceeds {MAXIMUM_PYTHON_SUPPORT_FILES} source files"
            )));
        }
        let relative = entry.file_name().into_string().map_err(|value| {
            CliError::Input(format!(
                "Python worker support filename is not UTF-8: {}",
                PathBuf::from(value).display()
            ))
        })?;
        paths.push((relative, path));
    }
    if paths.is_empty() {
        return Err(CliError::Input(format!(
            "Python worker support package contains no source files: {}",
            directory.display()
        )));
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));

    let mut total_bytes = 0_u64;
    let mut frames = Vec::with_capacity(paths.len() * 2 + 1);
    frames.push(b"marklab-python-worker-support-v1".to_vec());
    for (relative, path) in paths {
        let remaining = maximum_bytes.checked_sub(total_bytes).ok_or_else(|| {
            CliError::Input(format!(
                "Python worker support package exceeds {maximum_bytes} bytes"
            ))
        })?;
        let bytes = crate::input_file::read_regular_file(
            &path,
            remaining,
            &format!(
                "Python worker support source must be a regular file within the package limit: {}",
                path.display()
            ),
        )?;
        total_bytes = total_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| CliError::Input("Python worker support byte count overflowed".into()))?;
        frames.push(relative.into_bytes());
        frames.push(bytes);
    }
    let digest = ContentDigest::from_framed(frames.iter().map(Vec::as_slice));
    ArtifactRef::new(PYTHON_SUPPORT_KIND, digest, total_bytes)
        .map_err(|error| CliError::Input(error.to_string()))
}

pub(super) fn invalid(error: impl std::fmt::Display) -> CliError {
    CliError::Input(error.to_string())
}

/// Identity of a native command whose whole request is one JSON spec file.
pub(super) struct NativeSpecCommand {
    /// Subcommand name reported on stderr.
    pub(super) name: &'static str,
    /// How error messages refer to the input file.
    pub(super) label: &'static str,
    pub(super) input_kind: &'static str,
    pub(super) node_id: &'static str,
    pub(super) node_kind: &'static str,
    pub(super) node_version: u32,
    /// Versioned implementation name; also frames the request in the cache key.
    pub(super) implementation_identity: &'static str,
    pub(super) execution_policy: &'static [u8],
    pub(super) output_kind: &'static str,
    pub(super) result_schema: (&'static str, u32),
    pub(super) codec: Codec,
}

/// Parse the JSON spec at `input_path`, run `execute` on it durably and publish the output.
pub(super) fn run_native_spec<S: DeserializeOwned, O: Serialize + DeserializeOwned>(
    command: &NativeSpecCommand,
    project_path: &Path,
    input_path: PathBuf,
    output_path: &Path,
    execute: fn(&S) -> Result<O, NodeError>,
    validate: fn(&S, &O) -> Result<(), NodeError>,
) -> Result<(), CliError> {
    run_native_request(
        command,
        project_path,
        input_path,
        output_path,
        |bytes| serde_json::from_slice(bytes).map_err(CliError::from),
        execute,
        validate,
    )
}

/// [`run_native_spec`] with a command-specific request parser.
pub(super) fn run_native_request<S, O: Serialize + DeserializeOwned>(
    command: &NativeSpecCommand,
    project_path: &Path,
    input_path: PathBuf,
    output_path: &Path,
    parse: fn(&[u8]) -> Result<S, CliError>,
    execute: fn(&S) -> Result<O, NodeError>,
    validate: fn(&S, &O) -> Result<(), NodeError>,
) -> Result<(), CliError> {
    let request_bytes = crate::input_file::read_regular_file(
        &input_path,
        MAXIMUM_INPUT_BYTES,
        &format!("{} must be a regular file within 16 MiB", command.label),
    )?;
    let request = parse(&request_bytes)?;
    let source = Source::from_bytes(input_path, command.input_kind, &request_bytes)?;
    source.ensure_unchanged(command.label)?;
    let contract = NodeContract {
        id: command.node_id,
        kind: command.node_kind,
        version: command.node_version,
        configuration_digest: ContentDigest::from_framed([
            command.implementation_identity.as_bytes(),
            request_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(command.execution_policy),
        implementation_identity: command.implementation_identity,
        output_kind: command.output_kind,
        codec: command.codec,
    };
    let node = SourceNode::new(
        contract,
        command.label,
        vec![source],
        request,
        execute,
        validate,
    )?;
    let run = execute_node(
        project_path,
        &node,
        command.result_schema,
        MAXIMUM_RESULT_BYTES,
    )?;
    publish(command.name, output_path, &run.output, run.cache_status)
}

/// Write `value` to `output_path` and report on stderr whether it was replayed.
pub(super) fn publish(
    command: &str,
    output_path: &Path,
    value: &impl Serialize,
    cache_status: CacheStatus,
) -> Result<(), CliError> {
    bayes::publish_json(output_path, value)?;
    eprintln!(
        "project {command} cache_status={}",
        cache_status_name(cache_status)
    );
    Ok(())
}

pub(super) fn cache_status_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    }
}

pub(super) fn source_artifact(path: &Path, kind: &str) -> Result<ArtifactRef, CliError> {
    let mut file = File::open(path).map_err(|source| CliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied = copy_bounded(&mut file, &mut digest, MAXIMUM_INPUT_BYTES).map_err(|source| {
        CliError::Io {
            path: path.to_owned(),
            source,
        }
    })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(CliError::Input("source hashing byte count mismatch".into()));
    }
    ArtifactRef::new(kind, digest, byte_len).map_err(|error| CliError::Input(error.to_string()))
}

pub(in super::super) fn native_runtime_provenance() -> Result<NativeRuntimeProvenance, CliError> {
    let executable = executable_artifact()?;
    let git_sha = match env!("MARKLAB_BUILD_GIT_SHA") {
        "" => None,
        value => Some(value.to_owned()),
    };
    let git_dirty = match env!("MARKLAB_BUILD_GIT_DIRTY") {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    };
    NativeRuntimeProvenance::new(
        env!("CARGO_PKG_VERSION"),
        git_sha,
        git_dirty,
        env!("MARKLAB_BUILD_RUSTC"),
        compiled_features(),
        executable,
    )
    .map_err(|error| CliError::Input(error.to_string()))
}

fn executable_artifact() -> Result<ArtifactRef, CliError> {
    let path = std::env::current_exe().map_err(|source| CliError::Io {
        path: PathBuf::from("current executable"),
        source,
    })?;
    let mut file = File::open(&path).map_err(|source| CliError::Io {
        path: path.clone(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied =
        copy_bounded(&mut file, &mut digest, MAXIMUM_EXECUTABLE_BYTES).map_err(|source| {
            CliError::Io {
                path: path.clone(),
                source,
            }
        })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(CliError::Input(
            "executable hashing byte count mismatch".into(),
        ));
    }
    ArtifactRef::new("application/vnd.marklab.executable", digest, byte_len)
        .map_err(|error| CliError::Input(error.to_string()))
}

fn copy_bounded(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    maximum: u64,
) -> std::io::Result<u64> {
    let mut limited = reader.take(maximum.saturating_add(1));
    let copied = std::io::copy(&mut limited, writer)?;
    if copied > maximum {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "input exceeds its provenance hashing limit",
        ));
    }
    Ok(copied)
}

fn compiled_features() -> Vec<String> {
    let candidates = [
        (cfg!(feature = "allocator-mimalloc"), "allocator-mimalloc"),
        (true, "cli"),
        (cfg!(feature = "csv"), "csv"),
        (cfg!(feature = "dhat-heap"), "dhat-heap"),
        (cfg!(feature = "parallel"), "parallel"),
        (cfg!(feature = "parquet"), "parquet"),
        (cfg!(feature = "wsi"), "wsi"),
    ];
    candidates
        .into_iter()
        .filter_map(|(enabled, name)| enabled.then_some(name.to_owned()))
        .collect()
}

pub(super) fn report_recovery(project: &DurableProject) {
    let report = project.open_report();
    if report.action() != DurableRecoveryAction::None
        || !report.artifact_store().quarantined().is_empty()
        || !report.artifact_store().issues().is_empty()
    {
        eprintln!(
            "project recovery: action={:?}, quarantined_objects={}, retained_issues={}",
            report.action(),
            report.artifact_store().quarantined().len(),
            report.artifact_store().issues().len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute_node, python_support_artifact, python_support_artifact_with_limit, Codec,
        NodeContract, Source, SourceNode,
    };
    use marklab_project::ContentDigest;
    use marklab_workflow::{CacheStatus, NodeError};
    use std::borrow::Cow;
    use std::fs;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    struct FloatPayload {
        value: f64,
        executions: Arc<AtomicUsize>,
    }

    fn execute_float(payload: &FloatPayload) -> Result<f64, NodeError> {
        payload.executions.fetch_add(1, Ordering::SeqCst);
        Ok(payload.value)
    }

    fn validate_finite_float(_: &FloatPayload, output: &f64) -> Result<(), NodeError> {
        assert!(output.is_finite());
        Ok(())
    }

    fn float_contract() -> NodeContract {
        NodeContract {
            id: "json-policy-probe",
            kind: "json_policy_probe",
            version: 1,
            configuration_digest: ContentDigest::from_bytes(b"configuration"),
            execution_policy: Cow::Borrowed(b"serial"),
            implementation_identity: "probe-node-v1",
            output_kind: "application/json",
            codec: Codec::Json,
        }
    }

    #[test]
    fn source_node_json_policy_invalidates_legacy_float_results_once() {
        let temporary = tempfile::tempdir().expect("temporary project");
        let source_path = temporary.path().join("source.json");
        let source_bytes = b"57.925";
        fs::write(&source_path, source_bytes).expect("write source");
        let corrected: f64 = serde_json::from_slice(source_bytes).expect("parse corrected float");
        let historical = f64::from_bits(corrected.to_bits() - 1);
        assert_ne!(historical.to_bits(), corrected.to_bits());
        let executions = Arc::new(AtomicUsize::new(0));

        let mut legacy = SourceNode::new(
            float_contract(),
            "probe input",
            vec![
                Source::from_bytes(source_path.clone(), "application/json", source_bytes)
                    .expect("legacy source"),
            ],
            FloatPayload {
                value: historical,
                executions: Arc::clone(&executions),
            },
            execute_float,
            validate_finite_float,
        )
        .expect("legacy node");
        legacy.implementation_identity = legacy.contract.implementation_identity.to_owned();
        let legacy_run = execute_node(
            temporary.path(),
            &legacy,
            ("marklab.test_json_policy_probe", 1),
            1024,
        )
        .expect("prime legacy result");
        assert_eq!(legacy_run.cache_status, CacheStatus::Miss);
        assert_eq!(legacy_run.output.to_bits(), historical.to_bits());

        let current = SourceNode::new(
            float_contract(),
            "probe input",
            vec![
                Source::from_bytes(source_path, "application/json", source_bytes)
                    .expect("current source"),
            ],
            FloatPayload {
                value: corrected,
                executions: Arc::clone(&executions),
            },
            execute_float,
            validate_finite_float,
        )
        .expect("current node");
        let corrected_run = execute_node(
            temporary.path(),
            &current,
            ("marklab.test_json_policy_probe", 1),
            1024,
        )
        .expect("execute corrected result");
        assert_eq!(corrected_run.cache_status, CacheStatus::Miss);
        assert_eq!(corrected_run.output.to_bits(), corrected.to_bits());

        let replay = execute_node(
            temporary.path(),
            &current,
            ("marklab.test_json_policy_probe", 1),
            1024,
        )
        .expect("replay corrected result");
        assert_eq!(replay.cache_status, CacheStatus::Hit);
        assert_eq!(replay.output.to_bits(), corrected.to_bits());
        assert_eq!(executions.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn python_support_identity_binds_source_names_and_bytes_but_ignores_bytecode_cache() {
        let temporary = tempfile::tempdir().expect("temporary support package");
        let package = temporary.path();
        let init = package.join("__init__.py");
        let model = package.join("model.py");
        fs::write(&init, b"from .model import fit\n").expect("write package initializer");
        fs::write(&model, b"def fit(): return 1\n").expect("write support module");
        let baseline = python_support_artifact(package).expect("baseline identity");

        let cache = package.join("__pycache__");
        fs::create_dir(&cache).expect("create bytecode cache");
        fs::write(cache.join("model.pyc"), b"ignored cache bytes").expect("write bytecode cache");
        assert_eq!(
            python_support_artifact(package).expect("identity with cache"),
            baseline
        );

        fs::write(&model, b"def fit(): return 2\n").expect("change support bytes");
        assert_ne!(
            python_support_artifact(package).expect("changed-byte identity"),
            baseline
        );
        fs::write(&model, b"def fit(): return 1\n").expect("restore support bytes");

        let renamed = package.join("renamed.py");
        fs::rename(&model, &renamed).expect("rename support module");
        assert_ne!(
            python_support_artifact(package).expect("renamed identity"),
            baseline
        );
        fs::rename(&renamed, &model).expect("restore support name");

        fs::remove_file(&model).expect("remove support module");
        assert_ne!(
            python_support_artifact(package).expect("removed identity"),
            baseline
        );
        fs::write(&model, b"def fit(): return 1\n").expect("add support module back");
        assert_eq!(
            python_support_artifact(package).expect("restored identity"),
            baseline
        );
    }

    #[test]
    fn python_support_identity_enforces_the_aggregate_byte_limit() {
        let temporary = tempfile::tempdir().expect("temporary support package");
        fs::write(temporary.path().join("__init__.py"), b"1234").expect("write oversized package");
        assert!(python_support_artifact_with_limit(temporary.path(), 3).is_err());
    }
}
