use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
    path::Path,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArtifactDraft, ArtifactLocator, ArtifactRecord, ArtifactRecordError, ArtifactRef,
    ArtifactSchema, ArtifactStoreError, ContentDigest, LocalArtifactStore, MarklabProject,
    ProjectError, RecoveryReport, StoreId, VerifiedReaderError,
};

const HEAD_PATH: &str = "project.json";
const LEDGER_PATH: &str = "executions.jsonl";
const PENDING_PATH: &str = "pending-execution.json";
const LOCK_PATH: &str = ".marklab-project.lock";
const STORE_DIRECTORY: &str = "artifacts";
const STORE_ID: &str = "project_objects";
const PROJECT_FORMAT: &str = "marklab.project";
const EXECUTION_FORMAT: &str = "marklab.execution";
const PENDING_FORMAT: &str = "marklab.pending_execution";
const FORMAT_VERSION: u32 = 1;
const STORE_POLICY: &str = "local_content_addressed_sha256_v1";
const OUTPUT_SCHEMA_ID: &str = "marklab.workflow_node_output";
const OUTPUT_SCHEMA_VERSION: u32 = 1;
const MAX_EXECUTION_INPUTS: usize = 64;
const MAX_FEATURES: usize = 64;
const MAX_TEXT_BYTES: usize = 1_024;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Positive resource limits for one durable local project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableProjectLimits {
    /// Maximum canonical project-head or pending-intent bytes.
    pub maximum_control_bytes: usize,
    /// Maximum bytes scanned from the append-only execution ledger.
    pub maximum_ledger_bytes: usize,
    /// Maximum number of execution records scanned from the ledger.
    pub maximum_ledger_records: usize,
    /// Maximum canonical bytes for one execution record, excluding its newline.
    pub maximum_record_bytes: usize,
    /// Maximum bytes materialized for one durable node output.
    pub maximum_object_bytes: usize,
}

impl DurableProjectLimits {
    /// Build and validate explicit positive project-state bounds.
    pub fn new(
        maximum_control_bytes: usize,
        maximum_ledger_bytes: usize,
        maximum_ledger_records: usize,
        maximum_record_bytes: usize,
        maximum_object_bytes: usize,
    ) -> Result<Self, DurableProjectError> {
        let limits = Self {
            maximum_control_bytes,
            maximum_ledger_bytes,
            maximum_ledger_records,
            maximum_record_bytes,
            maximum_object_bytes,
        };
        limits.validate()?;
        Ok(limits)
    }

    fn validate(self) -> Result<(), DurableProjectError> {
        if self.maximum_control_bytes == 0
            || self.maximum_ledger_bytes == 0
            || self.maximum_ledger_records == 0
            || self.maximum_record_bytes == 0
            || self.maximum_object_bytes == 0
            || self.maximum_record_bytes > self.maximum_ledger_bytes
        {
            return Err(DurableProjectError::InvalidLimits);
        }
        Ok(())
    }
}

/// Exact provenance of the native executable driving a durable execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRuntimeProvenance {
    wire: NativeRuntimeWire,
    implementation_identity: String,
}

impl NativeRuntimeProvenance {
    /// Validate native build/runtime identity and derive its canonical implementation identity.
    pub fn new(
        crate_version: impl Into<String>,
        git_sha: Option<String>,
        git_dirty: Option<bool>,
        rustc_version: impl Into<String>,
        features: Vec<String>,
        executable: ArtifactRef,
    ) -> Result<Self, DurableProjectError> {
        let crate_version = crate_version.into();
        let rustc_version = rustc_version.into();
        validate_text("crate_version", &crate_version)?;
        validate_text("rustc_version", &rustc_version)?;
        if executable.kind() != "application/vnd.marklab.executable" {
            return Err(DurableProjectError::InvalidRuntime {
                reason: "executable reference has the wrong artifact kind".to_owned(),
            });
        }
        let git = match (git_sha, git_dirty) {
            (Some(sha), Some(dirty)) if valid_git_sha(&sha) => GitWire {
                availability: "available".to_owned(),
                sha: Some(sha),
                dirty: Some(dirty),
            },
            (None, None) => GitWire {
                availability: "unavailable".to_owned(),
                sha: None,
                dirty: None,
            },
            _ => {
                return Err(DurableProjectError::InvalidRuntime {
                    reason: "Git SHA and dirty state must both be available or both unavailable"
                        .to_owned(),
                });
            }
        };
        let mut features = features;
        if features.len() > MAX_FEATURES {
            return Err(DurableProjectError::InvalidRuntime {
                reason: format!("feature count exceeds {MAX_FEATURES}"),
            });
        }
        for feature in &features {
            if !valid_token(feature) {
                return Err(DurableProjectError::InvalidRuntime {
                    reason: format!("invalid feature token {feature:?}"),
                });
            }
        }
        features.sort();
        if features.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DurableProjectError::InvalidRuntime {
                reason: "compiled feature list contains duplicates".to_owned(),
            });
        }
        let wire = NativeRuntimeWire {
            backend: "native".to_owned(),
            crate_version,
            git,
            rustc_version,
            features,
            executable: ArtifactWire::from(&executable),
        };
        let encoded = canonical_compact(&wire, "native runtime")?;
        let implementation_identity = format!(
            "marklab-native-runtime-v1:{}",
            ContentDigest::from_bytes(&encoded)
        );
        Ok(Self {
            wire,
            implementation_identity,
        })
    }

    /// Stable identity to include in the workflow scheduler cache key.
    pub fn implementation_identity(&self) -> &str {
        &self.implementation_identity
    }

    fn from_wire(wire: NativeRuntimeWire) -> Result<Self, DurableProjectError> {
        let executable = wire.executable.to_artifact_ref()?;
        let (git_sha, git_dirty) = match wire.git.availability.as_str() {
            "available" => (wire.git.sha.clone(), wire.git.dirty),
            "unavailable" if wire.git.sha.is_none() && wire.git.dirty.is_none() => (None, None),
            _ => {
                return Err(DurableProjectError::InvalidRuntime {
                    reason: "invalid Git availability encoding".to_owned(),
                });
            }
        };
        let runtime = Self::new(
            wire.crate_version.clone(),
            git_sha,
            git_dirty,
            wire.rustc_version.clone(),
            wire.features.clone(),
            executable,
        )?;
        if runtime.wire != wire {
            return Err(DurableProjectError::InvalidRuntime {
                reason: "native runtime fields are not canonical".to_owned(),
            });
        }
        Ok(runtime)
    }
}

/// Exact single-node identity used for durable lookup and commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableExecutionRequest {
    node_id: String,
    node_spec_digest: ContentDigest,
    inputs: Vec<ArtifactRef>,
    configuration_digest: ContentDigest,
    execution_policy_digest: ContentDigest,
    scheduler_output_limit_bytes: u64,
    cache_key: ContentDigest,
    result_schema: ArtifactSchema,
    runtime: NativeRuntimeProvenance,
}

impl DurableExecutionRequest {
    /// Construct one bounded exact execution identity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: impl Into<String>,
        node_spec_digest: ContentDigest,
        inputs: Vec<ArtifactRef>,
        configuration_digest: ContentDigest,
        execution_policy_digest: ContentDigest,
        scheduler_output_limit_bytes: usize,
        cache_key: ContentDigest,
        result_schema: ArtifactSchema,
        runtime: NativeRuntimeProvenance,
    ) -> Result<Self, DurableProjectError> {
        let node_id = node_id.into();
        if !valid_token(&node_id) {
            return Err(DurableProjectError::InvalidRequest {
                reason: "node ID violates the shared token grammar".to_owned(),
            });
        }
        if inputs.is_empty() || inputs.len() > MAX_EXECUTION_INPUTS {
            return Err(DurableProjectError::InvalidRequest {
                reason: format!("input count must be in 1..={MAX_EXECUTION_INPUTS}"),
            });
        }
        let scheduler_output_limit_bytes =
            u64::try_from(scheduler_output_limit_bytes).map_err(|_| {
                DurableProjectError::InvalidRequest {
                    reason: "scheduler output limit exceeds u64".to_owned(),
                }
            })?;
        if scheduler_output_limit_bytes == 0 {
            return Err(DurableProjectError::InvalidRequest {
                reason: "scheduler output limit must be positive".to_owned(),
            });
        }
        Ok(Self {
            node_id,
            node_spec_digest,
            inputs,
            configuration_digest,
            execution_policy_digest,
            scheduler_output_limit_bytes,
            cache_key,
            result_schema,
            runtime,
        })
    }

    /// Scheduler-computed lookup key for this exact execution.
    pub fn cache_key(&self) -> ContentDigest {
        self.cache_key
    }

    fn identity_wire(&self) -> ExecutionIdentityWire {
        ExecutionIdentityWire {
            node: NodeWire {
                id: self.node_id.clone(),
                spec_digest: self.node_spec_digest.to_string(),
            },
            inputs: self.inputs.iter().map(ArtifactWire::from).collect(),
            configuration_digest: self.configuration_digest.to_string(),
            execution_policy_digest: self.execution_policy_digest.to_string(),
            scheduler_output_limit_bytes: self.scheduler_output_limit_bytes,
            runtime: self.runtime.wire.clone(),
            cache_key: self.cache_key.to_string(),
            result_schema: SchemaWire::from(&self.result_schema),
        }
    }
}

/// Project-level recovery action performed while opening durable state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableRecoveryAction {
    /// No project-level pending intent existed.
    None,
    /// A durable object and pending intent were completed into ledger and head.
    CompletedPendingExecution,
    /// An already-appended execution advanced a lagging project head.
    AdvancedProjectHead,
    /// A pending intent already represented by ledger and head was cleared.
    ClearedCommittedIntent,
}

/// Recovery evidence produced before a durable project becomes usable.
pub struct DurableOpenReport {
    action: DurableRecoveryAction,
    artifact_store: RecoveryReport,
}

impl DurableOpenReport {
    /// Project-level recovery action.
    pub fn action(&self) -> DurableRecoveryAction {
        self.action
    }

    /// Existing artifact-store staging recovery report.
    pub fn artifact_store(&self) -> &RecoveryReport {
        &self.artifact_store
    }
}

/// Exact durable replay found for an execution request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableReplay {
    sequence: u64,
    output: ArtifactRef,
}

impl DurableReplay {
    /// One-based append-only ledger sequence.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Verified output restored into the in-memory project.
    pub fn output(&self) -> &ArtifactRef {
        &self.output
    }
}

/// Disposition of one durable success commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableCommitDisposition {
    /// This call appended a new successful execution.
    Appended,
    /// The exact success already existed and was verified.
    AlreadyPresent,
}

/// Exclusive capability-confined owner of one durable local Marklab project.
pub struct DurableProject {
    root: Dir,
    _lock: fs::File,
    store: LocalArtifactStore,
    limits: DurableProjectLimits,
    head: ProjectHeadWire,
    ledger: Vec<ExecutionWire>,
    report: DurableOpenReport,
}

impl DurableProject {
    /// Create or open, recover, and fully validate one local project.
    pub fn open_or_create(
        root_path: &Path,
        limits: DurableProjectLimits,
    ) -> Result<Self, DurableProjectError> {
        limits.validate()?;
        ensure_project_root(root_path)?;
        let root = Dir::open_ambient_dir(root_path, ambient_authority()).map_err(|source| {
            DurableProjectError::Root {
                operation: "open",
                source,
            }
        })?;
        ensure_lock_file(&root)?;
        let lock = open_and_lock(&root)?;
        ensure_directory(&root, STORE_DIRECTORY)?;
        let store = LocalArtifactStore::open(
            &root_path.join(STORE_DIRECTORY),
            StoreId::new(STORE_ID).map_err(DurableProjectError::ArtifactRecord)?,
        )?;
        let store_report = store.recover_staging()?;
        let (mut head, mut ledger) = open_or_initialize_state(&root, root_path, limits)?;
        let action = recover_pending(&root, &store, limits, &mut head, &mut ledger)?;
        validate_head_against_ledger(&head, &ledger, ledger_byte_len(&ledger)?)?;
        Ok(Self {
            root,
            _lock: lock,
            store,
            limits,
            head,
            ledger,
            report: DurableOpenReport {
                action,
                artifact_store: store_report,
            },
        })
    }

    /// Recovery evidence collected during open.
    pub fn open_report(&self) -> &DurableOpenReport {
        &self.report
    }

    /// Stable project identity stored in the canonical head.
    pub fn project_id(&self) -> &str {
        &self.head.project_id
    }

    /// Number of durable successful executions.
    pub fn execution_count(&self) -> usize {
        self.ledger.len()
    }

    /// Restore an exact verified durable success into the existing in-memory project cache.
    pub fn restore_success(
        &self,
        request: &DurableExecutionRequest,
        project: &mut MarklabProject,
    ) -> Result<Option<DurableReplay>, DurableProjectError> {
        let requested = request.identity_wire();
        let Some(entry) = self
            .ledger
            .iter()
            .find(|entry| entry.identity.cache_key == requested.cache_key)
        else {
            return Ok(None);
        };
        if entry.identity != requested {
            return Err(DurableProjectError::ConflictingCacheIdentity {
                cache_key: request.cache_key,
            });
        }
        let (record, expected) = record_from_entry(entry)?;
        let bytes = read_verified_object(&self.store, &record, self.limits.maximum_object_bytes)?;
        let restored = project.commit_success(
            &request.node_id,
            request.cache_key,
            expected.kind(),
            bytes.into_boxed_slice(),
        )?;
        if restored != expected {
            return Err(DurableProjectError::RestoredOutputMismatch {
                cache_key: request.cache_key,
            });
        }
        Ok(Some(DurableReplay {
            sequence: entry.sequence,
            output: restored,
        }))
    }

    /// Publish and append one scheduler-committed canonical success.
    pub fn commit_success(
        &mut self,
        request: &DurableExecutionRequest,
        output_kind: &str,
        encoded_output: &[u8],
    ) -> Result<DurableCommitDisposition, DurableProjectError> {
        self.commit_success_inner(request, output_kind, encoded_output, CommitFault::None)
    }

    fn commit_success_inner(
        &mut self,
        request: &DurableExecutionRequest,
        output_kind: &str,
        encoded_output: &[u8],
        fault: CommitFault,
    ) -> Result<DurableCommitDisposition, DurableProjectError> {
        if encoded_output.len() > self.limits.maximum_object_bytes {
            return Err(DurableProjectError::ObjectTooLarge {
                observed: encoded_output.len(),
                maximum: self.limits.maximum_object_bytes,
            });
        }
        if let Some(existing) = self
            .ledger
            .iter()
            .find(|entry| entry.identity.cache_key == request.cache_key.to_string())
        {
            if existing.identity != request.identity_wire() {
                return Err(DurableProjectError::ConflictingCacheIdentity {
                    cache_key: request.cache_key,
                });
            }
            let (record, expected) = record_from_entry(existing)?;
            let bytes =
                read_verified_object(&self.store, &record, self.limits.maximum_object_bytes)?;
            expected.verify_bytes(&bytes)?;
            if bytes != encoded_output {
                return Err(DurableProjectError::RestoredOutputMismatch {
                    cache_key: request.cache_key,
                });
            }
            return Ok(DurableCommitDisposition::AlreadyPresent);
        }

        let prospective_records = self
            .ledger
            .len()
            .checked_add(1)
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        if prospective_records > self.limits.maximum_ledger_records {
            return Err(DurableProjectError::TooManyLedgerRecords {
                observed: prospective_records,
                maximum: self.limits.maximum_ledger_records,
            });
        }

        let content = ArtifactRef::from_bytes(output_kind, encoded_output)?;
        let draft = output_draft(request, content.clone())?;
        let publication = self
            .store
            .publish_new_send(&draft, |writer| writer.write_all(encoded_output))?;
        let record = publication.into_record();
        if fault == CommitFault::AfterObjectPublication {
            return Err(DurableProjectError::InjectedFailure {
                stage: "after object publication",
            });
        }

        let sequence = u64::try_from(self.ledger.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        let previous_record_digest = self
            .ledger
            .last()
            .map(execution_digest)
            .transpose()?
            .map(|digest| digest.to_string());
        let recorded_unix_ms = monotonic_timestamp(self.ledger.last())?;
        let entry = ExecutionWire {
            format: EXECUTION_FORMAT.to_owned(),
            version: FORMAT_VERSION,
            sequence,
            previous_record_digest,
            recorded_unix_ms,
            identity: request.identity_wire(),
            output: OutputWire {
                artifact_id: record.id().to_string(),
                semantic_digest: record.semantic_digest().to_string(),
                content: ArtifactWire::from(record.content()),
            },
            terminal_disposition: "success".to_owned(),
        };
        validate_execution(&entry, self.limits)?;
        let pending = PendingWire {
            format: PENDING_FORMAT.to_owned(),
            version: FORMAT_VERSION,
            execution: entry.clone(),
        };
        let pending_bytes = canonical_pretty(&pending, "pending execution")?;
        ensure_size(
            PENDING_PATH,
            pending_bytes.len(),
            self.limits.maximum_control_bytes,
        )?;
        prepare_execution_append(&self.root, &entry, self.limits)?;
        atomic_write(&self.root, PENDING_PATH, &pending_bytes)?;
        if fault == CommitFault::AfterPendingIntent {
            return Err(DurableProjectError::InjectedFailure {
                stage: "after pending intent",
            });
        }

        append_execution(&self.root, &entry, self.limits)?;
        self.ledger.push(entry.clone());
        if fault == CommitFault::AfterLedgerAppend {
            return Err(DurableProjectError::InjectedFailure {
                stage: "after ledger append",
            });
        }

        self.head = head_for_execution(
            self.head.project_id.clone(),
            &entry,
            self.ledger.len(),
            ledger_byte_len(&self.ledger)?,
        )?;
        write_head(&self.root, &self.head, self.limits)?;
        if fault == CommitFault::AfterHeadReplacement {
            return Err(DurableProjectError::InjectedFailure {
                stage: "after head replacement",
            });
        }
        remove_pending(&self.root)?;
        Ok(DurableCommitDisposition::Appended)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitFault {
    None,
    AfterObjectPublication,
    AfterPendingIntent,
    AfterLedgerAppend,
    AfterHeadReplacement,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactWire {
    kind: String,
    digest: String,
    byte_len: u64,
}

impl From<&ArtifactRef> for ArtifactWire {
    fn from(value: &ArtifactRef) -> Self {
        Self {
            kind: value.kind().to_owned(),
            digest: value.digest().to_string(),
            byte_len: value.byte_len(),
        }
    }
}

impl ArtifactWire {
    fn to_artifact_ref(&self) -> Result<ArtifactRef, DurableProjectError> {
        Ok(ArtifactRef::new(
            self.kind.clone(),
            parse_digest(&self.digest)?,
            self.byte_len,
        )?)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GitWire {
    availability: String,
    sha: Option<String>,
    dirty: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeRuntimeWire {
    backend: String,
    crate_version: String,
    git: GitWire,
    rustc_version: String,
    features: Vec<String>,
    executable: ArtifactWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SchemaWire {
    id: String,
    version: u32,
}

impl From<&ArtifactSchema> for SchemaWire {
    fn from(value: &ArtifactSchema) -> Self {
        Self {
            id: value.id().to_owned(),
            version: value.version(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeWire {
    id: String,
    spec_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutionIdentityWire {
    node: NodeWire,
    inputs: Vec<ArtifactWire>,
    configuration_digest: String,
    execution_policy_digest: String,
    scheduler_output_limit_bytes: u64,
    runtime: NativeRuntimeWire,
    cache_key: String,
    result_schema: SchemaWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OutputWire {
    artifact_id: String,
    semantic_digest: String,
    content: ArtifactWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutionWire {
    format: String,
    version: u32,
    sequence: u64,
    previous_record_digest: Option<String>,
    recorded_unix_ms: u64,
    identity: ExecutionIdentityWire,
    output: OutputWire,
    terminal_disposition: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PendingWire {
    format: String,
    version: u32,
    execution: ExecutionWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoreWire {
    id: String,
    policy: String,
    directory: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LedgerHeadWire {
    path: String,
    record_count: u64,
    byte_len: u64,
    tail_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LatestExecutionWire {
    sequence: u64,
    node_id: String,
    cache_key: String,
    output_artifact_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectHeadWire {
    format: String,
    version: u32,
    project_id: String,
    store: StoreWire,
    ledger: LedgerHeadWire,
    latest_execution: Option<LatestExecutionWire>,
    artifacts: Vec<ArtifactWire>,
}

fn ensure_project_root(root: &Path) -> Result<(), DurableProjectError> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(DurableProjectError::SymlinkBoundary {
                path: "<project-root>".to_owned(),
            })
        }
        Ok(metadata) if !metadata.is_dir() => Err(DurableProjectError::UnsupportedPathType {
            path: "<project-root>".to_owned(),
        }),
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(root).map_err(|source| DurableProjectError::Root {
                operation: "create",
                source,
            })?;
            let metadata =
                fs::symlink_metadata(root).map_err(|source| DurableProjectError::Root {
                    operation: "inspect created",
                    source,
                })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(DurableProjectError::UnsupportedPathType {
                    path: "<project-root>".to_owned(),
                });
            }
            Ok(())
        }
        Err(source) => Err(DurableProjectError::Root {
            operation: "inspect",
            source,
        }),
    }
}

fn ensure_directory(root: &Dir, path: &str) -> Result<(), DurableProjectError> {
    match root.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(DurableProjectError::SymlinkBoundary {
                path: path.to_owned(),
            })
        }
        Ok(metadata) if !metadata.is_dir() => Err(DurableProjectError::UnsupportedPathType {
            path: path.to_owned(),
        }),
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            root.create_dir(path)
                .map_err(|source| DurableProjectError::Io {
                    operation: "create directory",
                    path: path.to_owned(),
                    source,
                })?;
            sync_root(root)?;
            Ok(())
        }
        Err(source) => Err(DurableProjectError::Io {
            operation: "inspect directory",
            path: path.to_owned(),
            source,
        }),
    }
}

fn ensure_lock_file(root: &Dir) -> Result<(), DurableProjectError> {
    match root.symlink_metadata(LOCK_PATH) {
        Ok(metadata) => validate_regular_metadata(LOCK_PATH, metadata),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            match root.open_with(LOCK_PATH, &options) {
                Ok(file) => {
                    file.sync_all().map_err(|source| DurableProjectError::Io {
                        operation: "sync project lock",
                        path: LOCK_PATH.to_owned(),
                        source,
                    })?;
                    sync_root(root)
                }
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                    let metadata = root.symlink_metadata(LOCK_PATH).map_err(|source| {
                        DurableProjectError::Io {
                            operation: "inspect raced project lock",
                            path: LOCK_PATH.to_owned(),
                            source,
                        }
                    })?;
                    validate_regular_metadata(LOCK_PATH, metadata)
                }
                Err(source) => Err(DurableProjectError::Io {
                    operation: "create project lock",
                    path: LOCK_PATH.to_owned(),
                    source,
                }),
            }
        }
        Err(source) => Err(DurableProjectError::Io {
            operation: "inspect project lock",
            path: LOCK_PATH.to_owned(),
            source,
        }),
    }
}

fn open_and_lock(root: &Dir) -> Result<fs::File, DurableProjectError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    let file = root
        .open_with(LOCK_PATH, &options)
        .map_err(|source| DurableProjectError::Io {
            operation: "open project lock",
            path: LOCK_PATH.to_owned(),
            source,
        })?;
    if !file
        .metadata()
        .map_err(|source| DurableProjectError::Io {
            operation: "inspect opened project lock",
            path: LOCK_PATH.to_owned(),
            source,
        })?
        .is_file()
    {
        return Err(DurableProjectError::UnsupportedPathType {
            path: LOCK_PATH.to_owned(),
        });
    }
    let file = file.into_std();
    file.lock().map_err(|source| DurableProjectError::Io {
        operation: "lock project",
        path: LOCK_PATH.to_owned(),
        source,
    })?;
    Ok(file)
}

fn open_or_initialize_state(
    root: &Dir,
    root_path: &Path,
    limits: DurableProjectLimits,
) -> Result<(ProjectHeadWire, Vec<ExecutionWire>), DurableProjectError> {
    let head_exists = path_exists(root, HEAD_PATH)?;
    let ledger_exists = path_exists(root, LEDGER_PATH)?;
    match (head_exists, ledger_exists) {
        (false, false) => {
            create_empty_ledger(root)?;
            let head = empty_head(generate_project_id(root_path)?);
            write_head(root, &head, limits)?;
            Ok((head, Vec::new()))
        }
        (true, true) => {
            let head = read_head(root, limits)?;
            let (ledger, byte_len) = read_ledger(root, limits)?;
            if !path_exists(root, PENDING_PATH)? {
                validate_head_against_ledger(&head, &ledger, byte_len)?;
            }
            Ok((head, ledger))
        }
        _ => Err(DurableProjectError::IncompleteProjectState),
    }
}

fn empty_head(project_id: String) -> ProjectHeadWire {
    ProjectHeadWire {
        format: PROJECT_FORMAT.to_owned(),
        version: FORMAT_VERSION,
        project_id,
        store: StoreWire {
            id: STORE_ID.to_owned(),
            policy: STORE_POLICY.to_owned(),
            directory: STORE_DIRECTORY.to_owned(),
        },
        ledger: LedgerHeadWire {
            path: LEDGER_PATH.to_owned(),
            record_count: 0,
            byte_len: 0,
            tail_digest: None,
        },
        latest_execution: None,
        artifacts: Vec::new(),
    }
}

fn generate_project_id(root: &Path) -> Result<String, DurableProjectError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DurableProjectError::ClockBeforeEpoch)?
        .as_nanos()
        .to_be_bytes();
    let process = std::process::id().to_be_bytes();
    Ok(ContentDigest::from_framed([
        b"marklab-project-id-v1".as_slice(),
        root.as_os_str().to_string_lossy().as_bytes(),
        now.as_slice(),
        process.as_slice(),
    ])
    .to_string())
}

fn create_empty_ledger(root: &Dir) -> Result<(), DurableProjectError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let file = root
        .open_with(LEDGER_PATH, &options)
        .map_err(|source| DurableProjectError::Io {
            operation: "create execution ledger",
            path: LEDGER_PATH.to_owned(),
            source,
        })?;
    file.sync_all().map_err(|source| DurableProjectError::Io {
        operation: "sync execution ledger",
        path: LEDGER_PATH.to_owned(),
        source,
    })?;
    sync_root(root)
}

fn read_head(
    root: &Dir,
    limits: DurableProjectLimits,
) -> Result<ProjectHeadWire, DurableProjectError> {
    let bytes = read_bounded(root, HEAD_PATH, limits.maximum_control_bytes)?;
    let head: ProjectHeadWire = decode_canonical_pretty(HEAD_PATH, &bytes)?;
    validate_head_shape(&head)?;
    Ok(head)
}

fn validate_head_shape(head: &ProjectHeadWire) -> Result<(), DurableProjectError> {
    if head.format != PROJECT_FORMAT {
        return Err(DurableProjectError::UnsupportedFormat {
            path: HEAD_PATH.to_owned(),
            observed: head.format.clone(),
        });
    }
    if head.version != FORMAT_VERSION {
        return Err(DurableProjectError::UnsupportedVersion {
            path: HEAD_PATH.to_owned(),
            observed: head.version,
        });
    }
    if !valid_digest(&head.project_id)
        || head.store.id != STORE_ID
        || head.store.policy != STORE_POLICY
        || head.store.directory != STORE_DIRECTORY
        || head.ledger.path != LEDGER_PATH
    {
        return Err(DurableProjectError::InvalidHead);
    }
    if let Some(tail) = &head.ledger.tail_digest {
        parse_digest(tail)?;
    }
    for artifact in &head.artifacts {
        artifact.to_artifact_ref()?;
    }
    Ok(())
}

fn read_ledger(
    root: &Dir,
    limits: DurableProjectLimits,
) -> Result<(Vec<ExecutionWire>, u64), DurableProjectError> {
    let bytes = read_bounded(root, LEDGER_PATH, limits.maximum_ledger_bytes)?;
    if bytes.is_empty() {
        return Ok((Vec::new(), 0));
    }
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        return Err(DurableProjectError::TruncatedLedger);
    }
    let mut ledger = Vec::new();
    let mut previous_digest = None;
    let mut cache_keys = BTreeSet::new();
    let mut previous_time = 0;
    let records = &bytes[..bytes.len() - 1];
    for (index, line) in records.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            return Err(DurableProjectError::MalformedLedger {
                reason: "empty record".to_owned(),
            });
        }
        if line.len() > limits.maximum_record_bytes {
            return Err(DurableProjectError::StateTooLarge {
                path: LEDGER_PATH.to_owned(),
                observed: line.len(),
                maximum: limits.maximum_record_bytes,
            });
        }
        if index == limits.maximum_ledger_records {
            return Err(DurableProjectError::TooManyLedgerRecords {
                observed: index + 1,
                maximum: limits.maximum_ledger_records,
            });
        }
        let entry: ExecutionWire = decode_canonical_compact(LEDGER_PATH, line)?;
        validate_execution(&entry, limits)?;
        let expected_sequence = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        if entry.sequence != expected_sequence
            || entry.previous_record_digest != previous_digest
            || entry.recorded_unix_ms < previous_time
        {
            return Err(DurableProjectError::InvalidLedgerChain {
                sequence: entry.sequence,
            });
        }
        if !cache_keys.insert(entry.identity.cache_key.clone()) {
            return Err(DurableProjectError::DuplicateLedgerCacheKey {
                cache_key: parse_digest(&entry.identity.cache_key)?,
            });
        }
        previous_digest = Some(ContentDigest::from_bytes(line).to_string());
        previous_time = entry.recorded_unix_ms;
        ledger.push(entry);
    }
    Ok((
        ledger,
        u64::try_from(bytes.len()).map_err(|_| DurableProjectError::LedgerSequenceOverflow)?,
    ))
}

fn validate_execution(
    entry: &ExecutionWire,
    limits: DurableProjectLimits,
) -> Result<(), DurableProjectError> {
    if entry.format != EXECUTION_FORMAT {
        return Err(DurableProjectError::UnsupportedFormat {
            path: LEDGER_PATH.to_owned(),
            observed: entry.format.clone(),
        });
    }
    if entry.version != FORMAT_VERSION {
        return Err(DurableProjectError::UnsupportedVersion {
            path: LEDGER_PATH.to_owned(),
            observed: entry.version,
        });
    }
    if entry.sequence == 0
        || entry.recorded_unix_ms == 0
        || entry.terminal_disposition != "success"
        || !valid_token(&entry.identity.node.id)
        || entry.identity.inputs.is_empty()
        || entry.identity.inputs.len() > MAX_EXECUTION_INPUTS
        || entry.identity.scheduler_output_limit_bytes == 0
    {
        return Err(DurableProjectError::MalformedLedger {
            reason: "invalid required execution field".to_owned(),
        });
    }
    if let Some(previous) = &entry.previous_record_digest {
        parse_digest(previous)?;
    }
    parse_digest(&entry.identity.node.spec_digest)?;
    parse_digest(&entry.identity.configuration_digest)?;
    parse_digest(&entry.identity.execution_policy_digest)?;
    parse_digest(&entry.identity.cache_key)?;
    for input in &entry.identity.inputs {
        input.to_artifact_ref()?;
    }
    ArtifactSchema::new(
        entry.identity.result_schema.id.clone(),
        entry.identity.result_schema.version,
    )?;
    NativeRuntimeProvenance::from_wire(entry.identity.runtime.clone())?;
    let content = entry.output.content.to_artifact_ref()?;
    let maximum = u64::try_from(limits.maximum_object_bytes).unwrap_or(u64::MAX);
    if content.byte_len() > maximum {
        return Err(DurableProjectError::ObjectTooLarge {
            observed: usize::try_from(content.byte_len()).unwrap_or(usize::MAX),
            maximum: limits.maximum_object_bytes,
        });
    }
    record_from_entry(entry)?;
    Ok(())
}

fn validate_head_against_ledger(
    head: &ProjectHeadWire,
    ledger: &[ExecutionWire],
    byte_len: u64,
) -> Result<(), DurableProjectError> {
    validate_head_shape(head)?;
    let count =
        u64::try_from(ledger.len()).map_err(|_| DurableProjectError::LedgerSequenceOverflow)?;
    let tail = ledger
        .last()
        .map(execution_digest)
        .transpose()?
        .map(|digest| digest.to_string());
    if head.ledger.record_count != count
        || head.ledger.byte_len != byte_len
        || head.ledger.tail_digest != tail
    {
        return Err(DurableProjectError::HeadLedgerMismatch);
    }
    match (ledger.last(), &head.latest_execution) {
        (None, None) if head.artifacts.is_empty() => Ok(()),
        (Some(entry), Some(latest)) => {
            let expected =
                head_for_execution(head.project_id.clone(), entry, ledger.len(), byte_len)?;
            if latest == expected.latest_execution.as_ref().expect("latest")
                && head.artifacts == expected.artifacts
            {
                Ok(())
            } else {
                Err(DurableProjectError::HeadLedgerMismatch)
            }
        }
        _ => Err(DurableProjectError::HeadLedgerMismatch),
    }
}

fn head_for_execution(
    project_id: String,
    entry: &ExecutionWire,
    record_count: usize,
    byte_len: u64,
) -> Result<ProjectHeadWire, DurableProjectError> {
    let count =
        u64::try_from(record_count).map_err(|_| DurableProjectError::LedgerSequenceOverflow)?;
    let mut artifacts = entry.identity.inputs.clone();
    artifacts.push(entry.output.content.clone());
    Ok(ProjectHeadWire {
        format: PROJECT_FORMAT.to_owned(),
        version: FORMAT_VERSION,
        project_id,
        store: StoreWire {
            id: STORE_ID.to_owned(),
            policy: STORE_POLICY.to_owned(),
            directory: STORE_DIRECTORY.to_owned(),
        },
        ledger: LedgerHeadWire {
            path: LEDGER_PATH.to_owned(),
            record_count: count,
            byte_len,
            tail_digest: Some(execution_digest(entry)?.to_string()),
        },
        latest_execution: Some(LatestExecutionWire {
            sequence: entry.sequence,
            node_id: entry.identity.node.id.clone(),
            cache_key: entry.identity.cache_key.clone(),
            output_artifact_id: entry.output.artifact_id.clone(),
        }),
        artifacts,
    })
}

fn recover_pending(
    root: &Dir,
    store: &LocalArtifactStore,
    limits: DurableProjectLimits,
    head: &mut ProjectHeadWire,
    ledger: &mut Vec<ExecutionWire>,
) -> Result<DurableRecoveryAction, DurableProjectError> {
    if !path_exists(root, PENDING_PATH)? {
        return Ok(DurableRecoveryAction::None);
    }
    let bytes = read_bounded(root, PENDING_PATH, limits.maximum_control_bytes)?;
    let pending: PendingWire = decode_canonical_pretty(PENDING_PATH, &bytes)?;
    if pending.format != PENDING_FORMAT || pending.version != FORMAT_VERSION {
        return Err(DurableProjectError::UnsupportedVersion {
            path: PENDING_PATH.to_owned(),
            observed: pending.version,
        });
    }
    validate_execution(&pending.execution, limits)?;
    let (record, _) = record_from_entry(&pending.execution)?;
    store.verify(&record)?;

    let prior_count = ledger.len();
    let expected_next = u64::try_from(prior_count)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
    let expected_previous = ledger
        .last()
        .map(execution_digest)
        .transpose()?
        .map(|digest| digest.to_string());
    if pending.execution.sequence == expected_next
        && pending.execution.previous_record_digest == expected_previous
    {
        let prospective_records = prior_count
            .checked_add(1)
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        if prospective_records > limits.maximum_ledger_records {
            return Err(DurableProjectError::TooManyLedgerRecords {
                observed: prospective_records,
                maximum: limits.maximum_ledger_records,
            });
        }
        append_execution(root, &pending.execution, limits)?;
        ledger.push(pending.execution.clone());
        *head = head_for_execution(
            head.project_id.clone(),
            &pending.execution,
            ledger.len(),
            ledger_byte_len(ledger)?,
        )?;
        write_head(root, head, limits)?;
        remove_pending(root)?;
        return Ok(DurableRecoveryAction::CompletedPendingExecution);
    }

    if ledger.last() == Some(&pending.execution) {
        let target = head_for_execution(
            head.project_id.clone(),
            &pending.execution,
            ledger.len(),
            ledger_byte_len(ledger)?,
        )?;
        let action = if *head == target {
            DurableRecoveryAction::ClearedCommittedIntent
        } else {
            *head = target;
            write_head(root, head, limits)?;
            DurableRecoveryAction::AdvancedProjectHead
        };
        remove_pending(root)?;
        return Ok(action);
    }
    Err(DurableProjectError::ConflictingPendingExecution)
}

fn output_draft(
    request: &DurableExecutionRequest,
    content: ArtifactRef,
) -> Result<ArtifactDraft, DurableProjectError> {
    let mut metadata = BTreeMap::new();
    metadata.insert("cache_key".to_owned(), request.cache_key.to_string());
    metadata.insert("node_id".to_owned(), request.node_id.clone());
    metadata.insert(
        "result_schema_id".to_owned(),
        request.result_schema.id().to_owned(),
    );
    metadata.insert(
        "result_schema_version".to_owned(),
        request.result_schema.version().to_string(),
    );
    Ok(ArtifactDraft::new(
        content,
        ArtifactSchema::new(OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION)?,
        None,
        Vec::new(),
        metadata,
    )?)
}

fn record_from_entry(
    entry: &ExecutionWire,
) -> Result<(ArtifactRecord, ArtifactRef), DurableProjectError> {
    let runtime = NativeRuntimeProvenance::from_wire(entry.identity.runtime.clone())?;
    let request = DurableExecutionRequest::new(
        entry.identity.node.id.clone(),
        parse_digest(&entry.identity.node.spec_digest)?,
        entry
            .identity
            .inputs
            .iter()
            .map(ArtifactWire::to_artifact_ref)
            .collect::<Result<Vec<_>, _>>()?,
        parse_digest(&entry.identity.configuration_digest)?,
        parse_digest(&entry.identity.execution_policy_digest)?,
        usize::try_from(entry.identity.scheduler_output_limit_bytes).map_err(|_| {
            DurableProjectError::InvalidRequest {
                reason: "scheduler limit exceeds usize".to_owned(),
            }
        })?,
        parse_digest(&entry.identity.cache_key)?,
        ArtifactSchema::new(
            entry.identity.result_schema.id.clone(),
            entry.identity.result_schema.version,
        )?,
        runtime,
    )?;
    let content = entry.output.content.to_artifact_ref()?;
    let draft = output_draft(&request, content.clone())?;
    let claimed_id = crate::ArtifactId::from_str(&entry.output.artifact_id)
        .map_err(|_| DurableProjectError::InvalidArtifactIdentity)?;
    let claimed_semantic = parse_digest(&entry.output.semantic_digest)?;
    if draft.id() != claimed_id || draft.semantic_digest() != claimed_semantic {
        return Err(DurableProjectError::InvalidArtifactIdentity);
    }
    let record = draft.located_record(ArtifactLocator::managed(
        StoreId::new(STORE_ID).map_err(DurableProjectError::ArtifactRecord)?,
        draft.id(),
    ));
    Ok((record, content))
}

fn read_verified_object(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    maximum: usize,
) -> Result<Vec<u8>, DurableProjectError> {
    let declared = usize::try_from(record.content().byte_len()).map_err(|_| {
        DurableProjectError::ObjectTooLarge {
            observed: usize::MAX,
            maximum,
        }
    })?;
    if declared > maximum {
        return Err(DurableProjectError::ObjectTooLarge {
            observed: declared,
            maximum,
        });
    }
    store
        .with_verified_reader(record, |reader| {
            let capacity = declared.min(64 * 1024);
            let mut bytes = Vec::with_capacity(capacity);
            reader
                .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
                .read_to_end(&mut bytes)?;
            if bytes.len() > maximum {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "durable object exceeds materialization limit",
                ));
            }
            Ok(bytes)
        })
        .map_err(|error| match error {
            VerifiedReaderError::Store(source) => DurableProjectError::ArtifactStore(source),
            VerifiedReaderError::Callback(source) => DurableProjectError::Io {
                operation: "read verified durable object",
                path: STORE_DIRECTORY.to_owned(),
                source,
            },
        })
}

fn append_execution(
    root: &Dir,
    entry: &ExecutionWire,
    limits: DurableProjectLimits,
) -> Result<(), DurableProjectError> {
    let encoded = prepare_execution_append(root, entry, limits)?;
    let mut options = OpenOptions::new();
    options.write(true).append(true);
    let mut file =
        root.open_with(LEDGER_PATH, &options)
            .map_err(|source| DurableProjectError::Io {
                operation: "open execution ledger for append",
                path: LEDGER_PATH.to_owned(),
                source,
            })?;
    file.write_all(&encoded)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|source| DurableProjectError::Io {
            operation: "append and sync execution ledger",
            path: LEDGER_PATH.to_owned(),
            source,
        })
}

fn prepare_execution_append(
    root: &Dir,
    entry: &ExecutionWire,
    limits: DurableProjectLimits,
) -> Result<Vec<u8>, DurableProjectError> {
    let encoded = canonical_compact(entry, "execution record")?;
    ensure_size(LEDGER_PATH, encoded.len(), limits.maximum_record_bytes)?;
    let current = root
        .symlink_metadata(LEDGER_PATH)
        .map_err(|source| DurableProjectError::Io {
            operation: "inspect execution ledger",
            path: LEDGER_PATH.to_owned(),
            source,
        })?;
    validate_regular_metadata(LEDGER_PATH, current)?;
    let current_len = usize::try_from(
        root.metadata(LEDGER_PATH)
            .map_err(|source| DurableProjectError::Io {
                operation: "measure execution ledger",
                path: LEDGER_PATH.to_owned(),
                source,
            })?
            .len(),
    )
    .unwrap_or(usize::MAX);
    let appended = encoded.len().saturating_add(1);
    if current_len.saturating_add(appended) > limits.maximum_ledger_bytes {
        return Err(DurableProjectError::StateTooLarge {
            path: LEDGER_PATH.to_owned(),
            observed: current_len.saturating_add(appended),
            maximum: limits.maximum_ledger_bytes,
        });
    }
    Ok(encoded)
}

fn ledger_byte_len(ledger: &[ExecutionWire]) -> Result<u64, DurableProjectError> {
    ledger.iter().try_fold(0_u64, |total, entry| {
        let encoded = canonical_compact(entry, "execution record")?;
        let length = u64::try_from(encoded.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        total
            .checked_add(length)
            .ok_or(DurableProjectError::LedgerSequenceOverflow)
    })
}

fn execution_digest(entry: &ExecutionWire) -> Result<ContentDigest, DurableProjectError> {
    Ok(ContentDigest::from_bytes(&canonical_compact(
        entry,
        "execution record",
    )?))
}

fn monotonic_timestamp(previous: Option<&ExecutionWire>) -> Result<u64, DurableProjectError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DurableProjectError::ClockBeforeEpoch)?
        .as_millis();
    let now = u64::try_from(millis).map_err(|_| DurableProjectError::ClockOverflow)?;
    Ok(previous.map_or(now, |entry| now.max(entry.recorded_unix_ms)))
}

fn write_head(
    root: &Dir,
    head: &ProjectHeadWire,
    limits: DurableProjectLimits,
) -> Result<(), DurableProjectError> {
    validate_head_shape(head)?;
    let bytes = canonical_pretty(head, "project head")?;
    ensure_size(HEAD_PATH, bytes.len(), limits.maximum_control_bytes)?;
    atomic_write(root, HEAD_PATH, &bytes)
}

fn atomic_write(root: &Dir, destination: &str, bytes: &[u8]) -> Result<(), DurableProjectError> {
    let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let staging = format!(".{destination}.tmp-{}-{sequence}", std::process::id());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file =
        root.open_with(&staging, &options)
            .map_err(|source| DurableProjectError::Io {
                operation: "create control staging file",
                path: staging.clone(),
                source,
            })?;
    let write_result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(source) = write_result {
        let _ = root.remove_file(&staging);
        return Err(DurableProjectError::Io {
            operation: "write and sync control staging file",
            path: staging,
            source,
        });
    }
    if let Err(source) = root.rename(&staging, root, destination) {
        let _ = root.remove_file(&staging);
        return Err(DurableProjectError::Io {
            operation: "replace durable control file",
            path: destination.to_owned(),
            source,
        });
    }
    sync_root(root)
}

fn remove_pending(root: &Dir) -> Result<(), DurableProjectError> {
    root.remove_file(PENDING_PATH)
        .map_err(|source| DurableProjectError::Io {
            operation: "remove completed pending execution",
            path: PENDING_PATH.to_owned(),
            source,
        })?;
    sync_root(root)
}

fn read_bounded(root: &Dir, path: &str, maximum: usize) -> Result<Vec<u8>, DurableProjectError> {
    let metadata = root
        .symlink_metadata(path)
        .map_err(|source| DurableProjectError::Io {
            operation: "inspect durable control file",
            path: path.to_owned(),
            source,
        })?;
    validate_regular_metadata(path, metadata)?;
    let mut options = OpenOptions::new();
    options.read(true);
    let file = root
        .open_with(path, &options)
        .map_err(|source| DurableProjectError::Io {
            operation: "open durable control file",
            path: path.to_owned(),
            source,
        })?;
    if !file
        .metadata()
        .map_err(|source| DurableProjectError::Io {
            operation: "inspect opened durable control file",
            path: path.to_owned(),
            source,
        })?
        .is_file()
    {
        return Err(DurableProjectError::UnsupportedPathType {
            path: path.to_owned(),
        });
    }
    let limit = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|source| DurableProjectError::Io {
            operation: "read durable control file",
            path: path.to_owned(),
            source,
        })?;
    ensure_size(path, bytes.len(), maximum)?;
    Ok(bytes)
}

fn path_exists(root: &Dir, path: &str) -> Result<bool, DurableProjectError> {
    match root.symlink_metadata(path) {
        Ok(metadata) => {
            validate_regular_metadata(path, metadata)?;
            Ok(true)
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(DurableProjectError::Io {
            operation: "inspect durable control path",
            path: path.to_owned(),
            source,
        }),
    }
}

fn validate_regular_metadata(
    path: &str,
    metadata: cap_std::fs::Metadata,
) -> Result<(), DurableProjectError> {
    if metadata.file_type().is_symlink() {
        Err(DurableProjectError::SymlinkBoundary {
            path: path.to_owned(),
        })
    } else if !metadata.is_file() {
        Err(DurableProjectError::UnsupportedPathType {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn sync_root(root: &Dir) -> Result<(), DurableProjectError> {
    root.try_clone()
        .and_then(|directory| directory.into_std_file().sync_all())
        .map_err(|source| DurableProjectError::Io {
            operation: "sync project root",
            path: ".".to_owned(),
            source,
        })
}

#[cfg(windows)]
fn sync_root(root: &Dir) -> Result<(), DurableProjectError> {
    use cap_std::fs::OpenOptionsExt;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let mut options = OpenOptions::new();
    options.write(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    root.open_with(".", &options)
        .and_then(|file| file.sync_all())
        .map_err(|source| DurableProjectError::Io {
            operation: "sync project root",
            path: ".".to_owned(),
            source,
        })
}

fn canonical_compact<T: Serialize>(
    value: &T,
    context: &'static str,
) -> Result<Vec<u8>, DurableProjectError> {
    serde_json::to_vec(value).map_err(|source| DurableProjectError::Json {
        path: context.to_owned(),
        reason: source.to_string(),
    })
}

fn canonical_pretty<T: Serialize>(
    value: &T,
    context: &'static str,
) -> Result<Vec<u8>, DurableProjectError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|source| DurableProjectError::Json {
            path: context.to_owned(),
            reason: source.to_string(),
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn decode_canonical_compact<T>(path: &str, bytes: &[u8]) -> Result<T, DurableProjectError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let value: T = serde_json::from_slice(bytes).map_err(|source| DurableProjectError::Json {
        path: path.to_owned(),
        reason: source.to_string(),
    })?;
    if canonical_compact(&value, "decoded compact state")? != bytes {
        return Err(DurableProjectError::NonCanonicalState {
            path: path.to_owned(),
        });
    }
    Ok(value)
}

fn decode_canonical_pretty<T>(path: &str, bytes: &[u8]) -> Result<T, DurableProjectError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let value: T = serde_json::from_slice(bytes).map_err(|source| DurableProjectError::Json {
        path: path.to_owned(),
        reason: source.to_string(),
    })?;
    if canonical_pretty(&value, "decoded pretty state")? != bytes {
        return Err(DurableProjectError::NonCanonicalState {
            path: path.to_owned(),
        });
    }
    Ok(value)
}

fn ensure_size(path: &str, observed: usize, maximum: usize) -> Result<(), DurableProjectError> {
    if observed > maximum {
        Err(DurableProjectError::StateTooLarge {
            path: path.to_owned(),
            observed,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn parse_digest(value: &str) -> Result<ContentDigest, DurableProjectError> {
    ContentDigest::from_str(value).map_err(|_| DurableProjectError::InvalidDigest {
        value: value.to_owned(),
    })
}

fn valid_digest(value: &str) -> bool {
    ContentDigest::from_str(value).is_ok()
}

fn valid_git_sha(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn validate_text(field: &str, value: &str) -> Result<(), DurableProjectError> {
    if value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        Err(DurableProjectError::InvalidRuntime {
            reason: format!("{field} must be bounded visible ASCII"),
        })
    } else {
        Ok(())
    }
}

/// Durable project validation, integrity, recovery, and I/O failures.
#[derive(Debug, Error)]
pub enum DurableProjectError {
    /// At least one limit is zero or the record limit exceeds the ledger limit.
    #[error(
        "durable project limits must be positive and record bytes may not exceed ledger bytes"
    )]
    InvalidLimits,
    /// Native runtime provenance is incomplete or malformed.
    #[error("invalid native runtime provenance: {reason}")]
    InvalidRuntime {
        /// Stable rejection reason.
        reason: String,
    },
    /// Execution request violates the bounded single-node contract.
    #[error("invalid durable execution request: {reason}")]
    InvalidRequest {
        /// Stable rejection reason.
        reason: String,
    },
    /// Project-root creation, inspection, or opening failed.
    #[error("failed to {operation} durable project root: {source}")]
    Root {
        /// Failed operation.
        operation: &'static str,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// A project control path crosses a symbolic link.
    #[error("durable project path crosses a symlink boundary at {path:?}")]
    SymlinkBoundary {
        /// Root-relative path or root marker.
        path: String,
    },
    /// A path expected to be a directory or regular file has another type.
    #[error("durable project path has an unsupported file type at {path:?}")]
    UnsupportedPathType {
        /// Root-relative path or root marker.
        path: String,
    },
    /// Capability-relative project I/O failed.
    #[error("failed to {operation} at durable project path {path:?}: {source}")]
    Io {
        /// Failed operation.
        operation: &'static str,
        /// Root-relative path.
        path: String,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// Canonical state exceeds a configured bound.
    #[error("durable state {path:?} has {observed} bytes, exceeding {maximum}")]
    StateTooLarge {
        /// Affected control path.
        path: String,
        /// Observed bytes.
        observed: usize,
        /// Configured maximum bytes.
        maximum: usize,
    },
    /// Ledger contains too many records.
    #[error("execution ledger has {observed} records, exceeding {maximum}")]
    TooManyLedgerRecords {
        /// Observed records.
        observed: usize,
        /// Configured maximum records.
        maximum: usize,
    },
    /// One output exceeds the project object materialization limit.
    #[error("durable output has {observed} bytes, exceeding {maximum}")]
    ObjectTooLarge {
        /// Observed bytes.
        observed: usize,
        /// Configured maximum bytes.
        maximum: usize,
    },
    /// Strict JSON decoding or encoding failed.
    #[error("invalid JSON for durable state {path:?}: {reason}")]
    Json {
        /// Affected path or encoding context.
        path: String,
        /// Parser/encoder reason.
        reason: String,
    },
    /// State decoded but was not the required canonical fixed point.
    #[error("durable state {path:?} is not canonically encoded")]
    NonCanonicalState {
        /// Affected control path.
        path: String,
    },
    /// Format identifier is unsupported.
    #[error("unsupported durable format {observed:?} in {path:?}")]
    UnsupportedFormat {
        /// Affected control path.
        path: String,
        /// Observed format identifier.
        observed: String,
    },
    /// Format version is unsupported.
    #[error("unsupported durable version {observed} in {path:?}")]
    UnsupportedVersion {
        /// Affected control path.
        path: String,
        /// Observed numeric version.
        observed: u32,
    },
    /// Project head contains an invalid fixed policy or identifier.
    #[error("durable project head contains an invalid identity or fixed policy")]
    InvalidHead,
    /// Only one of the required initial head/ledger pair exists.
    #[error("durable project has an incomplete project-head/execution-ledger pair")]
    IncompleteProjectState,
    /// Ledger lacks its required final newline.
    #[error("execution ledger is truncated before a final newline")]
    TruncatedLedger,
    /// Ledger contains an invalid required value.
    #[error("malformed execution ledger: {reason}")]
    MalformedLedger {
        /// Stable rejection reason.
        reason: String,
    },
    /// Sequence, prior digest, or monotonic time does not form one append-only chain.
    #[error("invalid execution-ledger chain at sequence {sequence}")]
    InvalidLedgerChain {
        /// Affected sequence.
        sequence: u64,
    },
    /// Ledger or byte-length arithmetic overflowed.
    #[error("execution-ledger sequence or byte length overflowed")]
    LedgerSequenceOverflow,
    /// One cache key appears more than once in the append-only ledger.
    #[error("execution ledger repeats cache key {cache_key}")]
    DuplicateLedgerCacheKey {
        /// Repeated key.
        cache_key: ContentDigest,
    },
    /// Head summary does not exactly describe the validated ledger.
    #[error("durable project head does not match its execution ledger")]
    HeadLedgerMismatch,
    /// Pending intent cannot extend or equal the validated ledger.
    #[error("pending durable execution conflicts with the execution ledger")]
    ConflictingPendingExecution,
    /// A matching cache key names different execution identity fields.
    #[error("durable cache key {cache_key} conflicts with recorded execution identity")]
    ConflictingCacheIdentity {
        /// Conflicting key.
        cache_key: ContentDigest,
    },
    /// Reconstructed schema-bound artifact identity differs from the ledger claim.
    #[error("durable execution contains an invalid output artifact identity")]
    InvalidArtifactIdentity,
    /// Restored bytes do not produce the exact recorded output reference.
    #[error("restored durable output differs for cache key {cache_key}")]
    RestoredOutputMismatch {
        /// Affected key.
        cache_key: ContentDigest,
    },
    /// System clock predates the Unix epoch.
    #[error("system clock is before the Unix epoch")]
    ClockBeforeEpoch,
    /// System time cannot be represented in milliseconds.
    #[error("system clock milliseconds exceed u64")]
    ClockOverflow,
    /// Test-only crash point interrupted the durable commit.
    #[error("injected durable project failure {stage}")]
    InjectedFailure {
        /// Interrupted lifecycle stage.
        stage: &'static str,
    },
    /// Immutable artifact declaration is invalid.
    #[error(transparent)]
    ArtifactRecord(#[from] ArtifactRecordError),
    /// Existing local object-store operation failed.
    #[error(transparent)]
    ArtifactStore(#[from] ArtifactStoreError),
    /// In-memory project identity or cache commit failed.
    #[error(transparent)]
    Project(#[from] ProjectError),
    /// Digest text is malformed.
    #[error("invalid SHA-256 digest {value:?}")]
    InvalidDigest {
        /// Rejected text.
        value: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> DurableProjectLimits {
        DurableProjectLimits::new(64 * 1024, 1024 * 1024, 32, 64 * 1024, 1024).expect("limits")
    }

    fn request() -> DurableExecutionRequest {
        let executable =
            ArtifactRef::from_bytes("application/vnd.marklab.executable", b"executable")
                .expect("executable");
        let runtime = NativeRuntimeProvenance::new(
            "0.1.0",
            Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
            Some(false),
            "rustc 1.96.0",
            vec!["cli".to_owned(), "csv".to_owned()],
            executable,
        )
        .expect("runtime");
        DurableExecutionRequest::new(
            "node",
            ContentDigest::from_bytes(b"spec"),
            vec![ArtifactRef::from_bytes("application/input", b"input").expect("input")],
            ContentDigest::from_bytes(b"config"),
            ContentDigest::from_bytes(b"policy"),
            1024,
            ContentDigest::from_bytes(b"cache"),
            ArtifactSchema::new("marklab.test_result", 1).expect("schema"),
            runtime,
        )
        .expect("request")
    }

    #[test]
    fn every_postpublication_crash_phase_has_one_deterministic_recovery_outcome() {
        let cases = [
            (
                CommitFault::AfterObjectPublication,
                DurableRecoveryAction::None,
                0,
            ),
            (
                CommitFault::AfterPendingIntent,
                DurableRecoveryAction::CompletedPendingExecution,
                1,
            ),
            (
                CommitFault::AfterLedgerAppend,
                DurableRecoveryAction::AdvancedProjectHead,
                1,
            ),
            (
                CommitFault::AfterHeadReplacement,
                DurableRecoveryAction::ClearedCommittedIntent,
                1,
            ),
        ];
        for (fault, expected_action, expected_count) in cases {
            let directory = tempfile::tempdir().expect("tempdir");
            let mut project =
                DurableProject::open_or_create(directory.path(), limits()).expect("project");
            let error = project
                .commit_success_inner(&request(), "application/result", b"result", fault)
                .expect_err("injected failure");
            assert!(matches!(error, DurableProjectError::InjectedFailure { .. }));
            drop(project);

            let recovered =
                DurableProject::open_or_create(directory.path(), limits()).expect("recovered");
            assert_eq!(recovered.open_report().action(), expected_action);
            assert_eq!(recovered.execution_count(), expected_count);
            let mut in_memory =
                MarklabProject::with_inline_artifact_limit(1024).expect("in-memory project");
            let replay = recovered
                .restore_success(&request(), &mut in_memory)
                .expect("durable replay decision");
            assert_eq!(replay.is_some(), expected_count == 1);
            assert!(!directory.path().join(PENDING_PATH).exists());
        }
    }

    #[test]
    fn object_limit_rejects_before_publication_or_control_state_mutation() {
        let directory = tempfile::tempdir().expect("tempdir");
        let small_limits =
            DurableProjectLimits::new(64 * 1024, 1024 * 1024, 32, 64 * 1024, 4).expect("limits");
        let mut project =
            DurableProject::open_or_create(directory.path(), small_limits).expect("project");
        assert!(matches!(
            project.commit_success(&request(), "application/result", b"12345"),
            Err(DurableProjectError::ObjectTooLarge {
                observed: 5,
                maximum: 4
            })
        ));
        assert_eq!(project.execution_count(), 0);
        assert!(!directory.path().join(PENDING_PATH).exists());
    }

    #[test]
    fn ledger_record_limit_rejects_before_control_state_mutation() {
        let directory = tempfile::tempdir().expect("tempdir");
        let one_record_limits =
            DurableProjectLimits::new(64 * 1024, 1024 * 1024, 1, 64 * 1024, 1024).expect("limits");
        let mut project =
            DurableProject::open_or_create(directory.path(), one_record_limits).expect("project");
        project
            .commit_success(&request(), "application/result", b"first")
            .expect("first commit");

        let mut second = request();
        second.cache_key = ContentDigest::from_bytes(b"second cache key");
        assert!(matches!(
            project.commit_success(&second, "application/result", b"second"),
            Err(DurableProjectError::TooManyLedgerRecords {
                observed: 2,
                maximum: 1
            })
        ));
        assert_eq!(project.execution_count(), 1);
        assert!(!directory.path().join(PENDING_PATH).exists());
        assert_eq!(
            fs::read_to_string(directory.path().join(LEDGER_PATH))
                .expect("ledger")
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn ledger_byte_limit_rejects_before_pending_intent_publication() {
        let directory = tempfile::tempdir().expect("tempdir");
        let mut project =
            DurableProject::open_or_create(directory.path(), limits()).expect("project");
        project
            .commit_success(&request(), "application/result", b"first")
            .expect("first commit");
        let first_ledger_len = fs::metadata(directory.path().join(LEDGER_PATH))
            .expect("ledger metadata")
            .len() as usize;
        project.limits.maximum_record_bytes = first_ledger_len;
        project.limits.maximum_ledger_bytes = first_ledger_len + 1;

        let mut second = request();
        second.cache_key = ContentDigest::from_bytes(b"second cache key");
        assert!(matches!(
            project.commit_success(&second, "application/result", b"other"),
            Err(DurableProjectError::StateTooLarge { ref path, .. }) if path == LEDGER_PATH
        ));
        assert_eq!(project.execution_count(), 1);
        assert!(!directory.path().join(PENDING_PATH).exists());
        assert_eq!(
            fs::read_to_string(directory.path().join(LEDGER_PATH))
                .expect("ledger")
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn pending_recovery_respects_the_openers_record_limit_before_append() {
        let directory = tempfile::tempdir().expect("tempdir");
        let two_record_limits =
            DurableProjectLimits::new(64 * 1024, 1024 * 1024, 2, 64 * 1024, 1024).expect("limits");
        let mut project =
            DurableProject::open_or_create(directory.path(), two_record_limits).expect("project");
        project
            .commit_success(&request(), "application/result", b"first")
            .expect("first commit");
        let mut second = request();
        second.cache_key = ContentDigest::from_bytes(b"second cache key");
        assert!(matches!(
            project.commit_success_inner(
                &second,
                "application/result",
                b"other",
                CommitFault::AfterPendingIntent,
            ),
            Err(DurableProjectError::InjectedFailure { .. })
        ));
        drop(project);

        let one_record_limits =
            DurableProjectLimits::new(64 * 1024, 1024 * 1024, 1, 64 * 1024, 1024).expect("limits");
        assert!(matches!(
            DurableProject::open_or_create(directory.path(), one_record_limits),
            Err(DurableProjectError::TooManyLedgerRecords {
                observed: 2,
                maximum: 1
            })
        ));
        assert_eq!(
            fs::read_to_string(directory.path().join(LEDGER_PATH))
                .expect("ledger")
                .lines()
                .count(),
            1
        );
        assert!(directory.path().join(PENDING_PATH).exists());
    }

    #[test]
    fn pending_intent_rejects_unknown_fields_versions_and_noncanonical_bytes() {
        for case in ["unknown-field", "unsupported-version", "noncanonical"] {
            let directory = tempfile::tempdir().expect("tempdir");
            let mut project =
                DurableProject::open_or_create(directory.path(), limits()).expect("project");
            assert!(matches!(
                project.commit_success_inner(
                    &request(),
                    "application/result",
                    b"result",
                    CommitFault::AfterPendingIntent,
                ),
                Err(DurableProjectError::InjectedFailure { .. })
            ));
            drop(project);

            let path = directory.path().join(PENDING_PATH);
            let canonical = fs::read(&path).expect("pending intent");
            let mutated = match case {
                "unknown-field" => {
                    let mut bytes = canonical.clone();
                    let closing = bytes
                        .windows(3)
                        .rposition(|window| window == b"\n}\n")
                        .expect("pending closing brace");
                    bytes.splice(
                        closing..closing + 1,
                        b",\n  \"unknown\": true\n".iter().copied(),
                    );
                    bytes
                }
                "unsupported-version" => {
                    let mut bytes = canonical.clone();
                    let version = b"\"version\": 1";
                    let offset = bytes
                        .windows(version.len())
                        .position(|window| window == version)
                        .expect("pending version");
                    bytes[offset + version.len() - 1] = b'2';
                    bytes
                }
                "noncanonical" => {
                    let mut bytes = vec![b' '];
                    bytes.extend_from_slice(&canonical);
                    bytes
                }
                _ => unreachable!(),
            };
            fs::write(&path, mutated).expect("mutated pending intent");

            let error = match DurableProject::open_or_create(directory.path(), limits()) {
                Ok(_) => panic!("{case} pending intent must be rejected"),
                Err(error) => error,
            };
            match case {
                "unknown-field" => assert!(matches!(
                    error,
                    DurableProjectError::Json { ref path, ref reason }
                        if path == PENDING_PATH && reason.contains("unknown field")
                )),
                "unsupported-version" => assert!(matches!(
                    error,
                    DurableProjectError::UnsupportedVersion { ref path, observed: 2 }
                        if path == PENDING_PATH
                )),
                "noncanonical" => assert!(matches!(
                    error,
                    DurableProjectError::NonCanonicalState { ref path } if path == PENDING_PATH
                )),
                _ => unreachable!(),
            }
        }
    }
}
