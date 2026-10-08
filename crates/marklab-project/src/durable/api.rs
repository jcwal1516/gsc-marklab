use crate::{ArtifactRef, ArtifactSchema, ContentDigest, RecoveryReport};

use super::{
    wire::{
        canonical_compact, valid_token, ArtifactWire, ExecutionIdentityWire, GitWire,
        NativeRuntimeWire, NodeWire, SchemaWire, MAX_EXECUTION_INPUTS,
    },
    DurableProjectError,
};

const MAX_FEATURES: usize = 64;
const MAX_TEXT_BYTES: usize = 1_024;

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

    pub(super) fn validate(self) -> Result<(), DurableProjectError> {
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
    pub(super) wire: NativeRuntimeWire,
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

    pub(super) fn from_wire(wire: NativeRuntimeWire) -> Result<Self, DurableProjectError> {
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
    pub(super) node_id: String,
    pub(super) node_spec_digest: ContentDigest,
    pub(super) inputs: Vec<ArtifactRef>,
    pub(super) configuration_digest: ContentDigest,
    pub(super) execution_policy_digest: ContentDigest,
    pub(super) scheduler_output_limit_bytes: u64,
    pub(super) cache_key: ContentDigest,
    pub(super) result_schema: ArtifactSchema,
    pub(super) runtime: NativeRuntimeProvenance,
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

    pub(super) fn identity_wire(&self) -> ExecutionIdentityWire {
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
    pub(super) action: DurableRecoveryAction,
    pub(super) artifact_store: RecoveryReport,
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
    pub(super) sequence: u64,
    pub(super) output: ArtifactRef,
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

fn valid_git_sha(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
