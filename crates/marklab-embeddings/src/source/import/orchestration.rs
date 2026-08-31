use std::io::Cursor;

use marklab_project::{ArtifactReadSeek, LocalArtifactStore};

use crate::{digest::canonical_positive_zero, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry};

use super::{
    super::{
        csv::{parse_reader as parse_csv_reader, ParsedCsv},
        enforce_retained,
        npy::{reader_shape as npy_reader_shape, visit_reader as visit_npy_reader},
        SourceBundleError,
    },
    validation::{
        bind_identity_rows, component_count, domain_error, import_error, peak_retained_bytes,
        require_source_identity, validate_request, verified_reader_error,
    },
    CellVitHeImportCandidate, CellVitHeImportRequest, ImportFailure, MAX_NPY_HEADER_RETAINED_BYTES,
};

/// Import complete borrowed NPY/CSV bytes through the same bounded seekable-reader path.
pub fn import_cellvit_he_bundle_bytes(
    npy_bytes: &[u8],
    csv_bytes: &[u8],
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    let mut npy_reader = Cursor::new(npy_bytes);
    let mut csv_reader = Cursor::new(csv_bytes);
    import_cellvit_he_bundle_readers(&mut npy_reader, &mut csv_reader, request)
}

/// Import bounded seekable NPY/CSV readers into one canonical allocation.
pub fn import_cellvit_he_bundle_readers(
    npy_reader: &mut dyn ArtifactReadSeek,
    csv_reader: &mut dyn ArtifactReadSeek,
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    validate_request(request)?;
    let mut parsed = parse_csv_reader(csv_reader, request.budgets)?;
    require_source_identity(
        &request.bindings.source_cells,
        parsed.summary.content_digest(),
        parsed.summary.encoded_byte_len(),
    )?;
    bind_identity_rows(&mut parsed, request)?;
    finish_import(npy_reader, parsed, request)
}

/// Import bound managed source records through post-verified store readers.
pub fn import_cellvit_he_bundle_from_store(
    store: &LocalArtifactStore,
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    validate_request(request)?;
    let mut parsed = store
        .with_verified_reader(&request.bindings.source_cells, |csv_reader| {
            let parsed = parse_csv_reader(csv_reader, request.budgets)?;
            require_source_identity(
                &request.bindings.source_cells,
                parsed.summary.content_digest(),
                parsed.summary.encoded_byte_len(),
            )?;
            Ok(parsed)
        })
        .map_err(verified_reader_error)?;
    bind_identity_rows(&mut parsed, request)?;
    store
        .with_verified_reader(&request.bindings.source_vectors, |npy_reader| {
            finish_import(npy_reader, parsed, request)
        })
        .map_err(verified_reader_error)
}

fn finish_import(
    npy_reader: &mut dyn ArtifactReadSeek,
    mut parsed: ParsedCsv,
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    let shape_peak = parsed
        .retained_bytes
        .checked_add(MAX_NPY_HEADER_RETAINED_BYTES)
        .ok_or(SourceBundleError::SizeOverflow)?;
    enforce_retained(shape_peak, request.budgets)?;
    let (npy_rows, dimension) = npy_reader_shape(npy_reader, request.budgets)?;
    if npy_rows != parsed.summary.row_count() {
        return Err(SourceBundleError::RowCountMismatch {
            npy: npy_rows,
            csv: parsed.summary.row_count(),
        });
    }

    let component_count = component_count(npy_rows, dimension)?;
    let required = peak_retained_bytes(&parsed, request.expected, component_count)?;
    enforce_retained(required, request.budgets)?;
    let value_bytes = component_count
        .checked_mul(size_of::<f32>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(component_count)
        .map_err(|_| SourceBundleError::AllocationFailed {
            requested: value_bytes,
        })?;
    values.resize(component_count, 0.0);

    let dimension_usize =
        usize::try_from(dimension).map_err(|_| SourceBundleError::SizeOverflow)?;
    let npy_summary = visit_npy_reader(npy_reader, request.budgets, &mut |row, column, value| {
        let source_row = usize::try_from(row).map_err(|_| SourceBundleError::SizeOverflow)?;
        let canonical_row = parsed
            .rows
            .get(source_row)
            .ok_or(SourceBundleError::SizeOverflow)?
            .canonical_row;
        let index = canonical_row
            .checked_mul(dimension_usize)
            .and_then(|offset| offset.checked_add(column as usize))
            .ok_or(SourceBundleError::SizeOverflow)?;
        *values
            .get_mut(index)
            .ok_or(SourceBundleError::SizeOverflow)? = canonical_positive_zero(value);
        Ok(())
    })?;
    require_source_identity(
        &request.bindings.source_vectors,
        npy_summary.content_digest(),
        npy_summary.encoded_byte_len(),
    )?;

    let csv_summary = parsed.summary;
    parsed.rows.sort_unstable_by_key(|row| row.canonical_row);
    let mut row_entries = Vec::new();
    row_entries
        .try_reserve_exact(parsed.rows.len())
        .map_err(|_| SourceBundleError::AllocationFailed {
            requested: parsed
                .rows
                .len()
                .saturating_mul(size_of::<CellEmbeddingRowLinkEntry>()),
        })?;
    for (canonical_row, source_row) in parsed.rows.into_iter().enumerate() {
        if source_row.canonical_row != canonical_row {
            return Err(import_error(ImportFailure::IdentityMapMismatch));
        }
        let cell_id = request
            .expected
            .cells()
            .get(canonical_row)
            .ok_or_else(|| import_error(ImportFailure::IdentityMapMismatch))?
            .clone();
        row_entries.push(CellEmbeddingRowLinkEntry::present(
            cell_id,
            source_row.native_row,
            source_row.embedding_row,
        ));
    }

    let bindings = request.bindings;
    let row_link = CellEmbeddingRowLink::new(
        bindings.source_cells_artifact_id(),
        bindings.source_vectors_artifact_id(),
        bindings.expected_cells_artifact_id(),
        bindings.identity_map_artifact_id(),
        bindings.converter_artifact_id(),
        request.expected,
        request.hierarchy,
        row_entries,
        request.budgets.maximum_retained_bytes(),
    )
    .map_err(domain_error)?;
    Ok(CellVitHeImportCandidate {
        values,
        dimension,
        row_link,
        expected_cells_logical_digest: request.expected.logical_digest(),
        maximum_retained_bytes: request.budgets.maximum_retained_bytes(),
        npy_summary,
        csv_summary,
    })
}
