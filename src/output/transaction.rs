use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::errors::{MarklabError, Result};

static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct OutputTransaction {
    final_path: PathBuf,
    staging_path: PathBuf,
    replace_empty_target: bool,
    committed: bool,
}

impl OutputTransaction {
    pub(super) fn new(final_path: &Path) -> Result<Self> {
        let file_name = final_path.file_name().ok_or_else(|| {
            MarklabError::Validation(format!(
                "output path must name a run directory: {}",
                final_path.display()
            ))
        })?;
        let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;

        let replace_empty_target = match fs::symlink_metadata(final_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(MarklabError::Validation(format!(
                        "output path must be a directory and may not be a symbolic link: {}",
                        final_path.display()
                    )));
                }
                if fs::read_dir(final_path)
                    .map_err(|source| MarklabError::io(final_path, source))?
                    .next()
                    .is_some()
                {
                    return Err(MarklabError::Validation(format!(
                        "refusing to overwrite non-empty output directory: {}",
                        final_path.display()
                    )));
                }
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(source) => return Err(MarklabError::io(final_path, source)),
        };

        let staging_path = create_staging_directory(parent, file_name)?;
        Ok(Self {
            final_path: final_path.to_path_buf(),
            staging_path,
            replace_empty_target,
            committed: false,
        })
    }

    pub(super) fn staging_path(&self) -> &Path {
        &self.staging_path
    }

    pub(super) fn commit(mut self) -> Result<()> {
        if self.replace_empty_target {
            fs::remove_dir(&self.final_path)
                .map_err(|source| MarklabError::io(&self.final_path, source))?;
        }
        fs::rename(&self.staging_path, &self.final_path)
            .map_err(|source| MarklabError::io(&self.final_path, source))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for OutputTransaction {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.staging_path);
        }
    }
}

fn create_staging_directory(parent: &Path, file_name: &std::ffi::OsStr) -> Result<PathBuf> {
    for _ in 0..32 {
        let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".marklab-tmp-{}-{sequence}", std::process::id()));
        let path = parent.join(temporary_name);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(MarklabError::io(&path, source)),
        }
    }
    Err(MarklabError::Io {
        path: parent.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not reserve a unique temporary output directory",
        ),
    })
}
