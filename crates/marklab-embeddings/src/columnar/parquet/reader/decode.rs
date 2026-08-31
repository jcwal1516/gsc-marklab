use std::{
    io::{Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::array::{Array, FixedSizeListArray, Float32Array, RecordBatchReader, StringArray};
use bytes::Bytes;
use parquet::arrow::arrow_reader::{
    ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
};

use crate::{
    table::EmbeddingQcAccumulator, CellEmbeddingRowLink, CellEmbeddingTable, EmbeddingQcSummary,
    EmbeddingStatus, ExpectedCellSet, VerifiedCellEmbeddingArtifactGraph,
};

use super::{
    super::{
        preflight::PreparedCellEmbeddingParquet,
        profile::{embedding_schema, ROW_GROUP_ROWS},
    },
    record::{map_table_construction_error, parquet_failure, parse_embedding_status},
    resources::{
        estimate_group_peak, estimate_materialized_table_bytes, estimate_maximum_group_peak,
        validated_group_range,
    },
    window::RowGroupWindow,
};
use crate::columnar::{
    enforce_retained_budget, enforce_row_group_budget, EmbeddingColumnarBudgets,
    EmbeddingColumnarError, ParquetFailure,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_cell_embedding_table_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    dimension: u32,
    declared_logical_digest: marklab_project::ContentDigest,
    prepared: PreparedCellEmbeddingParquet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, EmbeddingColumnarError> {
    let final_table_bytes = estimate_materialized_table_bytes(expected, dimension)?;
    let maximum_group_peak = estimate_maximum_group_peak(&prepared, dimension)?;
    let metadata_bytes = prepared
        .metadata
        .memory_size()
        .checked_mul(2)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let materialization_peak = final_table_bytes
        .checked_add(metadata_bytes)
        .and_then(|value| value.checked_add(maximum_group_peak))
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let required_retained = materialization_peak.max(prepared.summary.retained_preflight_bytes);
    enforce_retained_budget(required_retained, budgets)?;

    let component_count = expected
        .cells()
        .len()
        .checked_mul(usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let mut values = Vec::new();
    values.try_reserve_exact(component_count).map_err(|_| {
        EmbeddingColumnarError::AllocationFailed {
            requested: component_count.saturating_mul(size_of::<f32>()),
        }
    })?;
    let mut statuses = Vec::new();
    statuses
        .try_reserve_exact(expected.cells().len())
        .map_err(|_| EmbeddingColumnarError::AllocationFailed {
            requested: expected
                .cells()
                .len()
                .saturating_mul(size_of::<EmbeddingStatus>()),
        })?;

    visit_cell_embedding_table_parquet(
        source,
        expected,
        row_link,
        dimension,
        prepared,
        budgets,
        |_, status, vector| {
            statuses.push(status);
            values.extend_from_slice(vector);
            Ok(())
        },
    )?;
    if values.len() != component_count || statuses.len() != expected.cells().len() {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    let table = CellEmbeddingTable::from_physical_values(
        dimension,
        expected,
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_logical_digest,
        statuses,
        values,
        budgets.maximum_retained_bytes(),
    )
    .map_err(map_table_construction_error)?;
    if table.qc_summary().logical_digest() != declared_logical_digest {
        return Err(parquet_failure(ParquetFailure::LogicalDigestMismatch));
    }
    Ok(table)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn scan_cell_embedding_table_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    dimension: u32,
    declared_logical_digest: marklab_project::ContentDigest,
    prepared: PreparedCellEmbeddingParquet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EmbeddingQcSummary, EmbeddingColumnarError> {
    let maximum_group_peak = estimate_maximum_group_peak(&prepared, dimension)?;
    let metadata_bytes = prepared
        .metadata
        .memory_size()
        .checked_mul(2)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let scan_peak = metadata_bytes
        .checked_add(maximum_group_peak)
        .and_then(|value| value.checked_add(64 * 1024))
        .and_then(|value| value.checked_add(size_of::<EmbeddingQcAccumulator>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    enforce_retained_budget(
        scan_peak.max(prepared.summary.retained_preflight_bytes),
        budgets,
    )?;
    let mut accumulator = EmbeddingQcAccumulator::new(
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_logical_digest,
        dimension,
        expected.cells().len(),
    )
    .map_err(map_table_construction_error)?;
    visit_cell_embedding_table_parquet(
        source,
        expected,
        row_link,
        dimension,
        prepared,
        budgets,
        |cell_id, status, vector| {
            accumulator
                .push(
                    cell_id,
                    status,
                    (status == EmbeddingStatus::Present).then_some(vector),
                )
                .map_err(map_table_construction_error)
        },
    )?;
    let summary = accumulator.finish().map_err(map_table_construction_error)?;
    if summary.logical_digest() != declared_logical_digest {
        return Err(parquet_failure(ParquetFailure::LogicalDigestMismatch));
    }
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn visit_cell_embedding_table_parquet<R, F>(
    source: &mut R,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    dimension: u32,
    prepared: PreparedCellEmbeddingParquet,
    budgets: EmbeddingColumnarBudgets,
    mut visit: F,
) -> Result<(), EmbeddingColumnarError>
where
    R: Read + Seek + ?Sized,
    F: FnMut(&marklab_data::CellId, EmbeddingStatus, &[f32]) -> Result<(), EmbeddingColumnarError>,
{
    let expected_schema = embedding_schema(dimension)?;
    let PreparedCellEmbeddingParquet {
        summary: _,
        metadata,
    } = prepared;
    let metadata = Arc::new(metadata);
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(expected_schema.clone())),
    )
    .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut global_row = 0_usize;
    for (group_index, row_group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(row_group.num_rows())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let (start, length) = validated_group_range(row_group)?;
        let group_peak = estimate_group_peak(row_group, dimension)?;
        enforce_row_group_budget(group_peak, budgets)?;
        let mut group_bytes = Vec::new();
        group_bytes
            .try_reserve_exact(length)
            .map_err(|_| EmbeddingColumnarError::AllocationFailed { requested: length })?;
        group_bytes.resize(length, 0);
        source
            .seek(SeekFrom::Start(start))
            .and_then(|_| source.read_exact(&mut group_bytes))
            .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
        let end = start
            .checked_add(u64::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let window = RowGroupWindow::new(start, end, Bytes::from(group_bytes));
        let mut reader =
            ParquetRecordBatchReaderBuilder::new_with_metadata(window, reader_metadata.clone())
                .with_row_groups(vec![group_index])
                .with_batch_size(ROW_GROUP_ROWS)
                .build()
                .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if reader.schema().as_ref() != &expected_schema {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        let group_start_row = global_row;
        for decoded in &mut reader {
            let batch = decoded.map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
            validate_batch(
                &batch,
                expected,
                row_link,
                dimension,
                &mut global_row,
                &mut visit,
            )?;
        }
        if global_row
            .checked_sub(group_start_row)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowCount));
        }
    }
    if global_row != expected.cells().len() {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}

fn validate_batch<F>(
    batch: &arrow::record_batch::RecordBatch,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    dimension: u32,
    global_row: &mut usize,
    visit: &mut F,
) -> Result<(), EmbeddingColumnarError>
where
    F: FnMut(&marklab_data::CellId, EmbeddingStatus, &[f32]) -> Result<(), EmbeddingColumnarError>,
{
    if batch.num_columns() != 3 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for column in batch.columns() {
        column
            .to_data()
            .validate_full()
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    }
    let cells = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
    let embeddings = batch
        .column(1)
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
    let components = embeddings
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
    let status_values = batch
        .column(2)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
    if cells.null_count() != 0
        || embeddings.null_count() != 0
        || components.null_count() != 0
        || status_values.null_count() != 0
        || embeddings.value_length()
            != i32::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?
    {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    let dimension = usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    for local_row in 0..batch.num_rows() {
        let expected_cell = expected
            .cells()
            .get(*global_row)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidCellOrder))?;
        let link = row_link
            .entries()
            .get(*global_row)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidCellOrder))?;
        if cells.value(local_row) != expected_cell.as_str() || link.cell_id() != expected_cell {
            return Err(parquet_failure(ParquetFailure::InvalidCellOrder));
        }
        let status = parse_embedding_status(status_values.value(local_row))?;
        if status != link.status() {
            return Err(parquet_failure(ParquetFailure::InvalidStatus));
        }
        let start = usize::try_from(embeddings.value_offset(local_row))
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let end = start
            .checked_add(dimension)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let vector = components
            .values()
            .get(start..end)
            .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
        if vector.iter().any(|value| {
            !value.is_finite()
                || (*value == 0.0 && value.to_bits() != 0)
                || (status != EmbeddingStatus::Present && value.to_bits() != 0)
        }) {
            return Err(parquet_failure(ParquetFailure::InvalidComponent));
        }
        visit(expected_cell, status, vector)?;
        *global_row = global_row
            .checked_add(1)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok(())
}
