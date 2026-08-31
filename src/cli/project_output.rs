use std::{fs::OpenOptions, io::Write, path::Path};

use crate::{CacheStatus, MarklabError, Result};

pub(super) fn write_output(
    path: &Path,
    encoded: &[u8],
    cache_status: CacheStatus,
    command: &str,
) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| MarklabError::io(path, source))?;
    output
        .write_all(encoded)
        .map_err(|source| MarklabError::io(path, source))?;
    output
        .sync_all()
        .map_err(|source| MarklabError::io(path, source))?;
    let cache = match cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project {command} cache_status={cache}");
    Ok(())
}
