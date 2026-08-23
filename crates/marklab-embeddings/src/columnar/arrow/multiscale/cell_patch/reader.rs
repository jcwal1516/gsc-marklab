use std::io::{Cursor, Read, Seek};

use arrow::array::{Array, StringArray, UInt64Array};
use arrow_ipc::reader::FileReaderBuilder;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};

use crate::{
    columnar::{
        multiscale::{cell_patch_dependencies, enforce_file_budget, enforce_retained_budget},
        EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
    },
    multiscale::physical::{record_matches_encoding, SpatialPhysicalEncoding},
    CellPatchAssignmentMode, CellPatchLink, VerifiedCellPatchAssignmentArtifact,
    VerifiedCellPatchEdgeArtifact, VerifiedCellPatchInputArtifactGraph,
};

use super::{
    preflight::{
        preflight_cell_patch_assignment_table_arrow_bytes,
        preflight_cell_patch_edge_table_arrow_bytes, preflight_reader,
        CellPatchAssignmentArrowPreflight, CellPatchEdgeArrowPreflight, CommonArrowPreflight,
    },
    profile::{arrow_failure, expected_schema, CellPatchArrowProfile},
};

/// Fully decode and validate borrowed canonical cell-patch assignment Arrow bytes.
pub fn validate_cell_patch_assignment_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentArrowPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, CellPatchArrowProfile::Assignment, link)?;
    let preflight = preflight_cell_patch_assignment_table_arrow_bytes(bytes, link, budgets)?;
    validate_identity(
        preflight.content_digest(),
        preflight.encoded_byte_len(),
        record,
    )?;
    decode_arrow(
        Cursor::new(bytes),
        CellPatchArrowProfile::Assignment,
        link,
        preflight.retained_preflight_bytes,
        preflight.maximum_batch_decoded_bytes,
        budgets,
    )?;
    Ok(preflight)
}

/// Fully decode and validate borrowed canonical cell-patch edge Arrow bytes.
pub fn validate_cell_patch_edge_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeArrowPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, CellPatchArrowProfile::Edge, link)?;
    let preflight = preflight_cell_patch_edge_table_arrow_bytes(bytes, link, budgets)?;
    validate_identity(
        preflight.content_digest(),
        preflight.encoded_byte_len(),
        record,
    )?;
    decode_arrow(
        Cursor::new(bytes),
        CellPatchArrowProfile::Edge,
        link,
        preflight.retained_preflight_bytes,
        preflight.maximum_batch_decoded_bytes,
        budgets,
    )?;
    Ok(preflight)
}

/// Fully decode one managed cell-patch assignment Arrow artifact inside one integrity envelope.
pub fn validate_cell_patch_assignment_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentArrowPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        validate_managed(
            reader,
            record,
            CellPatchArrowProfile::Assignment,
            link,
            budgets,
        )
        .map(CellPatchAssignmentArrowPreflight::from_common)
    })
}

/// Fully decode one managed cell-patch edge Arrow artifact inside one integrity envelope.
pub fn validate_cell_patch_edge_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeArrowPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        validate_managed(reader, record, CellPatchArrowProfile::Edge, link, budgets)
            .map(CellPatchEdgeArrowPreflight::from_common)
    })
}

/// Verify borrowed assignment Arrow bytes and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_assignment_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, MultiscaleColumnarError> {
    validate_cell_patch_assignment_table_arrow_bytes(bytes, record, link, budgets)?;
    VerifiedCellPatchAssignmentArtifact::new(record.id(), link, graph)
}

/// Verify a managed assignment Arrow artifact and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_assignment_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_cell_patch_assignment_table_arrow_from_store(store, record, link, budgets)?;
    VerifiedCellPatchAssignmentArtifact::new(record.id(), link, graph)
        .map_err(VerifiedReaderError::Callback)
}

/// Verify borrowed edge Arrow bytes and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_edge_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, MultiscaleColumnarError> {
    validate_cell_patch_edge_table_arrow_bytes(bytes, record, link, budgets)?;
    VerifiedCellPatchEdgeArtifact::new(record.id(), link, graph)
}

/// Verify a managed edge Arrow artifact and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_edge_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_cell_patch_edge_table_arrow_from_store(store, record, link, budgets)?;
    VerifiedCellPatchEdgeArtifact::new(record.id(), link, graph)
        .map_err(VerifiedReaderError::Callback)
}

fn validate_managed<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CommonArrowPreflight, MultiscaleColumnarError> {
    let encoded_len = reader
        .seek(std::io::SeekFrom::End(0))
        .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, profile, link)?;
    let common = preflight_reader(reader, record.content().digest(), profile, link, budgets)?;
    validate_identity(common.content_digest, common.encoded_byte_len, record)?;
    decode_arrow(
        reader,
        profile,
        link,
        common.retained_preflight_bytes,
        common.maximum_batch_decoded_bytes,
        budgets,
    )?;
    Ok(common)
}

fn decode_arrow<R: Read + Seek>(
    source: R,
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
    retained_preflight_bytes: usize,
    maximum_batch_decoded_bytes: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    let retained = retained_preflight_bytes
        .checked_add(maximum_batch_decoded_bytes)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let schema = expected_schema(profile, link)?;
    let mut reader = FileReaderBuilder::new()
        .with_max_footer_fb_depth(8)
        .with_max_footer_fb_tables(32)
        .build(source)
        .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
    if reader.schema().as_ref() != &schema || !reader.custom_metadata().is_empty() {
        return Err(arrow_failure(SpatialArrowFailure::StockDecode));
    }
    let mut global_row = 0_usize;
    for decoded in &mut reader {
        let batch = decoded.map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        if batch.num_columns() != profile.field_count() {
            return Err(arrow_failure(SpatialArrowFailure::StockDecode));
        }
        for column in batch.columns() {
            column
                .to_data()
                .validate_full()
                .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        }
        match profile {
            CellPatchArrowProfile::Assignment => {
                decode_assignment_batch(&batch, link, &mut global_row)?
            }
            CellPatchArrowProfile::Edge => decode_edge_batch(&batch, link, &mut global_row)?,
        }
    }
    if global_row != profile.row_count(link) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    Ok(())
}

fn decode_assignment_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), MultiscaleColumnarError> {
    let cells = string_column(batch, 0)?;
    let statuses = string_column(batch, 1)?;
    let x = u64_column(batch, 2)?;
    let y = u64_column(batch, 3)?;
    let starts = u64_column(batch, 4)?;
    let counts = u64_column(batch, 5)?;
    if [
        cells.null_count(),
        statuses.null_count(),
        x.null_count(),
        y.null_count(),
        starts.null_count(),
        counts.null_count(),
    ]
    .iter()
    .any(|count| *count != 0)
    {
        return Err(arrow_failure(SpatialArrowFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let row = link
            .assignments()
            .get(*global_row)
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidRowCount))?;
        if cells.value(local) != row.cell_id().as_str()
            || statuses.value(local) != row.status().wire_name()
            || x.value(local) != row.anchor_px()[0].to_bits()
            || y.value(local) != row.anchor_px()[1].to_bits()
            || starts.value(local) != row.edge_start()
            || counts.value(local) != row.edge_count()
        {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        *global_row = global_row
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn decode_edge_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), MultiscaleColumnarError> {
    let assignment_rows = u64_column(batch, 0)?;
    let patches = string_column(batch, 1)?;
    let numerators = u64_column(batch, 2)?;
    let denominators = u64_column(batch, 3)?;
    if assignment_rows.null_count() != 0 || patches.null_count() != 0 {
        return Err(arrow_failure(SpatialArrowFailure::StockDecode));
    }
    let expected_nulls = match link.mode() {
        CellPatchAssignmentMode::ContainedShared => batch.num_rows(),
        CellPatchAssignmentMode::DeclaredWeightedInterpolation => 0,
    };
    if numerators.null_count() != expected_nulls || denominators.null_count() != expected_nulls {
        return Err(arrow_failure(SpatialArrowFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let row = link
            .edges()
            .get(*global_row)
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidRowCount))?;
        let numerator = row.weight().map_or(0, |weight| weight.numerator());
        let denominator = row.weight().map_or(0, |weight| weight.denominator());
        if assignment_rows.value(local) != row.assignment_row()
            || patches.value(local) != row.patch_id().as_str()
            || numerators.value(local) != numerator
            || denominators.value(local) != denominator
            || numerators.is_null(local) != row.weight().is_none()
            || denominators.is_null(local) != row.weight().is_none()
        {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        *global_row = global_row
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
) -> Result<(), MultiscaleColumnarError> {
    let row_count = u64::try_from(profile.row_count(link))
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if !record_matches_encoding(
        record,
        profile.role(),
        SpatialPhysicalEncoding::Arrow,
        row_count,
    ) || record.dependencies() != cell_patch_dependencies(link)
        || record.content().byte_len() != encoded_len
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    digest: marklab_project::ContentDigest,
    encoded_len: u64,
    record: &ArtifactRecord,
) -> Result<(), MultiscaleColumnarError> {
    if digest != record.content().digest() || encoded_len != record.content().byte_len() {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn string_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&StringArray, MultiscaleColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))
}

fn u64_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&UInt64Array, MultiscaleColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))
}
