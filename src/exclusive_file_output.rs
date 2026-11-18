use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) enum ExclusiveFileError<E> {
    OutputExists,
    OutputMustNameFile,
    Io { path: PathBuf, source: io::Error },
    Write(E),
}

pub(crate) fn publish_exclusive_file<E>(
    path: &Path,
    staging_namespace: &str,
    create_parent: bool,
    write: impl FnOnce(&mut File) -> Result<(), E>,
) -> Result<(), ExclusiveFileError<E>> {
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(ExclusiveFileError::OutputExists),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ExclusiveFileError::Io {
                path: path.to_owned(),
                source,
            });
        }
    }

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if create_parent {
        fs::create_dir_all(parent).map_err(|source| ExclusiveFileError::Io {
            path: parent.to_owned(),
            source,
        })?;
    }
    let file_name = path
        .file_name()
        .ok_or(ExclusiveFileError::OutputMustNameFile)?;
    let (staging, mut file) = create_staging_file(parent, file_name, staging_namespace)?;
    write(&mut file).map_err(ExclusiveFileError::Write)?;
    file.sync_all().map_err(|source| ExclusiveFileError::Io {
        path: staging.0.clone(),
        source,
    })?;
    drop(file);
    // Claim the destination only after the complete file has been synced.
    fs::hard_link(&staging.0, path).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            ExclusiveFileError::OutputExists
        } else {
            ExclusiveFileError::Io {
                path: path.to_owned(),
                source,
            }
        }
    })
}

fn create_staging_file<E>(
    parent: &Path,
    final_file_name: &OsStr,
    staging_namespace: &str,
) -> Result<(StagingPath, File), ExclusiveFileError<E>> {
    for _ in 0..32 {
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging_name = format!(
            ".marklab-{staging_namespace}-{}-{sequence}.tmp",
            std::process::id()
        );
        if final_file_name.eq_ignore_ascii_case(&staging_name) {
            continue;
        }
        let path = parent.join(staging_name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((StagingPath(path), file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(ExclusiveFileError::Io { path, source }),
        }
    }
    Err(ExclusiveFileError::Io {
        path: parent.to_owned(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a unique temporary output file",
        ),
    })
}

struct StagingPath(PathBuf);

impl Drop for StagingPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
