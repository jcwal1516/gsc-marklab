use std::{
    error::Error as StdError,
    fmt, fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(windows)]
use cap_std::fs::OpenOptionsExt;
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use thiserror::Error;

use crate::{
    ArtifactId, ArtifactLocator, ArtifactRecord, ArtifactRecordError, ContentDigest,
    ContentDigestWriter, ProjectError, StoreId,
};

const STAGING_DIRECTORY: &str = ".marklab-staging";
const QUARANTINE_DIRECTORY: &str = ".marklab-quarantine";
const OBJECT_DIRECTORY: &str = "objects/sha256";
const COORDINATION_FILE: &str = ".marklab-store.lock";
const STREAM_BUFFER_BYTES: usize = 64 * 1024;
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Scoped reader surface for one verified artifact descriptor.
pub trait ArtifactReadSeek: Read + Seek + Send {}

impl<T: Read + Seek + Send> ArtifactReadSeek for T {}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PublishFault {
    None,
    #[cfg(test)]
    AfterFileSync,
    #[cfg(test)]
    AfterPublication,
    #[cfg(test)]
    StagingCreateCollision,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum RecoveryFault {
    None,
    #[cfg(test)]
    AfterQuarantineLink,
    #[cfg(test)]
    AfterQuarantineSync,
    #[cfg(test)]
    AfterStagingRemove,
}

/// Capability-confined local binding for immutable artifact objects.
pub struct LocalArtifactStore {
    root: Dir,
    store_id: StoreId,
}

impl LocalArtifactStore {
    /// Open an existing non-symlink directory as the single ambient-authority boundary.
    pub fn open(root: &Path, store_id: StoreId) -> Result<Self, ArtifactStoreError> {
        let metadata = fs::symlink_metadata(root).map_err(|source| ArtifactStoreError::Root {
            operation: "inspect",
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ArtifactStoreError::SymlinkBoundary {
                key: "<store-root>".to_owned(),
            });
        }
        if !metadata.is_dir() {
            return Err(ArtifactStoreError::UnsupportedFileType {
                key: "<store-root>".to_owned(),
            });
        }
        let root = Dir::open_ambient_dir(root, ambient_authority()).map_err(|source| {
            ArtifactStoreError::Root {
                operation: "open",
                source,
            }
        })?;
        let store = Self { root, store_id };
        store.ensure_directory(STAGING_DIRECTORY)?;
        store.ensure_directory(QUARANTINE_DIRECTORY)?;
        store.ensure_directory(OBJECT_DIRECTORY)?;
        store.ensure_coordination_file()?;
        Ok(store)
    }

    /// Logical binding name associated with this capability root.
    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    /// Stream, verify, durably publish, and return a record with the managed locator.
    ///
    /// Existing verified content is returned idempotently without invoking `write`.
    pub fn publish<F>(
        &self,
        record: &ArtifactRecord,
        write: F,
    ) -> Result<ArtifactPublication, ArtifactStoreError>
    where
        F: FnOnce(&mut dyn Write) -> io::Result<()>,
    {
        self.publish_inner(record, |writer| write(writer), PublishFault::None)
    }

    /// Stream through a send-capable writer while preserving publication semantics.
    ///
    /// Existing verified content is returned idempotently without invoking `write`.
    pub fn publish_send<F>(
        &self,
        record: &ArtifactRecord,
        write: F,
    ) -> Result<ArtifactPublication, ArtifactStoreError>
    where
        F: FnOnce(&mut (dyn Write + Send)) -> io::Result<()>,
    {
        self.publish_inner(record, |writer| write(writer), PublishFault::None)
    }

    fn publish_inner<F>(
        &self,
        record: &ArtifactRecord,
        write: F,
        fault: PublishFault,
    ) -> Result<ArtifactPublication, ArtifactStoreError>
    where
        F: FnOnce(&mut DigestingWriter) -> io::Result<()>,
    {
        let _ = fault;
        let _coordination = self.acquire_coordination_lock(CoordinationMode::Shared)?;
        let managed = ArtifactLocator::managed(self.store_id.clone(), record.id());
        let mut located_record = record.clone();
        located_record.merge_locations(std::slice::from_ref(&managed))?;
        match self.verify_locator(record, &managed) {
            Ok(()) => {
                return Ok(Self::publication(
                    &located_record,
                    PublicationDisposition::AlreadyPresent,
                ));
            }
            Err(ArtifactStoreError::MissingObject { .. }) => {}
            Err(ArtifactStoreError::ContentIntegrity { .. })
            | Err(ArtifactStoreError::UnsupportedFileType { .. }) => {
                return Err(ArtifactStoreError::ImmutableConflict {
                    artifact: record.id(),
                });
            }
            Err(error) => return Err(error),
        }

        let final_key = managed.key().as_str();
        let (final_parent, final_leaf) = split_parent(final_key);
        self.ensure_directory(final_parent)?;
        let destination_dir =
            self.root
                .open_dir(final_parent)
                .map_err(|source| ArtifactStoreError::Io {
                    operation: "open destination directory",
                    key: final_parent.to_owned(),
                    source,
                })?;
        let staging_key = self.create_staging_key(record.id())?;
        #[cfg(test)]
        if fault == PublishFault::StagingCreateCollision {
            self.root
                .write(&staging_key, b"owned by another writer")
                .map_err(|source| ArtifactStoreError::Io {
                    operation: "inject staging collision",
                    key: staging_key.clone(),
                    source,
                })?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let file = self
            .root
            .open_with(&staging_key, &options)
            .map_err(|source| ArtifactStoreError::Io {
                operation: "create staging file",
                key: staging_key.clone(),
                source,
            })?;
        let mut guard = StagingGuard::new(&self.root, staging_key.clone());
        let mut writer = DigestingWriter::new(file, record.content().byte_len());
        if let Err(source) = write(&mut writer) {
            drop(writer);
            guard.cleanup("callback cleanup")?;
            return Err(ArtifactStoreError::WriteCallback { source });
        }
        if let Err(source) = writer.flush() {
            drop(writer);
            guard.cleanup("flush-failure cleanup")?;
            return Err(ArtifactStoreError::Io {
                operation: "flush staging file",
                key: staging_key,
                source,
            });
        }
        if let Err(source) = writer.sync_all() {
            drop(writer);
            guard.cleanup("sync-failure cleanup")?;
            return Err(ArtifactStoreError::Io {
                operation: "sync staging file",
                key: staging_key,
                source,
            });
        }
        #[cfg(test)]
        if fault == PublishFault::AfterFileSync {
            drop(writer);
            guard.cleanup("injected prepublication cleanup")?;
            return Err(ArtifactStoreError::Io {
                operation: "injected after file sync",
                key: staging_key,
                source: io::Error::other("injected failure after staging file sync"),
            });
        }
        let (digest, byte_len) = writer.finish();
        if let Err(source) = record.content().verify_identity(digest, byte_len) {
            guard.cleanup("integrity cleanup")?;
            return Err(ArtifactStoreError::ContentIntegrity {
                artifact: record.id(),
                source: Box::new(source),
            });
        }

        match self
            .root
            .hard_link(&staging_key, &destination_dir, final_leaf)
        {
            Ok(()) => {
                if let Err(source) = sync_directory_at(&self.root, final_parent) {
                    let _ = guard.cleanup("destination-sync failure cleanup");
                    return Err(ArtifactStoreError::PublishedButCleanupFailed {
                        artifact: record.id(),
                        operation: "sync destination directory",
                        source,
                    });
                }
                #[cfg(test)]
                if fault == PublishFault::AfterPublication {
                    guard.leave_for_recovery();
                    return Err(ArtifactStoreError::PublishedButCleanupFailed {
                        artifact: record.id(),
                        operation: "injected after publication",
                        source: io::Error::other(
                            "injected failure after durable destination publication",
                        ),
                    });
                }
                guard
                    .cleanup("published staging cleanup")
                    .map_err(|error| match error {
                        ArtifactStoreError::Io { source, .. } => {
                            ArtifactStoreError::PublishedButCleanupFailed {
                                artifact: record.id(),
                                operation: "remove or sync staging file",
                                source,
                            }
                        }
                        other => other,
                    })?;
                Ok(Self::publication(
                    &located_record,
                    PublicationDisposition::Created,
                ))
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                match self.verify_locator(record, &managed) {
                    Ok(()) => {
                        if let Err(source) = sync_directory_at(&self.root, final_parent) {
                            let _ = guard.cleanup("concurrent-sync failure cleanup");
                            return Err(ArtifactStoreError::PublishedButCleanupFailed {
                                artifact: record.id(),
                                operation: "sync concurrently published directory",
                                source,
                            });
                        }
                        guard.cleanup("concurrent staging cleanup")?;
                        Ok(Self::publication(
                            &located_record,
                            PublicationDisposition::AlreadyPresent,
                        ))
                    }
                    Err(ArtifactStoreError::ContentIntegrity { .. })
                    | Err(ArtifactStoreError::UnsupportedFileType { .. }) => {
                        guard.cleanup("conflict staging cleanup")?;
                        Err(ArtifactStoreError::ImmutableConflict {
                            artifact: record.id(),
                        })
                    }
                    Err(error) => {
                        guard.cleanup("concurrent-error staging cleanup")?;
                        Err(error)
                    }
                }
            }
            Err(source) => {
                guard.cleanup("publication-failure staging cleanup")?;
                Err(ArtifactStoreError::Io {
                    operation: "publish hard link",
                    key: final_key.to_owned(),
                    source,
                })
            }
        }
    }

    #[cfg(test)]
    fn publish_with_fault<F>(
        &self,
        record: &ArtifactRecord,
        write: F,
        fault: PublishFault,
    ) -> Result<ArtifactPublication, ArtifactStoreError>
    where
        F: FnOnce(&mut dyn Write) -> io::Result<()>,
    {
        self.publish_inner(record, |writer| write(writer), fault)
    }

    /// Stream and verify the record replica bound to this store.
    ///
    /// A catalog declaration alone is insufficient; successful verification
    /// is the availability boundary used by workflows.
    pub fn verify(&self, record: &ArtifactRecord) -> Result<(), ArtifactStoreError> {
        let locator = record
            .locations()
            .iter()
            .find(|locator| locator.store_id() == &self.store_id)
            .ok_or_else(|| ArtifactStoreError::LocatorNotFound {
                artifact: record.id(),
                store_id: self.store_id.clone(),
            })?;
        self.verify_locator(record, locator)
    }

    /// Verify, lend, and reverify one managed artifact descriptor under the store lock.
    ///
    /// The callback receives no path, capability directory, locator key, or owned
    /// handle. Post-read store failures take precedence over the callback result.
    pub fn with_verified_reader<T, E, F>(
        &self,
        record: &ArtifactRecord,
        read: F,
    ) -> Result<T, VerifiedReaderError<E>>
    where
        F: FnOnce(&mut dyn ArtifactReadSeek) -> Result<T, E>,
    {
        let _coordination = self
            .acquire_coordination_lock(CoordinationMode::Shared)
            .map_err(VerifiedReaderError::Store)?;
        let managed = ArtifactLocator::managed(self.store_id.clone(), record.id());
        if !record.locations().contains(&managed) {
            return Err(VerifiedReaderError::Store(
                ArtifactStoreError::LocatorNotFound {
                    artifact: record.id(),
                    store_id: self.store_id.clone(),
                },
            ));
        }
        let key = managed.key().as_str();
        let mut file = self
            .open_locator(record, &managed)
            .map_err(VerifiedReaderError::Store)?;
        self.verify_open_file(record, key, &mut file)
            .map_err(VerifiedReaderError::Store)?;
        file.seek(SeekFrom::Start(0)).map_err(|source| {
            VerifiedReaderError::Store(ArtifactStoreError::Io {
                operation: "seek verified artifact",
                key: key.to_owned(),
                source,
            })
        })?;

        let callback = read(&mut file);
        let post_read = self.verify_open_file(record, key, &mut file);
        match post_read {
            Err(error) => Err(VerifiedReaderError::Store(error)),
            Ok(()) => callback.map_err(VerifiedReaderError::Callback),
        }
    }

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
    fn recover_staging_with_fault(
        &self,
        fault: RecoveryFault,
    ) -> Result<RecoveryReport, ArtifactStoreError> {
        self.recover_staging_inner(fault)
    }

    fn publication(
        located_record: &ArtifactRecord,
        disposition: PublicationDisposition,
    ) -> ArtifactPublication {
        ArtifactPublication {
            record: located_record.clone(),
            disposition,
        }
    }

    fn verify_locator(
        &self,
        record: &ArtifactRecord,
        locator: &ArtifactLocator,
    ) -> Result<(), ArtifactStoreError> {
        let key = locator.key().as_str();
        let mut file = self.open_locator(record, locator)?;
        self.verify_open_file(record, key, &mut file)
    }

    fn open_locator(
        &self,
        record: &ArtifactRecord,
        locator: &ArtifactLocator,
    ) -> Result<cap_std::fs::File, ArtifactStoreError> {
        if locator.store_id() != &self.store_id {
            return Err(ArtifactStoreError::WrongStore {
                expected: self.store_id.clone(),
                observed: locator.store_id().clone(),
            });
        }
        let key = locator.key().as_str();
        self.inspect_regular_file(record.id(), key)?;
        let file = self
            .root
            .open(key)
            .map_err(|source| map_open_error(record.id(), key, source))?;
        if !file
            .metadata()
            .map_err(|source| ArtifactStoreError::Io {
                operation: "inspect opened artifact",
                key: key.to_owned(),
                source,
            })?
            .is_file()
        {
            return Err(ArtifactStoreError::UnsupportedFileType {
                key: key.to_owned(),
            });
        }
        Ok(file)
    }

    fn verify_open_file(
        &self,
        record: &ArtifactRecord,
        key: &str,
        file: &mut cap_std::fs::File,
    ) -> Result<(), ArtifactStoreError> {
        file.seek(SeekFrom::Start(0))
            .map_err(|source| ArtifactStoreError::Io {
                operation: "seek artifact",
                key: key.to_owned(),
                source,
            })?;
        let mut digest = ContentDigest::builder();
        let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|source| ArtifactStoreError::Io {
                    operation: "read artifact",
                    key: key.to_owned(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            digest
                .write_all(&buffer[..read])
                .map_err(|source| ArtifactStoreError::Io {
                    operation: "hash artifact",
                    key: key.to_owned(),
                    source,
                })?;
        }
        let (observed_digest, observed_len) = digest.finish();
        record
            .content()
            .verify_identity(observed_digest, observed_len)
            .map_err(|source| ArtifactStoreError::ContentIntegrity {
                artifact: record.id(),
                source: Box::new(source),
            })
    }

    fn inspect_regular_file(
        &self,
        artifact: ArtifactId,
        key: &str,
    ) -> Result<(), ArtifactStoreError> {
        let components = key.split('/').collect::<Vec<_>>();
        let mut prefix = PathBuf::new();
        for (index, component) in components.iter().enumerate() {
            prefix.push(component);
            let prefix_text = prefix.to_string_lossy().into_owned();
            let metadata = self.root.symlink_metadata(&prefix).map_err(|source| {
                map_open_error_for_key(artifact, key, prefix_text.clone(), source)
            })?;
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                return Err(ArtifactStoreError::SymlinkBoundary { key: prefix_text });
            }
            let final_component = index + 1 == components.len();
            if (!final_component && !metadata.is_dir()) || (final_component && !metadata.is_file())
            {
                return Err(ArtifactStoreError::UnsupportedFileType { key: prefix_text });
            }
        }
        Ok(())
    }

    fn ensure_directory(&self, path: &str) -> Result<(), ArtifactStoreError> {
        let mut prefix = PathBuf::new();
        for component in path.split('/') {
            prefix.push(component);
            match self.root.symlink_metadata(&prefix) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(ArtifactStoreError::SymlinkBoundary {
                        key: prefix.to_string_lossy().into_owned(),
                    });
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(ArtifactStoreError::UnsupportedFileType {
                        key: prefix.to_string_lossy().into_owned(),
                    });
                }
                Ok(_) => {}
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    let created = match self.root.create_dir(&prefix) {
                        Ok(()) => true,
                        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => false,
                        Err(source) => {
                            return Err(ArtifactStoreError::Io {
                                operation: "create directory",
                                key: prefix.to_string_lossy().into_owned(),
                                source,
                            });
                        }
                    };
                    let metadata = self.root.symlink_metadata(&prefix).map_err(|source| {
                        ArtifactStoreError::Io {
                            operation: "inspect created directory",
                            key: prefix.to_string_lossy().into_owned(),
                            source,
                        }
                    })?;
                    if metadata.file_type().is_symlink() {
                        return Err(ArtifactStoreError::SymlinkBoundary {
                            key: prefix.to_string_lossy().into_owned(),
                        });
                    }
                    if !metadata.is_dir() {
                        return Err(ArtifactStoreError::UnsupportedFileType {
                            key: prefix.to_string_lossy().into_owned(),
                        });
                    }
                    if created {
                        sync_parent_directory(&self.root, &prefix).map_err(|source| {
                            ArtifactStoreError::Io {
                                operation: "sync created-directory parent",
                                key: prefix.to_string_lossy().into_owned(),
                                source,
                            }
                        })?;
                    }
                }
                Err(source) => {
                    return Err(ArtifactStoreError::Io {
                        operation: "inspect directory",
                        key: prefix.to_string_lossy().into_owned(),
                        source,
                    });
                }
            }
        }
        Ok(())
    }

    fn create_staging_key(&self, id: ArtifactId) -> Result<String, ArtifactStoreError> {
        for _ in 0..1_024 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = format!("marklab-{}-{sequence}-{id}.part", std::process::id());
            let key = format!("{STAGING_DIRECTORY}/{name}");
            match self.root.symlink_metadata(&key) {
                Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(key),
                Ok(_) => continue,
                Err(source) => {
                    return Err(ArtifactStoreError::Io {
                        operation: "inspect staging candidate",
                        key,
                        source,
                    });
                }
            }
        }
        Err(ArtifactStoreError::StagingNameExhausted)
    }

    fn ensure_coordination_file(&self) -> Result<(), ArtifactStoreError> {
        match self.root.symlink_metadata(COORDINATION_FILE) {
            Ok(metadata) => return validate_coordination_metadata(metadata),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ArtifactStoreError::Io {
                    operation: "inspect coordination file",
                    key: COORDINATION_FILE.to_owned(),
                    source,
                });
            }
        }

        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        match self.root.open_with(COORDINATION_FILE, &options) {
            Ok(file) => {
                file.sync_all().map_err(|source| ArtifactStoreError::Io {
                    operation: "sync coordination file",
                    key: COORDINATION_FILE.to_owned(),
                    source,
                })?;
                sync_directory_at(&self.root, Path::new("")).map_err(|source| {
                    ArtifactStoreError::Io {
                        operation: "sync coordination-file parent",
                        key: COORDINATION_FILE.to_owned(),
                        source,
                    }
                })?;
                Ok(())
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => self
                .root
                .symlink_metadata(COORDINATION_FILE)
                .map_err(|source| ArtifactStoreError::Io {
                    operation: "inspect raced coordination file",
                    key: COORDINATION_FILE.to_owned(),
                    source,
                })
                .and_then(validate_coordination_metadata),
            Err(source) => Err(ArtifactStoreError::Io {
                operation: "create coordination file",
                key: COORDINATION_FILE.to_owned(),
                source,
            }),
        }
    }

    fn open_coordination_file(&self) -> Result<fs::File, ArtifactStoreError> {
        let metadata = self
            .root
            .symlink_metadata(COORDINATION_FILE)
            .map_err(|source| ArtifactStoreError::Io {
                operation: "inspect coordination file",
                key: COORDINATION_FILE.to_owned(),
                source,
            })?;
        validate_coordination_metadata(metadata)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        let file = self
            .root
            .open_with(COORDINATION_FILE, &options)
            .map_err(|source| ArtifactStoreError::Io {
                operation: "open coordination file",
                key: COORDINATION_FILE.to_owned(),
                source,
            })?;
        if !file
            .metadata()
            .map_err(|source| ArtifactStoreError::Io {
                operation: "inspect opened coordination file",
                key: COORDINATION_FILE.to_owned(),
                source,
            })?
            .is_file()
        {
            return Err(ArtifactStoreError::UnsupportedFileType {
                key: COORDINATION_FILE.to_owned(),
            });
        }
        Ok(file.into_std())
    }

    fn acquire_coordination_lock(
        &self,
        mode: CoordinationMode,
    ) -> Result<CoordinationGuard, ArtifactStoreError> {
        let file = self.open_coordination_file()?;
        let result = match mode {
            CoordinationMode::Shared => file.lock_shared(),
            CoordinationMode::Exclusive => file.lock(),
        };
        result.map_err(|source| ArtifactStoreError::Io {
            operation: "lock store coordination file",
            key: COORDINATION_FILE.to_owned(),
            source,
        })?;
        Ok(CoordinationGuard { _file: file })
    }
}

fn validate_coordination_metadata(
    metadata: cap_std::fs::Metadata,
) -> Result<(), ArtifactStoreError> {
    if metadata.file_type().is_symlink() {
        Err(ArtifactStoreError::SymlinkBoundary {
            key: COORDINATION_FILE.to_owned(),
        })
    } else if !metadata.is_file() {
        Err(ArtifactStoreError::UnsupportedFileType {
            key: COORDINATION_FILE.to_owned(),
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum CoordinationMode {
    Shared,
    Exclusive,
}

struct CoordinationGuard {
    _file: fs::File,
}

fn map_open_error(artifact: ArtifactId, key: &str, source: io::Error) -> ArtifactStoreError {
    if source.kind() == io::ErrorKind::NotFound {
        ArtifactStoreError::MissingObject {
            artifact,
            key: key.to_owned(),
        }
    } else {
        ArtifactStoreError::Io {
            operation: "open artifact",
            key: key.to_owned(),
            source,
        }
    }
}

fn map_open_error_for_key(
    artifact: ArtifactId,
    key: &str,
    inspected: String,
    source: io::Error,
) -> ArtifactStoreError {
    if source.kind() == io::ErrorKind::NotFound {
        ArtifactStoreError::MissingObject {
            artifact,
            key: key.to_owned(),
        }
    } else {
        ArtifactStoreError::Io {
            operation: "inspect artifact path",
            key: inspected,
            source,
        }
    }
}

fn split_parent(key: &str) -> (&str, &str) {
    key.rsplit_once('/')
        .expect("managed object key always contains a parent")
}

fn sync_parent_directory(root: &Dir, path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    sync_directory_at(root, parent)
}

#[cfg(not(windows))]
fn sync_directory_at(root: &Dir, path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    let directory = if path.as_os_str().is_empty() {
        root.try_clone()?
    } else {
        root.open_dir(path)?
    };
    directory.into_std_file().sync_all()
}

#[cfg(windows)]
fn sync_directory_at(root: &Dir, path: impl AsRef<Path>) -> io::Result<()> {
    // Windows' FlushFileBuffers requires GENERIC_WRITE. cap-std opens `Dir`
    // capabilities read-only, so obtain a capability-relative writable
    // directory handle solely for the durability barrier.
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let path = path.as_ref();
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    };
    let mut options = OpenOptions::new();
    options.write(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    root.open_with(path, &options)?.sync_all()
}

fn available_quarantine_name(directory: &Dir, base: &str) -> Result<String, ArtifactStoreError> {
    for suffix in 0_u64..=u64::MAX {
        let candidate = if suffix == 0 {
            base.to_owned()
        } else {
            format!("{base}.recovered-{suffix}")
        };
        match directory.symlink_metadata(&candidate) {
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(candidate),
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

struct DigestingWriter {
    file: cap_std::fs::File,
    digest: ContentDigestWriter,
    expected_len: u64,
    written: u64,
}

impl DigestingWriter {
    fn new(file: cap_std::fs::File, expected_len: u64) -> Self {
        Self {
            file,
            digest: ContentDigest::builder(),
            expected_len,
            written: 0,
        }
    }

    fn sync_all(&self) -> io::Result<()> {
        self.file.sync_all()
    }

    fn finish(self) -> (ContentDigest, u64) {
        self.digest.finish()
    }
}

impl Write for DigestingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let requested = u64::try_from(buffer.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "write length exceeds u64"))?;
        if self
            .written
            .checked_add(requested)
            .is_none_or(|total| total > self.expected_len)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "artifact writer exceeded declared byte length",
            ));
        }
        let written = self.file.write(buffer)?;
        self.digest.write_all(&buffer[..written])?;
        let written_u64 =
            u64::try_from(written).expect("accepted write length was already representable as u64");
        self.written = self
            .written
            .checked_add(written_u64)
            .expect("accepted write length cannot overflow");
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

struct StagingGuard<'a> {
    root: &'a Dir,
    key: String,
    armed: bool,
}

impl<'a> StagingGuard<'a> {
    fn new(root: &'a Dir, key: String) -> Self {
        Self {
            root,
            key,
            armed: true,
        }
    }

    fn cleanup(&mut self, operation: &'static str) -> Result<(), ArtifactStoreError> {
        if !self.armed {
            return Ok(());
        }
        self.root
            .remove_file(&self.key)
            .map_err(|source| ArtifactStoreError::Io {
                operation,
                key: self.key.clone(),
                source,
            })?;
        self.armed = false;
        sync_directory_at(self.root, STAGING_DIRECTORY).map_err(|source| {
            ArtifactStoreError::Io {
                operation: "sync staging directory",
                key: STAGING_DIRECTORY.to_owned(),
                source,
            }
        })?;
        Ok(())
    }

    #[cfg(test)]
    fn leave_for_recovery(&mut self) {
        self.armed = false;
    }
}

impl Drop for StagingGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.root.remove_file(&self.key);
        }
    }
}

/// Outcome of an immutable managed publication attempt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PublicationDisposition {
    /// This caller atomically created the final hard link.
    Created,
    /// Matching content was already present or won a concurrent race.
    AlreadyPresent,
}

/// Published record with its managed local replica declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactPublication {
    record: ArtifactRecord,
    disposition: PublicationDisposition,
}

impl ArtifactPublication {
    /// Record augmented with the managed locator for this store.
    pub fn record(&self) -> &ArtifactRecord {
        &self.record
    }

    /// Whether this caller created or reused the immutable object.
    pub fn disposition(&self) -> PublicationDisposition {
        self.disposition
    }

    /// Consume the publication and return the augmented record.
    pub fn into_record(self) -> ArtifactRecord {
        self.record
    }
}

/// Report from conservative staging recovery.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecoveryReport {
    quarantined: Vec<String>,
    issues: Vec<RecoveryIssue>,
}

impl RecoveryReport {
    /// Recognized regular entries moved to quarantine, in deterministic order.
    pub fn quarantined(&self) -> &[String] {
        &self.quarantined
    }

    /// Entries retained or partially handled for explicit operator review.
    pub fn issues(&self) -> &[RecoveryIssue] {
        &self.issues
    }
}

/// One staging entry that recovery deliberately did not silently delete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryIssue {
    entry: String,
    quarantine_target: Option<String>,
    reason: RecoveryIssueReason,
}

impl RecoveryIssue {
    /// Entry name, lossily rendered only when the filesystem name is not UTF-8.
    pub fn entry(&self) -> &str {
        &self.entry
    }

    /// Quarantine target already created before a partial recovery failure.
    pub fn quarantine_target(&self) -> Option<&str> {
        self.quarantine_target.as_deref()
    }

    /// Why the entry requires operator attention.
    pub fn reason(&self) -> &RecoveryIssueReason {
        &self.reason
    }
}

/// Conservative reason an abandoned entry was retained or needs review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryIssueReason {
    /// Filesystem entry name is not UTF-8.
    NonUtf8Name,
    /// Entry name was not generated by this store implementation.
    UnrecognizedName,
    /// Entry is a symlink, directory, FIFO, socket, device, or other non-regular type.
    NonRegularFile,
    /// A capability-relative filesystem operation failed.
    Io(String),
}

/// Failure from the verified store boundary or its scoped reader callback.
#[derive(Debug)]
pub enum VerifiedReaderError<E> {
    /// Opening, pre-verifying, seeking, or post-verifying the managed artifact failed.
    Store(ArtifactStoreError),
    /// The caller rejected otherwise verified artifact bytes.
    Callback(E),
}

impl<E: fmt::Display> fmt::Display for VerifiedReaderError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(formatter),
            Self::Callback(error) => {
                write!(formatter, "verified artifact callback failed: {error}")
            }
        }
    }
}

impl<E: StdError + 'static> StdError for VerifiedReaderError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Callback(error) => Some(error),
        }
    }
}

/// Capability, availability, integrity, publication, and recovery failures.
#[derive(Debug, Error)]
pub enum ArtifactStoreError {
    /// Ambient store-root inspection/opening failed.
    #[error("failed to {operation} local artifact store root: {source}")]
    Root {
        /// Failed operation.
        operation: &'static str,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// An observed root, intermediate component, or leaf is a symlink.
    #[error("artifact path crosses a symlink boundary at {key:?}")]
    SymlinkBoundary {
        /// Store-relative component or root marker.
        key: String,
    },
    /// An object expected to be a directory/regular file has another type.
    #[error("artifact path has an unsupported file type at {key:?}")]
    UnsupportedFileType {
        /// Store-relative component or root marker.
        key: String,
    },
    /// A record has no replica bound to this store.
    #[error("artifact {artifact} has no locator for store {store_id:?}")]
    LocatorNotFound {
        /// Affected artifact.
        artifact: ArtifactId,
        /// Required store.
        store_id: StoreId,
    },
    /// Explicit locator is bound to another store.
    #[error("artifact locator uses store {observed:?}, expected {expected:?}")]
    WrongStore {
        /// Required store.
        expected: StoreId,
        /// Locator store.
        observed: StoreId,
    },
    /// Referenced regular object is absent.
    #[error("artifact {artifact} is missing at store key {key:?}")]
    MissingObject {
        /// Affected artifact.
        artifact: ArtifactId,
        /// Missing relative key.
        key: String,
    },
    /// Streamed bytes differ in digest or length.
    #[error("artifact {artifact} failed content verification: {source}")]
    ContentIntegrity {
        /// Affected artifact.
        artifact: ArtifactId,
        /// Exact digest/length mismatch.
        #[source]
        source: Box<ProjectError>,
    },
    /// Existing managed target is not the expected immutable content.
    #[error("managed target for artifact {artifact} contains conflicting content")]
    ImmutableConflict {
        /// Conflicting artifact target.
        artifact: ArtifactId,
    },
    /// Caller-supplied streaming writer failed before publication.
    #[error("artifact write callback failed: {source}")]
    WriteCallback {
        /// Callback I/O error.
        #[source]
        source: io::Error,
    },
    /// Final object was published, but a subsequent durability/cleanup step failed.
    #[error("artifact {artifact} was published but failed to {operation}: {source}")]
    PublishedButCleanupFailed {
        /// Published artifact.
        artifact: ArtifactId,
        /// Failed post-publication operation.
        operation: &'static str,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// Capability-relative filesystem operation failed.
    #[error("failed to {operation} at store key {key:?}: {source}")]
    Io {
        /// Failed operation.
        operation: &'static str,
        /// Store-relative key.
        key: String,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// Managed record cannot add this store locator without conflict.
    #[error(transparent)]
    InvalidRecord(#[from] ArtifactRecordError),
    /// Unique staging names repeatedly collided.
    #[error("could not allocate a unique artifact staging name")]
    StagingNameExhausted,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::TempDir;

    use super::*;
    use crate::{ArtifactKey, ArtifactRef, ArtifactSchema};

    fn record(bytes: &[u8]) -> ArtifactRecord {
        ArtifactRecord::new(
            ArtifactRef::from_bytes("application/vnd.marklab.fault-test", bytes).expect("content"),
            ArtifactSchema::new("marklab.test.fault", 1).expect("schema"),
            None,
            Vec::new(),
            BTreeMap::new(),
            vec![ArtifactLocator::new(
                StoreId::new("source").expect("source store"),
                ArtifactKey::new("incoming/fault-test").expect("source key"),
                None,
            )
            .expect("source locator")],
        )
        .expect("record")
    }

    fn final_path(root: &TempDir, record: &ArtifactRecord) -> PathBuf {
        let id = record.id().to_string();
        root.path().join("objects/sha256").join(&id[..2]).join(id)
    }

    #[test]
    fn failure_after_file_sync_removes_staging_without_publishing() {
        let root = TempDir::new().expect("root");
        let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
            .expect("store");
        let bytes = b"prepublication fault";
        let record = record(bytes);
        assert!(matches!(
            store.publish_with_fault(
                &record,
                |writer| writer.write_all(bytes),
                PublishFault::AfterFileSync,
            ),
            Err(ArtifactStoreError::Io {
                operation: "injected after file sync",
                ..
            })
        ));
        assert!(!final_path(&root, &record).exists());
        assert_eq!(
            fs::read_dir(root.path().join(STAGING_DIRECTORY))
                .expect("staging")
                .count(),
            0
        );
    }

    #[test]
    fn create_new_collision_never_deletes_a_staging_file_owned_by_another_writer() {
        let root = TempDir::new().expect("root");
        let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
            .expect("store");
        let bytes = b"collision content";
        let record = record(bytes);
        assert!(matches!(
            store.publish_with_fault(
                &record,
                |writer| writer.write_all(bytes),
                PublishFault::StagingCreateCollision,
            ),
            Err(ArtifactStoreError::Io {
                operation: "create staging file",
                ..
            })
        ));
        assert!(!final_path(&root, &record).exists());
        let staging_entries = fs::read_dir(root.path().join(STAGING_DIRECTORY))
            .expect("staging")
            .collect::<Result<Vec<_>, _>>()
            .expect("staging entries");
        assert_eq!(staging_entries.len(), 1);
        assert_eq!(
            fs::read(staging_entries[0].path()).expect("colliding writer bytes"),
            b"owned by another writer"
        );
    }

    #[test]
    fn recovery_coordination_lock_excludes_active_cross_instance_publication() {
        use std::{fs::TryLockError, sync::mpsc, thread};

        let root = TempDir::new().expect("root");
        let publisher =
            LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
                .expect("publisher store");
        let recovery =
            LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
                .expect("recovery store");
        let bytes = b"coordinated publication".to_vec();
        let record = record(&bytes);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let publisher_thread = thread::spawn(move || {
            publisher.publish(&record, |writer| {
                entered_tx.send(()).expect("entered signal");
                release_rx.recv().expect("release signal");
                writer.write_all(&bytes)
            })
        });
        entered_rx.recv().expect("publisher entered callback");

        let coordination = recovery
            .open_coordination_file()
            .expect("coordination file");
        assert!(matches!(
            coordination.try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        release_tx.send(()).expect("release publisher");
        publisher_thread
            .join()
            .expect("publisher thread")
            .expect("publication");
        coordination
            .try_lock()
            .expect("exclusive lock after publication");
    }

    #[test]
    fn recovery_reports_quarantine_target_across_every_partial_move_phase() {
        for (fault, staging_remains, completed_move) in [
            (RecoveryFault::AfterQuarantineLink, true, false),
            (RecoveryFault::AfterQuarantineSync, true, false),
            (RecoveryFault::AfterStagingRemove, false, true),
        ] {
            let root = TempDir::new().expect("root");
            let store =
                LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
                    .expect("store");
            let record = record(b"partial recovery");
            let name = format!("marklab-3-1-{}.part", record.id());
            let staging_path = root.path().join(STAGING_DIRECTORY).join(&name);
            fs::write(&staging_path, b"partial recovery").expect("staging fixture");

            let report = store
                .recover_staging_with_fault(fault)
                .expect("recovery report");
            assert_eq!(report.issues().len(), 1);
            let target = report.issues()[0]
                .quarantine_target()
                .expect("reported quarantine target");
            assert!(root
                .path()
                .join(QUARANTINE_DIRECTORY)
                .join(target)
                .is_file());
            assert_eq!(staging_path.exists(), staging_remains);
            assert_eq!(!report.quarantined().is_empty(), completed_move);
        }
    }

    #[test]
    fn failure_after_publication_leaves_durable_final_and_recoverable_staging() {
        let root = TempDir::new().expect("root");
        let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
            .expect("store");
        let bytes = b"postpublication fault";
        let record = record(bytes);
        assert!(matches!(
            store.publish_with_fault(
                &record,
                |writer| writer.write_all(bytes),
                PublishFault::AfterPublication,
            ),
            Err(ArtifactStoreError::PublishedButCleanupFailed {
                operation: "injected after publication",
                ..
            })
        ));
        assert_eq!(fs::read(final_path(&root, &record)).expect("final"), bytes);
        assert_eq!(
            fs::read_dir(root.path().join(STAGING_DIRECTORY))
                .expect("staging")
                .count(),
            1
        );
        drop(store);

        let reopened =
            LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
                .expect("reopened store");
        let report = reopened.recover_staging().expect("recovery");
        assert_eq!(report.quarantined().len(), 1);
        assert!(report.issues().is_empty());
        let mut located = record.clone();
        located
            .merge_locations(&[ArtifactLocator::managed(
                StoreId::new("local").expect("store ID"),
                record.id(),
            )])
            .expect("managed locator");
        reopened.verify(&located).expect("durable final verifies");
    }
}
