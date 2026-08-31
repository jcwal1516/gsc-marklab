use std::io::{self, Write};

use cap_std::fs::OpenOptions;

use super::{
    coordination::{split_parent, sync_directory_at, CoordinationMode},
    ArtifactPublication, ArtifactStoreError, LocalArtifactStore, PublicationDisposition,
    PublishFault, STAGING_DIRECTORY,
};
use crate::{ArtifactDraft, ArtifactLocator, ArtifactRecord, ContentDigest, ContentDigestWriter};

impl LocalArtifactStore {
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

    /// Stream, verify, and durably publish a location-free fresh artifact declaration.
    ///
    /// No `ArtifactRecord` is returned unless the managed object already exists with exact
    /// identity or this call completes durable publication and cleanup successfully.
    pub fn publish_new_send<F>(
        &self,
        draft: &ArtifactDraft,
        write: F,
    ) -> Result<ArtifactPublication, ArtifactStoreError>
    where
        F: FnOnce(&mut (dyn Write + Send)) -> io::Result<()>,
    {
        let managed = ArtifactLocator::managed(self.store_id.clone(), draft.id());
        let record = draft.located_record(managed);
        self.publish_inner(&record, |writer| write(writer), PublishFault::None)
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
    pub(super) fn publish_with_fault<F>(
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

    fn publication(
        located_record: &ArtifactRecord,
        disposition: PublicationDisposition,
    ) -> ArtifactPublication {
        ArtifactPublication {
            record: located_record.clone(),
            disposition,
        }
    }
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
    root: &'a cap_std::fs::Dir,
    key: String,
    armed: bool,
}

impl<'a> StagingGuard<'a> {
    fn new(root: &'a cap_std::fs::Dir, key: String) -> Self {
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
