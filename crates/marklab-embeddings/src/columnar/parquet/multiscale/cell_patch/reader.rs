use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::array::{Array, RecordBatchReader, StringArray, UInt64Array};
use bytes::Bytes;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};
use parquet::{
    arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    },
    file::metadata::RowGroupMetaData,
};

use crate::{
    CellPatchAssignmentMode, CellPatchLink, VerifiedCellPatchAssignmentArtifact,
    VerifiedCellPatchEdgeArtifact, VerifiedCellPatchInputArtifactGraph,
};

use super::{
    super::super::physical::validated_stock_group_range,
    preflight::{
        parquet_failure, prepare_reader, CellPatchAssignmentParquetPreflight,
        CellPatchEdgeParquetPreflight, CommonParquetPreflight,
    },
    profile::{expected_schema, CellPatchParquetProfile},
};
use crate::{
    columnar::{
        multiscale::{
            assignment_decoded_bytes, cell_patch_dependencies, edge_decoded_bytes,
            enforce_file_budget, enforce_retained_budget, enforce_row_group_budget,
        },
        parquet::{profile::ROW_GROUP_ROWS, reader::RowGroupWindow},
        ColumnarError, EmbeddingColumnarBudgets, ParquetFailure,
    },
    multiscale::physical::{record_matches_encoding, SpatialPhysicalEncoding},
};

/// Fully decode and validate borrowed canonical cell-patch assignment Parquet bytes.
pub fn validate_cell_patch_assignment_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentParquetPreflight, ColumnarError> {
    validate_borrowed(
        bytes,
        record,
        CellPatchParquetProfile::Assignment,
        link,
        budgets,
        CellPatchAssignmentParquetPreflight::from_common,
    )
}

/// Fully decode and validate borrowed canonical cell-patch edge Parquet bytes.
pub fn validate_cell_patch_edge_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeParquetPreflight, ColumnarError> {
    validate_borrowed(
        bytes,
        record,
        CellPatchParquetProfile::Edge,
        link,
        budgets,
        CellPatchEdgeParquetPreflight::from_common,
    )
}

/// Fully decode one managed cell-patch assignment Parquet artifact in one integrity envelope.
pub fn validate_cell_patch_assignment_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentParquetPreflight, VerifiedReaderError<ColumnarError>> {
    store.with_verified_reader(record, |reader| {
        validate_managed(
            reader,
            record,
            CellPatchParquetProfile::Assignment,
            link,
            budgets,
            CellPatchAssignmentParquetPreflight::from_common,
        )
    })
}

/// Fully decode one managed cell-patch edge Parquet artifact in one integrity envelope.
pub fn validate_cell_patch_edge_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeParquetPreflight, VerifiedReaderError<ColumnarError>> {
    store.with_verified_reader(record, |reader| {
        validate_managed(
            reader,
            record,
            CellPatchParquetProfile::Edge,
            link,
            budgets,
            CellPatchEdgeParquetPreflight::from_common,
        )
    })
}

/// Verify borrowed assignment Parquet bytes and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_assignment_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, ColumnarError> {
    let context = crate::CellPatchVerifyContext { link, graph };
    crate::columnar::verify::<crate::Parquet, crate::CellPatchAssignmentEntity>(
        Cursor::new(bytes),
        record,
        &context,
        budgets,
    )
}

/// Verify a managed assignment Parquet artifact and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_assignment_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, VerifiedReaderError<ColumnarError>> {
    let context = crate::CellPatchVerifyContext { link, graph };
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::Parquet, crate::CellPatchAssignmentEntity>(&context)
    })
}

/// Verify borrowed edge Parquet bytes and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_edge_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, ColumnarError> {
    let context = crate::CellPatchVerifyContext { link, graph };
    crate::columnar::verify::<crate::Parquet, crate::CellPatchEdgeEntity>(
        Cursor::new(bytes),
        record,
        &context,
        budgets,
    )
}

/// Verify a managed edge Parquet artifact and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_edge_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, VerifiedReaderError<ColumnarError>> {
    let context = crate::CellPatchVerifyContext { link, graph };
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::Parquet, crate::CellPatchEdgeEntity>(&context)
    })
}

fn validate_borrowed<T>(
    bytes: &[u8],
    record: &ArtifactRecord,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
    summarize: fn(&CommonParquetPreflight) -> T,
) -> Result<T, ColumnarError> {
    let encoded_len = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, profile, link)?;
    let mut source = Cursor::new(bytes);
    let prepared = prepare_reader(
        &mut source,
        encoded_len,
        marklab_project::ContentDigest::from_bytes(bytes),
        profile,
        link,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = summarize(&prepared);
    decode_parquet(&mut source, profile, link, prepared, budgets)?;
    Ok(summary)
}

fn validate_managed<R: Read + Seek + ?Sized, T>(
    reader: &mut R,
    record: &ArtifactRecord,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
    summarize: fn(&CommonParquetPreflight) -> T,
) -> Result<T, ColumnarError> {
    let encoded_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, profile, link)?;
    let prepared = prepare_reader(
        reader,
        encoded_len,
        record.content().digest(),
        profile,
        link,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = summarize(&prepared);
    decode_parquet(reader, profile, link, prepared, budgets)?;
    Ok(summary)
}

fn decode_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    prepared: CommonParquetPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), ColumnarError> {
    let maximum_group_peak = maximum_group_peak(&prepared.metadata, profile, link)?;
    let retained = prepared
        .metadata
        .memory_size()
        .checked_add(maximum_group_peak)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(ColumnarError::SizeOverflow)?
        .max(prepared.retained_preflight_bytes);
    enforce_retained_budget(retained, budgets)?;

    let metadata = Arc::new(prepared.metadata);
    let schema = expected_schema(profile);
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(schema.clone())),
    )
    .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut global_row = 0_usize;
    for (group_index, group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(group.num_rows())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let (start, length) = validated_stock_group_range(group, profile.column_count())?;
        let peak = estimate_group_peak(group, profile, link, global_row)?;
        enforce_row_group_budget(peak, budgets)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ColumnarError::AllocationFailed { requested: length })?;
        bytes.resize(length, 0);
        source
            .seek(SeekFrom::Start(start))
            .and_then(|_| source.read_exact(&mut bytes))
            .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
        let end = start
            .checked_add(u64::try_from(length).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)?;
        let window = RowGroupWindow::new(start, end, Bytes::from(bytes));
        let mut reader =
            ParquetRecordBatchReaderBuilder::new_with_metadata(window, reader_metadata.clone())
                .with_row_groups(vec![group_index])
                .with_batch_size(ROW_GROUP_ROWS)
                .build()
                .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if reader.schema().as_ref() != &schema {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        let group_start = global_row;
        for decoded in &mut reader {
            let batch = decoded.map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
            validate_batch(&batch, profile, link, &mut global_row)?;
        }
        if global_row
            .checked_sub(group_start)
            .ok_or(ColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowCount));
        }
    }
    if global_row != profile.row_count(link) {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}

fn maximum_group_peak(
    metadata: &parquet::file::metadata::ParquetMetaData,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<usize, ColumnarError> {
    let mut maximum = 0_usize;
    let mut group_start = 0_usize;
    for group in metadata.row_groups() {
        maximum = maximum.max(estimate_group_peak(group, profile, link, group_start)?);
        group_start = group_start
            .checked_add(
                usize::try_from(group.num_rows())
                    .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?,
            )
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    if group_start != profile.row_count(link) {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(maximum)
}

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), ColumnarError> {
    if batch.num_columns() != profile.column_count() {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for column in batch.columns() {
        column
            .to_data()
            .validate_full()
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    }
    match profile {
        CellPatchParquetProfile::Assignment => validate_assignment_batch(batch, link, global_row),
        CellPatchParquetProfile::Edge => validate_edge_batch(batch, link, global_row),
    }
}

fn validate_assignment_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), ColumnarError> {
    visit_assignment_batch(
        batch,
        global_row,
        |index, id, status, anchor, start, count| {
            let row = link
                .assignments()
                .get(index)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?;
            if id != row.cell_id().as_str()
                || status != row.status()
                || anchor.map(f64::to_bits) != row.anchor_px().map(f64::to_bits)
                || start != row.edge_start()
                || count != row.edge_count()
            {
                return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
            }
            Ok(())
        },
    )
}

pub(in crate::columnar::parquet) fn visit_assignment_batch(
    batch: &arrow::record_batch::RecordBatch,
    global_row: &mut usize,
    mut visit: impl FnMut(
        usize,
        &str,
        crate::CellPatchAssignmentStatus,
        [f64; 2],
        u64,
        u64,
    ) -> Result<(), ColumnarError>,
) -> Result<(), ColumnarError> {
    crate::columnar::parquet::read::validate_batch_arrays(batch, 6)?;
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
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let anchor = [
            f64::from_bits(x.value(local)),
            f64::from_bits(y.value(local)),
        ];
        if anchor
            .iter()
            .any(|value| !value.is_finite() || (*value == 0.0 && value.to_bits() != 0))
        {
            return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
        }
        let status = match statuses.value(local) {
            "assigned" => crate::CellPatchAssignmentStatus::Assigned,
            "outside_sampled_support" => crate::CellPatchAssignmentStatus::OutsideSampledSupport,
            "interpolation_unavailable" => {
                crate::CellPatchAssignmentStatus::InterpolationUnavailable
            }
            _ => return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows)),
        };
        visit(
            *global_row,
            cells.value(local),
            status,
            anchor,
            starts.value(local),
            counts.value(local),
        )?;
        *global_row = global_row
            .checked_add(1)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn validate_edge_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), ColumnarError> {
    visit_edge_batch(
        batch,
        link.mode(),
        global_row,
        |index, assignment, patch, numerator, denominator| {
            let row = link
                .edges()
                .get(index)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?;
            if assignment != row.assignment_row()
                || patch != row.patch_id().as_str()
                || numerator != row.weight().map(|weight| weight.numerator())
                || denominator != row.weight().map(|weight| weight.denominator())
            {
                return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
            }
            Ok(())
        },
    )
}

pub(in crate::columnar::parquet) fn visit_edge_batch(
    batch: &arrow::record_batch::RecordBatch,
    mode: CellPatchAssignmentMode,
    global_row: &mut usize,
    mut visit: impl FnMut(usize, u64, &str, Option<u64>, Option<u64>) -> Result<(), ColumnarError>,
) -> Result<(), ColumnarError> {
    crate::columnar::parquet::read::validate_batch_arrays(batch, 4)?;
    let assignment_rows = u64_column(batch, 0)?;
    let patches = string_column(batch, 1)?;
    let numerators = u64_column(batch, 2)?;
    let denominators = u64_column(batch, 3)?;
    if assignment_rows.null_count() != 0 || patches.null_count() != 0 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    let expected_nulls = match mode {
        CellPatchAssignmentMode::ContainedShared => batch.num_rows(),
        CellPatchAssignmentMode::DeclaredWeightedInterpolation => 0,
    };
    if numerators.null_count() != expected_nulls || denominators.null_count() != expected_nulls {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let numerator = (!numerators.is_null(local)).then(|| numerators.value(local));
        let denominator = (!denominators.is_null(local)).then(|| denominators.value(local));
        if let (Some(numerator), Some(denominator)) = (numerator, denominator) {
            if numerator == 0 || denominator == 0 || numerator > denominator {
                return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
            }
        }
        visit(
            *global_row,
            assignment_rows.value(local),
            patches.value(local),
            numerator,
            denominator,
        )?;
        *global_row = global_row
            .checked_add(1)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn estimate_group_peak(
    group: &RowGroupMetaData,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    group_start: usize,
) -> Result<usize, ColumnarError> {
    let (_, encoded) = validated_stock_group_range(group, profile.column_count())?;
    let rows = usize::try_from(group.num_rows())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let group_end = group_start
        .checked_add(rows)
        .ok_or(ColumnarError::SizeOverflow)?;
    let decoded = match profile {
        CellPatchParquetProfile::Assignment => assignment_decoded_bytes(
            link.assignments()
                .get(group_start..group_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?,
        )?,
        CellPatchParquetProfile::Edge => edge_decoded_bytes(
            link.edges()
                .get(group_start..group_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?,
        )?,
    };
    encoded
        .checked_mul(2)
        .and_then(|value| value.checked_add(decoded.checked_mul(2)?))
        .and_then(|value| value.checked_add(rows.checked_mul(4)?))
        .ok_or(ColumnarError::SizeOverflow)
}

fn string_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&StringArray, ColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))
}

fn u64_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&UInt64Array, ColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))
}

pub(in crate::columnar::parquet) fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<(), ColumnarError> {
    let row_count =
        u64::try_from(profile.row_count(link)).map_err(|_| ColumnarError::SizeOverflow)?;
    if !record_matches_encoding(
        record,
        profile.role(),
        SpatialPhysicalEncoding::Parquet,
        row_count,
    ) || record.dependencies() != cell_patch_dependencies(link)
        || record.content().byte_len() != encoded_len
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    prepared: &CommonParquetPreflight,
    record: &ArtifactRecord,
) -> Result<(), ColumnarError> {
    if prepared.content_digest != record.content().digest()
        || prepared.encoded_byte_len != record.content().byte_len()
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn verify_cell_patch_assignment_table_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::CellPatchVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, ColumnarError> {
    validate_managed(
        reader,
        record,
        CellPatchParquetProfile::Assignment,
        context.link,
        budgets,
        |_| (),
    )?;
    VerifiedCellPatchAssignmentArtifact::new(record.id(), context.link, context.graph)
}

pub(crate) fn verify_cell_patch_edge_table_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::CellPatchVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, ColumnarError> {
    validate_managed(
        reader,
        record,
        CellPatchParquetProfile::Edge,
        context.link,
        budgets,
        |_| (),
    )?;
    VerifiedCellPatchEdgeArtifact::new(record.id(), context.link, context.graph)
}
