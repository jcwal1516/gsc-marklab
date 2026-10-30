//! Native project provenance and bounded source-file helpers shared by spatial commands.

use std::{
    fs::File,
    io::{Read, Write},
};

use crate::{
    workflow::{
        ArtifactRef, ContentDigest, DurableProject, DurableRecoveryAction, NativeRuntimeProvenance,
    },
    MarklabError, Result,
};

const MAXIMUM_EXECUTABLE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

pub(super) fn source_artifact(path: &std::path::Path, kind: &str) -> Result<ArtifactRef> {
    let mut file = File::open(path).map_err(|source| MarklabError::io(path, source))?;
    let mut digest = ContentDigest::builder();
    let copied =
        std::io::copy(&mut file, &mut digest).map_err(|source| MarklabError::io(path, source))?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(MarklabError::Validation(
            "source hashing byte count mismatch".to_owned(),
        ));
    }
    ArtifactRef::new(kind, digest, byte_len)
        .map_err(|error| MarklabError::Validation(error.to_string()))
}

pub(super) fn native_runtime_provenance() -> Result<NativeRuntimeProvenance> {
    let executable = executable_artifact()?;
    let git_sha = match env!("MARKLAB_BUILD_GIT_SHA") {
        "" => None,
        value => Some(value.to_owned()),
    };
    let git_dirty = match env!("MARKLAB_BUILD_GIT_DIRTY") {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    };
    NativeRuntimeProvenance::new(
        env!("CARGO_PKG_VERSION"),
        git_sha,
        git_dirty,
        env!("MARKLAB_BUILD_RUSTC"),
        compiled_features(),
        executable,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))
}

fn executable_artifact() -> Result<ArtifactRef> {
    let path = std::env::current_exe().map_err(MarklabError::CommandIo)?;
    let mut file = File::open(&path).map_err(|source| MarklabError::io(&path, source))?;
    let mut digest = ContentDigest::builder();
    let copied = io_copy_bounded(&mut file, &mut digest, MAXIMUM_EXECUTABLE_BYTES)
        .map_err(|source| MarklabError::io(&path, source))?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(MarklabError::Validation(
            "executable hashing byte count mismatch".to_owned(),
        ));
    }
    ArtifactRef::new("application/vnd.marklab.executable", digest, byte_len)
        .map_err(|error| MarklabError::Validation(error.to_string()))
}

fn io_copy_bounded(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    maximum: u64,
) -> std::io::Result<u64> {
    let mut limited = reader.take(maximum.saturating_add(1));
    let copied = std::io::copy(&mut limited, writer)?;
    if copied > maximum {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Marklab executable exceeds the four-GiB provenance hashing limit",
        ));
    }
    Ok(copied)
}

fn compiled_features() -> Vec<String> {
    let candidates = [
        (cfg!(feature = "allocator-mimalloc"), "allocator-mimalloc"),
        (cfg!(feature = "cli"), "cli"),
        (cfg!(feature = "csv"), "csv"),
        (cfg!(feature = "dhat-heap"), "dhat-heap"),
        (cfg!(feature = "parallel"), "parallel"),
        (cfg!(feature = "parquet"), "parquet"),
        (cfg!(feature = "wsi"), "wsi"),
    ];
    candidates
        .into_iter()
        .filter_map(|(enabled, name)| enabled.then_some(name.to_owned()))
        .collect()
}

pub(super) fn report_recovery(project: &DurableProject) {
    let report = project.open_report();
    if report.action() != DurableRecoveryAction::None
        || !report.artifact_store().quarantined().is_empty()
        || !report.artifact_store().issues().is_empty()
    {
        eprintln!(
            "project recovery: action={:?}, quarantined_objects={}, retained_issues={}",
            report.action(),
            report.artifact_store().quarantined().len(),
            report.artifact_store().issues().len()
        );
    }
}

pub(super) fn read_bounded_utf8(path: &std::path::Path, maximum_bytes: usize) -> Result<String> {
    let file = File::open(path).map_err(|source| MarklabError::io(path, source))?;
    let limit = u64::try_from(maximum_bytes)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| MarklabError::Validation("window byte limit is too large".into()))?;
    let mut text = String::new();
    file.take(limit)
        .read_to_string(&mut text)
        .map_err(|source| MarklabError::io(path, source))?;
    if text.len() > maximum_bytes {
        return Err(MarklabError::Geometry(format!(
            "window GeoJSON has more than {maximum_bytes} bytes"
        )));
    }
    Ok(text)
}
