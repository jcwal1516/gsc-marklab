use std::{fs::File, io, path::Path};

use crate::{
    exclusive_file_output::{self, ExclusiveFileError},
    MarklabError, Result,
};

pub(crate) fn publish_exclusive_file(
    path: &Path,
    staging_namespace: &str,
    create_parent: bool,
    write: impl FnOnce(&mut File) -> Result<()>,
) -> Result<()> {
    exclusive_file_output::publish_exclusive_file(path, staging_namespace, create_parent, write)
        .map_err(|error| match error {
            ExclusiveFileError::OutputExists => MarklabError::io(
                path,
                io::Error::new(io::ErrorKind::AlreadyExists, "output already exists"),
            ),
            ExclusiveFileError::OutputMustNameFile => MarklabError::Validation(format!(
                "output path must name a file: {}",
                path.display()
            )),
            ExclusiveFileError::Io { path, source } => MarklabError::io(path, source),
            ExclusiveFileError::Write(error) => error,
        })
}
