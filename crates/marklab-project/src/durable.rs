use std::{fs, path::Path};

use cap_std::{ambient_authority, fs::Dir};

use crate::{LocalArtifactStore, MarklabProject, StoreId};

mod api;
mod error;
mod execution;
mod layout;
mod recovery;
mod state;
mod wire;

pub use api::{
    DurableCommitDisposition, DurableExecutionRequest, DurableOpenReport, DurableProjectLimits,
    DurableRecoveryAction, DurableReplay, NativeRuntimeProvenance,
};
pub use error::DurableProjectError;

use execution::CommitFault;
use layout::{ensure_directory, ensure_lock_file, ensure_project_root, open_and_lock};
use recovery::recover_pending;
use state::{ledger_byte_len, open_or_initialize_state, validate_head_against_ledger};
use wire::{ExecutionWire, ProjectHeadWire, STORE_DIRECTORY, STORE_ID};
#[cfg(test)]
use wire::{LEDGER_PATH, PENDING_PATH};

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
        execution::restore_success(self, request, project)
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
        execution::commit_success_inner(self, request, output_kind, encoded_output, fault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArtifactRef, ArtifactSchema, ContentDigest};

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
