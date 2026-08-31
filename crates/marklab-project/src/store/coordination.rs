use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(windows)]
use cap_std::fs::OpenOptionsExt;
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};

use super::{
    ArtifactStoreError, LocalArtifactStore, COORDINATION_FILE, OBJECT_DIRECTORY,
    QUARANTINE_DIRECTORY, STAGING_DIRECTORY,
};
use crate::{ArtifactId, StoreId};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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

    pub(super) fn ensure_directory(&self, path: &str) -> Result<(), ArtifactStoreError> {
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

    pub(super) fn create_staging_key(&self, id: ArtifactId) -> Result<String, ArtifactStoreError> {
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

    pub(super) fn open_coordination_file(&self) -> Result<fs::File, ArtifactStoreError> {
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

    pub(super) fn acquire_coordination_lock(
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
pub(super) enum CoordinationMode {
    Shared,
    Exclusive,
}

pub(super) struct CoordinationGuard {
    _file: fs::File,
}

pub(super) fn split_parent(key: &str) -> (&str, &str) {
    key.rsplit_once('/')
        .expect("managed object key always contains a parent")
}

fn sync_parent_directory(root: &Dir, path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    sync_directory_at(root, parent)
}

#[cfg(not(windows))]
pub(super) fn sync_directory_at(root: &Dir, path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    let directory = if path.as_os_str().is_empty() {
        root.try_clone()?
    } else {
        root.open_dir(path)?
    };
    directory.into_std_file().sync_all()
}

#[cfg(windows)]
pub(super) fn sync_directory_at(root: &Dir, path: impl AsRef<Path>) -> io::Result<()> {
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
