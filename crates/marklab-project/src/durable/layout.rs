use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use cap_std::fs::{Dir, OpenOptions};

use super::{
    wire::{ensure_size, LEDGER_PATH, LOCK_PATH, PENDING_PATH},
    DurableProjectError,
};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) fn ensure_project_root(root: &Path) -> Result<(), DurableProjectError> {
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

pub(super) fn ensure_directory(root: &Dir, path: &str) -> Result<(), DurableProjectError> {
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

pub(super) fn ensure_lock_file(root: &Dir) -> Result<(), DurableProjectError> {
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

pub(super) fn open_and_lock(root: &Dir) -> Result<fs::File, DurableProjectError> {
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

pub(super) fn create_empty_ledger(root: &Dir) -> Result<(), DurableProjectError> {
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

pub(super) fn atomic_write(
    root: &Dir,
    destination: &str,
    bytes: &[u8],
) -> Result<(), DurableProjectError> {
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

pub(super) fn remove_pending(root: &Dir) -> Result<(), DurableProjectError> {
    root.remove_file(PENDING_PATH)
        .map_err(|source| DurableProjectError::Io {
            operation: "remove completed pending execution",
            path: PENDING_PATH.to_owned(),
            source,
        })?;
    sync_root(root)
}

pub(super) fn read_bounded(
    root: &Dir,
    path: &str,
    maximum: usize,
) -> Result<Vec<u8>, DurableProjectError> {
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

pub(super) fn path_exists(root: &Dir, path: &str) -> Result<bool, DurableProjectError> {
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

pub(super) fn validate_regular_metadata(
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
pub(super) fn sync_root(root: &Dir) -> Result<(), DurableProjectError> {
    // Reopen for reading because Linux O_PATH capabilities cannot be fsynced.
    root.open(".")
        .and_then(|directory| directory.sync_all())
        .map_err(|source| DurableProjectError::Io {
            operation: "sync project root",
            path: ".".to_owned(),
            source,
        })
}

#[cfg(windows)]
pub(super) fn sync_root(root: &Dir) -> Result<(), DurableProjectError> {
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
