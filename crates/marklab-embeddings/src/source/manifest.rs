use std::io::SeekFrom;

use marklab_project::{ArtifactReadSeek, ContentDigest};
use serde::Deserialize;

use super::{
    ManifestFailure, SourceBundleBudgets, SourceBundleError, SourceFileKind, SourceIoFailure,
    SourceIoOperation,
};

const MAX_MANIFEST_FILE_BYTES: u64 = 64 * 1024;
const MANIFEST_DECODE_MULTIPLIER: usize = 8;
const MANIFEST_DECODE_OVERHEAD: usize = 4 * 1024;

#[derive(Clone, Copy)]
pub(super) struct ManifestSummary {
    pub(super) content_digest: ContentDigest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWire {
    block_um: f64,
    coordinate_unit: String,
    deduplicated_cells: u64,
    embedding_dtype: String,
    embedding_width: u64,
    n_cells: u64,
    row_alignment: String,
    schema_name: String,
    schema_version: String,
    sources: Vec<ManifestSourceWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSourceWire {
    cells_json_sha256: String,
    cells_pt_sha256: String,
    graph_position_scale: f64,
    mpp: f64,
    roi_id: String,
    slide_path: String,
    specimen_id: String,
    timepoint: String,
    tumor_mask: String,
}

pub(super) fn parse_reader(
    reader: &mut dyn ArtifactReadSeek,
    expected_rows: u64,
    budgets: SourceBundleBudgets,
) -> Result<ManifestSummary, SourceBundleError> {
    let file_bytes = reader
        .seek(SeekFrom::End(0))
        .map_err(|error| io_error(SourceIoOperation::InspectLength, &error))?;
    let maximum = budgets
        .maximum_manifest_file_bytes()
        .min(MAX_MANIFEST_FILE_BYTES);
    if file_bytes > maximum {
        return Err(SourceBundleError::FileByteBudgetExceeded {
            file: SourceFileKind::Manifest,
            observed: file_bytes,
            maximum,
        });
    }
    let file_bytes_usize =
        usize::try_from(file_bytes).map_err(|_| SourceBundleError::SizeOverflow)?;
    let retained = file_bytes_usize
        .checked_mul(MANIFEST_DECODE_MULTIPLIER)
        .and_then(|value| value.checked_add(MANIFEST_DECODE_OVERHEAD))
        .ok_or(SourceBundleError::SizeOverflow)?;
    if retained > budgets.maximum_retained_bytes() {
        return Err(SourceBundleError::RetainedByteBudgetExceeded {
            required: retained,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(SourceIoOperation::Seek, &error))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(file_bytes_usize)
        .map_err(|_| SourceBundleError::AllocationFailed {
            requested: file_bytes_usize,
        })?;
    bytes.resize(file_bytes_usize, 0);
    read_exact(reader, &mut bytes)?;
    let content_digest = ContentDigest::from_bytes(&bytes);
    let wire: ManifestWire =
        serde_json::from_slice(&bytes).map_err(|_| manifest_error(ManifestFailure::InvalidJson))?;
    validate(wire, expected_rows)?;
    Ok(ManifestSummary { content_digest })
}

fn validate(wire: ManifestWire, expected_rows: u64) -> Result<(), SourceBundleError> {
    if !wire.block_um.is_finite()
        || wire.block_um != 500.0
        || wire.coordinate_unit != "micrometers"
        || wire.deduplicated_cells != 0
        || wire.embedding_dtype != "float32"
        || wire.embedding_width != 1_280
        || wire.row_alignment != "cells.csv embedding_row equals embeddings.npy row"
        || wire.schema_name != "cellvit_he_bundle"
        || wire.schema_version != "1.0"
        || wire.sources.len() != 1
    {
        return Err(manifest_error(ManifestFailure::InvalidProfile));
    }
    if wire.n_cells != expected_rows {
        return Err(manifest_error(ManifestFailure::RowCountMismatch));
    }
    let source = wire
        .sources
        .into_iter()
        .next()
        .ok_or_else(|| manifest_error(ManifestFailure::InvalidProfile))?;
    if !source.graph_position_scale.is_finite()
        || source.graph_position_scale != 0.662_356_930_902_925
        || !source.mpp.is_finite()
        || source.mpp != 0.377_44
    {
        return Err(manifest_error(ManifestFailure::InvalidProfile));
    }
    if !is_sha256(&source.cells_json_sha256) || !is_sha256(&source.cells_pt_sha256) {
        return Err(manifest_error(ManifestFailure::InvalidHash));
    }
    for value in [
        source.roi_id,
        source.slide_path,
        source.specimen_id,
        source.timepoint,
        source.tumor_mask,
    ] {
        if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(manifest_error(ManifestFailure::InvalidSensitiveString));
        }
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn read_exact(
    reader: &mut dyn ArtifactReadSeek,
    mut bytes: &mut [u8],
) -> Result<(), SourceBundleError> {
    while !bytes.is_empty() {
        match reader.read(bytes) {
            Ok(0) => {
                return Err(SourceBundleError::Io {
                    operation: SourceIoOperation::Read,
                    reason: SourceIoFailure::UnexpectedEnd,
                });
            }
            Ok(read) => bytes = &mut bytes[read..],
            Err(error) => return Err(io_error(SourceIoOperation::Read, &error)),
        }
    }
    Ok(())
}

fn io_error(operation: SourceIoOperation, error: &std::io::Error) -> SourceBundleError {
    SourceBundleError::Io {
        operation,
        reason: if error.kind() == std::io::ErrorKind::UnexpectedEof {
            SourceIoFailure::UnexpectedEnd
        } else {
            SourceIoFailure::Other
        },
    }
}

fn manifest_error(reason: ManifestFailure) -> SourceBundleError {
    SourceBundleError::Manifest { reason }
}
