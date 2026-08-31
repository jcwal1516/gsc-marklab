use marklab_data::{CellId, HierarchyId};
use marklab_project::{ArtifactRecord, ContentDigest, VerifiedReaderError};

use crate::{CellEmbeddingRowLinkEntry, EmbeddingError, EmbeddingStatus, ExpectedCellSet};

use super::{
    super::{
        csv::ParsedCsv, enforce_retained, npy::NPY_STREAM_BUFFER_BYTES, ImportFailure,
        SourceBundleBudgets, SourceBundleError,
    },
    CellVitHeImportRequest, MAX_NPY_HEADER_RETAINED_BYTES,
};

pub(super) fn validate_request(
    request: CellVitHeImportRequest<'_>,
) -> Result<(), SourceBundleError> {
    if request.expected.cells().is_empty() {
        return Err(import_error(ImportFailure::EmptyExpectedCells));
    }
    let bindings = request.bindings;
    if request.identity_map.source_cells_artifact_id() != bindings.source_cells_artifact_id()
        || request.identity_map.expected_cells_artifact_id()
            != bindings.expected_cells_artifact_id()
    {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    if request.identity_map.expected_cells_logical_digest() != request.expected.logical_digest()
        || request.identity_map.entries().len() != request.expected.cells().len()
    {
        return Err(import_error(ImportFailure::IdentityMapMismatch));
    }
    require_domain_payload(
        &bindings.expected_cells,
        request.expected.encoded_byte_len(),
        request.budgets,
        || request.expected.to_bytes(),
    )?;
    require_domain_payload(
        &bindings.identity_map,
        request.identity_map.encoded_byte_len(),
        request.budgets,
        || request.identity_map.to_bytes(),
    )?;
    if request.expected.cells().iter().any(|cell_id| {
        request
            .hierarchy
            .node(&HierarchyId::from(cell_id.clone()))
            .is_none()
    }) {
        return Err(import_error(ImportFailure::HierarchyMismatch));
    }
    Ok(())
}

pub(super) fn require_record(
    record: &ArtifactRecord,
    schema_id: &str,
    content_kind: Option<&str>,
) -> Result<(), SourceBundleError> {
    if record.schema().id() != schema_id
        || record.schema().version() != 1
        || content_kind.is_some_and(|kind| record.content().kind() != kind)
        || record.table().is_some()
        || !record.semantic_metadata().is_empty()
    {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

pub(super) fn require_schema(
    record: &ArtifactRecord,
    schema_id: &str,
) -> Result<(), SourceBundleError> {
    if record.schema().id() != schema_id || record.schema().version() != 1 {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

fn require_domain_payload<F>(
    record: &ArtifactRecord,
    encoded_byte_len: Result<usize, EmbeddingError>,
    budgets: SourceBundleBudgets,
    encode: F,
) -> Result<(), SourceBundleError>
where
    F: FnOnce() -> Result<Vec<u8>, EmbeddingError>,
{
    let encoded_byte_len = encoded_byte_len.map_err(domain_error)?;
    if u64::try_from(encoded_byte_len).map_err(|_| SourceBundleError::SizeOverflow)?
        != record.content().byte_len()
    {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    enforce_retained(encoded_byte_len, budgets)?;
    let bytes = encode().map_err(domain_error)?;
    if ContentDigest::from_bytes(&bytes) != record.content().digest() {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

pub(super) fn require_source_identity(
    record: &ArtifactRecord,
    digest: ContentDigest,
    byte_len: u64,
) -> Result<(), SourceBundleError> {
    if record.content().digest() != digest || record.content().byte_len() != byte_len {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

pub(super) fn verified_reader_error(
    error: VerifiedReaderError<SourceBundleError>,
) -> SourceBundleError {
    match error {
        VerifiedReaderError::Store(_) => import_error(ImportFailure::ArtifactUnavailable),
        VerifiedReaderError::Callback(error) => error,
    }
}

pub(super) fn bind_identity_rows(
    parsed: &mut ParsedCsv,
    request: CellVitHeImportRequest<'_>,
) -> Result<(), SourceBundleError> {
    if parsed.rows.len() != request.identity_map.entries().len() {
        return Err(import_error(ImportFailure::IdentityMapMismatch));
    }
    parsed
        .rows
        .sort_unstable_by(|left, right| left.source_cell_id.cmp(&right.source_cell_id));
    for (source_row, identity) in parsed.rows.iter_mut().zip(request.identity_map.entries()) {
        if source_row.source_cell_id != identity.source_cell_id() {
            return Err(import_error(ImportFailure::IdentityMapMismatch));
        }
        source_row.canonical_row = request
            .expected
            .cells()
            .binary_search(identity.cell_id())
            .map_err(|_| import_error(ImportFailure::IdentityMapMismatch))?;
    }
    parsed.rows.sort_unstable_by_key(|row| row.native_row);
    Ok(())
}

pub(super) fn component_count(rows: u64, dimension: u32) -> Result<usize, SourceBundleError> {
    rows.checked_mul(u64::from(dimension))
        .ok_or(SourceBundleError::SizeOverflow)
        .and_then(|count| usize::try_from(count).map_err(|_| SourceBundleError::SizeOverflow))
}

pub(super) fn peak_retained_bytes(
    parsed: &ParsedCsv,
    expected: &ExpectedCellSet,
    component_count: usize,
) -> Result<usize, SourceBundleError> {
    let value_bytes = component_count
        .checked_mul(size_of::<f32>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(cell.as_str().len())
            .ok_or(SourceBundleError::SizeOverflow)
    })?;
    let table_rows = expected
        .cells()
        .len()
        .checked_mul(size_of::<CellId>() + size_of::<EmbeddingStatus>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    let row_links = expected
        .cells()
        .len()
        .checked_mul(size_of::<CellEmbeddingRowLinkEntry>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    parsed
        .retained_bytes
        .checked_add(MAX_NPY_HEADER_RETAINED_BYTES)
        .and_then(|value| value.checked_add(NPY_STREAM_BUFFER_BYTES))
        .and_then(|value| value.checked_add(value_bytes))
        .and_then(|value| value.checked_add(table_rows))
        .and_then(|value| value.checked_add(row_links))
        .and_then(|value| value.checked_add(identifier_bytes.checked_mul(2)?))
        .ok_or(SourceBundleError::SizeOverflow)
}

pub(super) fn import_error(reason: ImportFailure) -> SourceBundleError {
    SourceBundleError::Import { reason }
}

pub(super) fn domain_error(error: EmbeddingError) -> SourceBundleError {
    match error {
        EmbeddingError::SizeOverflow => SourceBundleError::SizeOverflow,
        EmbeddingError::AllocationFailed { requested } => {
            SourceBundleError::AllocationFailed { requested }
        }
        EmbeddingError::RetainedByteBudgetExceeded { required, maximum } => {
            SourceBundleError::RetainedByteBudgetExceeded { required, maximum }
        }
        EmbeddingError::RowLinkCellMissingFromHierarchy => {
            import_error(ImportFailure::HierarchyMismatch)
        }
        _ => import_error(ImportFailure::DomainConstruction),
    }
}
