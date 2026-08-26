use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    BackendContract, FusedGromovWassersteinWorkerResult, NutsSamplingSpec, WorkerResult,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, DurableRecoveryAction, LocalScheduler, MarklabProject,
    NativeRuntimeProvenance, NodeError, NodeId, NodeSpec, SchedulerLimits, WorkflowGraph,
    WorkflowNode,
};

use super::bayes::{
    self, fused_gromov::PreparedFusedGromovWasserstein, BayesCliError, PreparedNormalMean,
};

const MAXIMUM_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;
const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;
const MAXIMUM_RESULT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy)]
enum StaticBackendWorkflow {
    PymcNormalMean,
    PotFusedGromovWasserstein,
}

#[derive(Clone, Copy)]
struct StaticBackendDescriptor {
    backend_id: &'static str,
    backend_version: &'static str,
    python_version: &'static str,
    license: &'static str,
    input_kind: &'static str,
    output_kind: &'static str,
    result_schema_id: &'static str,
    node_id: &'static str,
    node_kind: &'static str,
    implementation_identity: &'static str,
    deterministic_controls: &'static str,
}

impl StaticBackendWorkflow {
    fn descriptor(self) -> StaticBackendDescriptor {
        match self {
            Self::PymcNormalMean => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kind: "application/vnd.marklab.source.normal-mean-observations;version=1",
                output_kind: "application/vnd.marklab.pymc-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_worker_result",
                node_id: "pymc-normal-mean",
                node_kind: "bayesian_fit",
                implementation_identity: "marklab-project-pymc-normal-mean-node-v1",
                deterministic_controls: "seeded-nuts-request",
            },
            Self::PotFusedGromovWasserstein => StaticBackendDescriptor {
                backend_id: "pot",
                backend_version: "0.9.7.post1",
                python_version: "3.12",
                license: "MIT",
                input_kind: "application/vnd.marklab.source.fgw-input+json;version=1",
                output_kind:
                    "application/vnd.marklab.pot-fused-gromov-worker-result+json;version=1",
                result_schema_id: "marklab.pot_fused_gromov_wasserstein_worker_result",
                node_id: "pot-fused-gromov-wasserstein",
                node_kind: "descriptive_alignment",
                implementation_identity: "marklab-project-pot-fused-gromov-node-v1",
                deterministic_controls: "fixed-initialization-order-and-solver-controls",
            },
        }
    }
}

impl StaticBackendDescriptor {
    fn validate_request_backend(self, backend: &BackendContract) -> Result<(), BayesCliError> {
        if backend.name != self.backend_id
            || backend.version != self.backend_version
            || backend.python_version != self.python_version
            || !is_sha256(&backend.environment_lock_sha256)
            || !is_sha256(&backend.worker_sha256)
        {
            return Err(BayesCliError::Input(format!(
                "{} durable backend identity is incomplete or differs from its static descriptor",
                self.backend_id
            )));
        }
        Ok(())
    }

    fn configuration_digest(
        self,
        backend: &BackendContract,
        request_bytes: &[u8],
    ) -> ContentDigest {
        ContentDigest::from_framed([
            b"marklab-static-durable-backend-v1".as_slice(),
            self.backend_id.as_bytes(),
            self.backend_version.as_bytes(),
            self.python_version.as_bytes(),
            self.license.as_bytes(),
            backend.environment_lock_sha256.as_bytes(),
            backend.worker_sha256.as_bytes(),
            self.input_kind.as_bytes(),
            self.result_schema_id.as_bytes(),
            self.deterministic_controls.as_bytes(),
            request_bytes,
        ])
    }

    fn execution_policy(self) -> Vec<u8> {
        format!(
            "marklab-static-process-policy-v1\0backend={}\0version={}\0python={}\0license={}\0controls={}\0environment=cleared\0network=cleared\0bounded-process=true",
            self.backend_id,
            self.backend_version,
            self.python_version,
            self.license,
            self.deterministic_controls
        )
        .into_bytes()
    }

    fn result_schema(self) -> Result<ArtifactSchema, BayesCliError> {
        ArtifactSchema::new(self.result_schema_id, 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct ProjectCli {
    #[command(subcommand)]
    command: ProjectTopLevel,
}

#[derive(Debug, Subcommand)]
enum ProjectTopLevel {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    NormalMean {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        prior_mean: f64,
        #[arg(long)]
        prior_sd: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FusedGromovWasserstein {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        feature_scale: f64,
        #[arg(long)]
        structure_scale: f64,
        #[arg(long)]
        tolerance: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    match ProjectCli::parse_from(std::env::args_os()).command {
        ProjectTopLevel::Project {
            command:
                ProjectCommand::NormalMean {
                    project,
                    input,
                    prior_mean,
                    prior_sd,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_normal_mean(
            project,
            input,
            prior_mean,
            prior_sd,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::FusedGromovWasserstein {
                    project,
                    input,
                    alpha,
                    epsilon,
                    feature_scale,
                    structure_scale,
                    tolerance,
                    maximum_iterations,
                    timeout_seconds,
                    out,
                },
        } => run_fused_gromov_wasserstein(
            project,
            input,
            alpha,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            timeout_seconds,
            out,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_normal_mean(
    project_path: PathBuf,
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcNormalMean.descriptor();
    let before = source_artifact(&input_path, backend.input_kind)?;
    let prepared = bayes::prepare_normal_mean(
        input_path.clone(),
        prior_mean,
        prior_sd,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "normal-mean input changed while the durable request was prepared".into(),
        ));
    }

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

    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = NormalMeanProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_fit(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project normal-mean cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_fused_gromov_wasserstein(
    project_path: PathBuf,
    input_path: PathBuf,
    alpha: f64,
    epsilon: f64,
    feature_scale: f64,
    structure_scale: f64,
    tolerance: f64,
    maximum_iterations: u32,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PotFusedGromovWasserstein.descriptor();
    let before = source_artifact(&input_path, backend.input_kind)?;
    let prepared = bayes::fused_gromov::prepare(
        input_path.clone(),
        alpha,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "FGW input changed while the durable request was prepared".into(),
        ));
    }

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
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = FusedGromovProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::fused_gromov::publish_result(&output_path, &node.prepared, run.output)?;
    eprintln!("project fused-gromov-wasserstein cache_status={cache_status}");
    Ok(())
}

struct NormalMeanProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedNormalMean,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl NormalMeanProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedNormalMean,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for NormalMeanProjectNode {
    type Output = WorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed =
            source_artifact(&self.input_path, self.backend.input_kind).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "normal-mean input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_normal_mean(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: WorkerResult = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct FusedGromovProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedFusedGromovWasserstein,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl FusedGromovProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedFusedGromovWasserstein,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for FusedGromovProjectNode {
    type Output = FusedGromovWassersteinWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed =
            source_artifact(&self.input_path, self.backend.input_kind).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "FGW input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::fused_gromov::execute(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        let mut encoded = serde_json::to_vec(output).map_err(NodeError::encoding)?;
        for _ in 0..8 {
            let normalized: FusedGromovWassersteinWorkerResult =
                serde_json::from_slice(&encoded).map_err(NodeError::encoding)?;
            let next = serde_json::to_vec(&normalized).map_err(NodeError::encoding)?;
            if next == encoded {
                return Ok(next.into_boxed_slice());
            }
            encoded = next;
        }
        Err(NodeError::encoding(BayesCliError::Input(
            "typed POT result JSON did not reach a stable canonical representation".into(),
        )))
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: FusedGromovWassersteinWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

fn source_artifact(path: &Path, kind: &str) -> Result<ArtifactRef, BayesCliError> {
    let mut file = File::open(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied = copy_bounded(&mut file, &mut digest, MAXIMUM_INPUT_BYTES).map_err(|source| {
        BayesCliError::Io {
            path: path.to_owned(),
            source,
        }
    })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(BayesCliError::Input(
            "source hashing byte count mismatch".into(),
        ));
    }
    ArtifactRef::new(kind, digest, byte_len)
        .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn native_runtime_provenance() -> Result<NativeRuntimeProvenance, BayesCliError> {
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
    .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn executable_artifact() -> Result<ArtifactRef, BayesCliError> {
    let path = std::env::current_exe().map_err(|source| BayesCliError::Io {
        path: PathBuf::from("current executable"),
        source,
    })?;
    let mut file = File::open(&path).map_err(|source| BayesCliError::Io {
        path: path.clone(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied =
        copy_bounded(&mut file, &mut digest, MAXIMUM_EXECUTABLE_BYTES).map_err(|source| {
            BayesCliError::Io {
                path: path.clone(),
                source,
            }
        })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(BayesCliError::Input(
            "executable hashing byte count mismatch".into(),
        ));
    }
    ArtifactRef::new("application/vnd.marklab.executable", digest, byte_len)
        .map_err(|error| BayesCliError::Input(error.to_string()))
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
        (cfg!(feature = "cli"), "cli"),
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

fn report_recovery(project: &DurableProject) {
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
    use super::*;

    fn backend(
        name: &'static str,
        version: &'static str,
        lock_byte: char,
        worker_byte: char,
    ) -> BackendContract {
        BackendContract {
            name,
            version,
            python_version: "3.12",
            environment_lock_sha256: lock_byte.to_string().repeat(64),
            worker_sha256: worker_byte.to_string().repeat(64),
        }
    }

    #[test]
    fn static_backend_identity_binds_environment_worker_and_request_bytes() {
        let descriptor = StaticBackendWorkflow::PymcNormalMean.descriptor();
        let exact = backend("pymc", "6.3.0", 'a', 'b');
        descriptor
            .validate_request_backend(&exact)
            .expect("exact descriptor");
        let baseline = descriptor.configuration_digest(&exact, b"request-a");

        let changed_lock = backend("pymc", "6.3.0", 'c', 'b');
        let changed_worker = backend("pymc", "6.3.0", 'a', 'd');
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_lock, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_worker, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&exact, b"request-b")
        );

        let wrong_backend = backend("pot", "0.9.7.post1", 'a', 'b');
        assert!(descriptor.validate_request_backend(&wrong_backend).is_err());
    }
}
