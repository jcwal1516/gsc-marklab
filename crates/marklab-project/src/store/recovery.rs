use std::str::FromStr;

use cap_std::fs::Dir;

use super::{
    coordination::{sync_directory_at, CoordinationMode},
    ArtifactStoreError, LocalArtifactStore, RecoveryFault, RecoveryIssue, RecoveryIssueReason,
    RecoveryReport, QUARANTINE_DIRECTORY, STAGING_DIRECTORY,
};
use crate::ContentDigest;

impl LocalArtifactStore {
    /// Quarantine recognized abandoned regular staging files without deleting evidence.
    pub fn recover_staging(&self) -> Result<RecoveryReport, ArtifactStoreError> {
        self.recover_staging_inner(RecoveryFault::None)
    }

    fn recover_staging_inner(
        &self,
        fault: RecoveryFault,
    ) -> Result<RecoveryReport, ArtifactStoreError> {
        let _ = fault;
        let _coordination = self.acquire_coordination_lock(CoordinationMode::Exclusive)?;
        let staging =
            self.root
                .open_dir(STAGING_DIRECTORY)
                .map_err(|source| ArtifactStoreError::Io {
                    operation: "open staging directory",
                    key: STAGING_DIRECTORY.to_owned(),
                    source,
                })?;
        let quarantine =
            self.root
                .open_dir(QUARANTINE_DIRECTORY)
                .map_err(|source| ArtifactStoreError::Io {
                    operation: "open quarantine directory",
                    key: QUARANTINE_DIRECTORY.to_owned(),
                    source,
                })?;
        let mut entries = staging
            .entries()
            .map_err(|source| ArtifactStoreError::Io {
                operation: "read staging directory",
                key: STAGING_DIRECTORY.to_owned(),
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| ArtifactStoreError::Io {
                operation: "read staging entry",
                key: STAGING_DIRECTORY.to_owned(),
                source,
            })?;
        entries.sort_by_key(|entry| entry.file_name());
        let mut report = RecoveryReport::default();
        for entry in entries {
            let os_name = entry.file_name();
            let Some(name) = os_name.to_str() else {
                report.issues.push(RecoveryIssue {
                    entry: os_name.to_string_lossy().into_owned(),
                    quarantine_target: None,
                    reason: RecoveryIssueReason::NonUtf8Name,
                });
                continue;
            };
            if !recognized_staging_name(name) {
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: None,
                    reason: RecoveryIssueReason::UnrecognizedName,
                });
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(source) => {
                    report.issues.push(RecoveryIssue {
                        entry: name.to_owned(),
                        quarantine_target: None,
                        reason: RecoveryIssueReason::Io(source.to_string()),
                    });
                    continue;
                }
            };
            if !file_type.is_file() || file_type.is_symlink() {
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: None,
                    reason: RecoveryIssueReason::NonRegularFile,
                });
                continue;
            }
            let quarantine_name = available_quarantine_name(&quarantine, name)?;
            if let Err(source) = staging.hard_link(name, &quarantine, &quarantine_name) {
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: None,
                    reason: RecoveryIssueReason::Io(source.to_string()),
                });
                continue;
            }
            #[cfg(test)]
            if fault == RecoveryFault::AfterQuarantineLink {
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: Some(quarantine_name),
                    reason: RecoveryIssueReason::Io(
                        "injected failure after quarantine link".to_owned(),
                    ),
                });
                continue;
            }
            if let Err(source) = sync_directory_at(&self.root, QUARANTINE_DIRECTORY) {
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: Some(quarantine_name),
                    reason: RecoveryIssueReason::Io(source.to_string()),
                });
                continue;
            }
            #[cfg(test)]
            if fault == RecoveryFault::AfterQuarantineSync {
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: Some(quarantine_name),
                    reason: RecoveryIssueReason::Io(
                        "injected failure after quarantine sync".to_owned(),
                    ),
                });
                continue;
            }
            if let Err(source) = staging.remove_file(name) {
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: Some(quarantine_name),
                    reason: RecoveryIssueReason::Io(source.to_string()),
                });
                continue;
            }
            #[cfg(test)]
            if fault == RecoveryFault::AfterStagingRemove {
                report.quarantined.push(quarantine_name.clone());
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: Some(quarantine_name),
                    reason: RecoveryIssueReason::Io(
                        "injected failure after staging removal".to_owned(),
                    ),
                });
                continue;
            }
            if let Err(source) = sync_directory_at(&self.root, STAGING_DIRECTORY) {
                report.quarantined.push(quarantine_name.clone());
                report.issues.push(RecoveryIssue {
                    entry: name.to_owned(),
                    quarantine_target: Some(quarantine_name),
                    reason: RecoveryIssueReason::Io(source.to_string()),
                });
                continue;
            }
            report.quarantined.push(quarantine_name);
        }
        Ok(report)
    }

    #[cfg(test)]
    pub(super) fn recover_staging_with_fault(
        &self,
        fault: RecoveryFault,
    ) -> Result<RecoveryReport, ArtifactStoreError> {
        self.recover_staging_inner(fault)
    }
}

fn available_quarantine_name(directory: &Dir, base: &str) -> Result<String, ArtifactStoreError> {
    for suffix in 0_u64..=u64::MAX {
        let candidate = if suffix == 0 {
            base.to_owned()
        } else {
            format!("{base}.recovered-{suffix}")
        };
        match directory.symlink_metadata(&candidate) {
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => {}
            Err(source) => {
                return Err(ArtifactStoreError::Io {
                    operation: "inspect quarantine candidate",
                    key: candidate,
                    source,
                });
            }
        }
    }
    unreachable!("u64 quarantine suffix space cannot be exhausted in practice")
}

fn recognized_staging_name(name: &str) -> bool {
    let Some(body) = name
        .strip_prefix("marklab-")
        .and_then(|value| value.strip_suffix(".part"))
    else {
        return false;
    };
    let mut parts = body.split('-');
    let (Some(process), Some(sequence), Some(digest), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    !process.is_empty()
        && process.bytes().all(|byte| byte.is_ascii_digit())
        && !sequence.is_empty()
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
        && ContentDigest::from_str(digest).is_ok()
}
