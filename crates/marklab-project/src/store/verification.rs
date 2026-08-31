use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use super::{
    ArtifactReadSeek, ArtifactStoreError, LocalArtifactStore, VerifiedReaderError,
    STREAM_BUFFER_BYTES,
};
use crate::{ArtifactId, ArtifactLocator, ArtifactRecord, ContentDigest};

impl LocalArtifactStore {
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
            .acquire_coordination_lock(super::coordination::CoordinationMode::Shared)
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

    pub(super) fn verify_locator(
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
}

fn map_open_error(artifact: ArtifactId, key: &str, source: std::io::Error) -> ArtifactStoreError {
    if source.kind() == std::io::ErrorKind::NotFound {
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
    source: std::io::Error,
) -> ArtifactStoreError {
    if source.kind() == std::io::ErrorKind::NotFound {
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
