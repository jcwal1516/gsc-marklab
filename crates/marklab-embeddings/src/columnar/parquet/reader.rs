use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::array::{Array, FixedSizeListArray, Float32Array, RecordBatchReader, StringArray};
use bytes::Bytes;
use marklab_project::{
    ArtifactRecord, LocalArtifactStore, TableColumnType, TableFormat, TableScalarType,
    VerifiedReaderError,
};
use parquet::{
    arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    },
    errors::{ParquetError, Result as ParquetResult},
    file::{
        metadata::RowGroupMetaData,
        reader::{ChunkReader, Length},
    },
};

use crate::{
    CellEmbeddingRowLink, CellEmbeddingTable, EmbeddingError, EmbeddingStatus, ExpectedCellSet,
    VerifiedCellEmbeddingArtifactGraph,
};

use super::{
    super::{
        CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets, EmbeddingColumnarError,
        ParquetFailure,
    },
    preflight::{
        declared_table_logical_digest_parquet_reader, prepare_cell_embedding_table_parquet_reader,
        PreparedCellEmbeddingParquet,
    },
    profile::{
        embedding_schema, CONTENT_KIND, ENCODING_VERSION, MAXIMUM_DIMENSION, MAXIMUM_ROWS,
        ROW_GROUP_ROWS, SCHEMA_ID,
    },
};

/// Materialize fully preflighted borrowed Parquet bytes after provenance-graph validation.
pub fn read_cell_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    let dimension = validate_embedding_record(record, encoded_byte_len, expected, row_link, graph)?;
    let mut source = Cursor::new(bytes);
    let declared_logical_digest = declared_table_logical_digest_parquet_reader(
        &mut source,
        encoded_byte_len,
        expected,
        budgets,
    )?;
    let bindings = physical_bindings(graph, declared_logical_digest)?;
    let prepared = prepare_cell_embedding_table_parquet_reader(
        &mut source,
        encoded_byte_len,
        marklab_project::ContentDigest::from_bytes(bytes),
        expected,
        dimension,
        bindings,
        budgets,
    )?;
    validate_preflight_identity(&prepared, record, dimension)?;
    materialize_cell_embedding_table_parquet(
        &mut source,
        expected,
        row_link,
        graph,
        dimension,
        declared_logical_digest,
        prepared,
        budgets,
    )
}

/// Materialize a managed Parquet table through one pre/post-verified store descriptor.
pub fn read_cell_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, VerifiedReaderError<EmbeddingColumnarError>> {
    let encoded_byte_len = record.content().byte_len();
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded {
                observed: encoded_byte_len,
                maximum: budgets.maximum_file_bytes(),
            },
        ));
    }
    let dimension = validate_embedding_record(record, encoded_byte_len, expected, row_link, graph)
        .map_err(VerifiedReaderError::Callback)?;
    store.with_verified_reader(record, |reader| {
        let declared_logical_digest = declared_table_logical_digest_parquet_reader(
            reader,
            encoded_byte_len,
            expected,
            budgets,
        )?;
        let bindings = physical_bindings(graph, declared_logical_digest)?;
        let prepared = prepare_cell_embedding_table_parquet_reader(
            reader,
            encoded_byte_len,
            record.content().digest(),
            expected,
            dimension,
            bindings,
            budgets,
        )?;
        validate_preflight_identity(&prepared, record, dimension)?;
        materialize_cell_embedding_table_parquet(
            reader,
            expected,
            row_link,
            graph,
            dimension,
            declared_logical_digest,
            prepared,
            budgets,
        )
    })
}

fn physical_bindings(
    graph: VerifiedCellEmbeddingArtifactGraph,
    declared_logical_digest: marklab_project::ContentDigest,
) -> Result<CellEmbeddingTablePhysicalBindings, EmbeddingColumnarError> {
    CellEmbeddingTablePhysicalBindings::new(
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_artifact_id,
        graph.row_link_logical_digest,
        declared_logical_digest,
    )
}

fn validate_preflight_identity(
    prepared: &PreparedCellEmbeddingParquet,
    record: &ArtifactRecord,
    dimension: u32,
) -> Result<(), EmbeddingColumnarError> {
    if prepared.summary.dimension() != dimension
        || prepared.summary.content_digest() != record.content().digest()
        || prepared.summary.encoded_byte_len() != record.content().byte_len()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn materialize_cell_embedding_table_parquet<R: Read + Seek + ?Sized>(
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
        let window = RowGroupWindow {
            base: start,
            end,
            bytes: Bytes::from(group_bytes),
        };
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
                &mut statuses,
                &mut values,
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
    if global_row != expected.cells().len()
        || values.len() != component_count
        || statuses.len() != expected.cells().len()
    {
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

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    dimension: u32,
    global_row: &mut usize,
    statuses: &mut Vec<EmbeddingStatus>,
    values: &mut Vec<f32>,
) -> Result<(), EmbeddingColumnarError> {
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
        if end > components.len() {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        for index in start..end {
            let value = components.value(index);
            if !value.is_finite()
                || (value == 0.0 && value.to_bits() != 0)
                || (status != EmbeddingStatus::Present && value.to_bits() != 0)
            {
                return Err(parquet_failure(ParquetFailure::InvalidComponent));
            }
            values.push(value);
        }
        statuses.push(status);
        *global_row = global_row
            .checked_add(1)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn validate_embedding_record(
    record: &ArtifactRecord,
    observed_byte_len: u64,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
) -> Result<u32, EmbeddingColumnarError> {
    if record.content().kind() != CONTENT_KIND
        || record.content().byte_len() != observed_byte_len
        || record.schema().id() != SCHEMA_ID
        || record.schema().version() != 1
        || !record.semantic_metadata().is_empty()
        || graph.dependency_count != 13
        || graph.expected_cells_logical_digest != expected.logical_digest()
        || graph.row_link_logical_digest != row_link.logical_digest()
        || graph.expected_cells_artifact_id != row_link.expected_cells_artifact_id()
        || graph.source_cells_artifact_id != row_link.source_cells_artifact_id()
        || graph.source_vectors_artifact_id != row_link.source_vectors_artifact_id()
        || graph.identity_map_artifact_id != row_link.identity_map_artifact_id()
        || graph.converter_artifact_id != row_link.converter_artifact_id()
        || row_link.expected_cells_logical_digest() != expected.logical_digest()
        || row_link.entries().len() != expected.cells().len()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let mut expected_dependencies = vec![
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_artifact_id,
    ];
    expected_dependencies.sort_unstable();
    if record.dependencies() != expected_dependencies {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let manifest = record
        .table()
        .ok_or(EmbeddingColumnarError::ArtifactBindingMismatch)?;
    if manifest.format() != TableFormat::ParquetFile
        || manifest.encoding_version() != ENCODING_VERSION
        || manifest.row_count()
            != u64::try_from(expected.cells().len())
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?
        || manifest.columns().len() != 3
        || manifest.columns()[0].name() != "cell_id"
        || manifest.columns()[0].column_type() != &TableColumnType::Scalar(TableScalarType::Utf8)
        || manifest.columns()[0].nullable()
        || manifest.columns()[1].name() != "embedding"
        || manifest.columns()[1].nullable()
        || manifest.columns()[2].name() != "embedding_status"
        || manifest.columns()[2].column_type() != &TableColumnType::Scalar(TableScalarType::Utf8)
        || manifest.columns()[2].nullable()
        || manifest.primary_key() != ["cell_id"]
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let dimension = match manifest.columns()[1].column_type() {
        TableColumnType::FixedSizeList {
            element: TableScalarType::F32,
            length,
        } => *length,
        _ => return Err(EmbeddingColumnarError::ArtifactBindingMismatch),
    };
    if dimension == 0 || dimension > MAXIMUM_DIMENSION || expected.cells().len() > MAXIMUM_ROWS {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(dimension)
}

fn validated_group_range(
    row_group: &RowGroupMetaData,
) -> Result<(u64, usize), EmbeddingColumnarError> {
    if row_group.num_columns() != 3 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    let start = u64::try_from(row_group.column(0).data_page_offset())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut next = start;
    for column in row_group.columns() {
        let offset = u64::try_from(column.data_page_offset())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let length = u64::try_from(column.compressed_size())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if offset != next || length == 0 {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        next = next
            .checked_add(length)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok((
        start,
        usize::try_from(
            next.checked_sub(start)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?,
        )
        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
    ))
}

fn estimate_maximum_group_peak(
    prepared: &PreparedCellEmbeddingParquet,
    dimension: u32,
) -> Result<usize, EmbeddingColumnarError> {
    prepared
        .metadata
        .row_groups()
        .iter()
        .try_fold(0_usize, |maximum, group| {
            estimate_group_peak(group, dimension).map(|required| maximum.max(required))
        })
}

fn estimate_group_peak(
    row_group: &RowGroupMetaData,
    dimension: u32,
) -> Result<usize, EmbeddingColumnarError> {
    let (_, encoded_bytes) = validated_group_range(row_group)?;
    let rows = usize::try_from(row_group.num_rows())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let components = rows
        .checked_mul(usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let arrow_output = components
        .checked_mul(size_of::<f32>())
        .and_then(|value| value.checked_add(encoded_bytes))
        .and_then(|value| {
            value.checked_add(
                rows.checked_add(1)?
                    .checked_mul(size_of::<i32>())?
                    .checked_mul(2)?,
            )
        })
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    encoded_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(arrow_output.checked_mul(2)?))
        .and_then(|value| value.checked_add(components.checked_mul(4)?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

fn estimate_materialized_table_bytes(
    expected: &ExpectedCellSet,
    dimension: u32,
) -> Result<usize, EmbeddingColumnarError> {
    let component_bytes = expected
        .cells()
        .len()
        .checked_mul(usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .and_then(|value| value.checked_mul(size_of::<f32>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let row_bytes = expected
        .cells()
        .len()
        .checked_mul(size_of::<marklab_data::CellId>() + size_of::<EmbeddingStatus>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(cell.as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    component_bytes
        .checked_add(row_bytes)
        .and_then(|value| value.checked_add(identifier_bytes))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

fn parse_embedding_status(value: &str) -> Result<EmbeddingStatus, EmbeddingColumnarError> {
    match value {
        "present" => Ok(EmbeddingStatus::Present),
        "missing_vector" => Ok(EmbeddingStatus::MissingVector),
        "extraction_failed" => Ok(EmbeddingStatus::ExtractionFailed),
        "qc_rejected" => Ok(EmbeddingStatus::QcRejected),
        _ => Err(parquet_failure(ParquetFailure::InvalidStatus)),
    }
}

fn map_table_construction_error(error: EmbeddingError) -> EmbeddingColumnarError {
    match error {
        EmbeddingError::SizeOverflow => EmbeddingColumnarError::SizeOverflow,
        EmbeddingError::RetainedByteBudgetExceeded { required, maximum } => {
            EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum }
        }
        EmbeddingError::AllocationFailed { requested } => {
            EmbeddingColumnarError::AllocationFailed { requested }
        }
        _ => parquet_failure(ParquetFailure::InvalidComponent),
    }
}

fn enforce_retained_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

fn enforce_row_group_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_row_group_bytes() {
        return Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum: budgets.maximum_row_group_bytes(),
        });
    }
    Ok(())
}

fn parquet_failure(reason: ParquetFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Parquet { reason }
}

#[derive(Clone)]
pub(super) struct RowGroupWindow {
    base: u64,
    end: u64,
    bytes: Bytes,
}

impl RowGroupWindow {
    pub(super) fn new(base: u64, end: u64, bytes: Bytes) -> Self {
        Self { base, end, bytes }
    }
}

impl Length for RowGroupWindow {
    fn len(&self) -> u64 {
        self.end
    }
}

impl ChunkReader for RowGroupWindow {
    type T = Cursor<Bytes>;

    fn get_read(&self, start: u64) -> ParquetResult<Self::T> {
        let relative = checked_relative(self.base, self.end, start, 0)?;
        Ok(Cursor::new(self.bytes.slice(relative..)))
    }

    fn get_bytes(&self, start: u64, length: usize) -> ParquetResult<Bytes> {
        let relative = checked_relative(self.base, self.end, start, length)?;
        let end = relative
            .checked_add(length)
            .ok_or_else(bounded_range_error)?;
        Ok(self.bytes.slice(relative..end))
    }
}

fn checked_relative(base: u64, end: u64, start: u64, length: usize) -> ParquetResult<usize> {
    let requested_end = start
        .checked_add(u64::try_from(length).map_err(|_| bounded_range_error())?)
        .ok_or_else(bounded_range_error)?;
    if start < base || requested_end > end {
        return Err(bounded_range_error());
    }
    usize::try_from(start - base).map_err(|_| bounded_range_error())
}

fn bounded_range_error() -> ParquetError {
    ParquetError::General("bounded Parquet row-group range rejected".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> RowGroupWindow {
        RowGroupWindow {
            base: 10,
            end: 14,
            bytes: Bytes::from_static(&[1, 2, 3, 4]),
        }
    }

    #[test]
    fn row_group_window_rejects_every_out_of_range_translation() {
        let window = window();
        assert!(window.get_read(9).is_err());
        assert!(window.get_read(15).is_err());
        assert!(window.get_bytes(9, 1).is_err());
        assert!(window.get_bytes(13, 2).is_err());
        assert!(window.get_bytes(u64::MAX, 2).is_err());

        let mut at_end = window.get_read(14).expect("exact empty end");
        let mut empty = Vec::new();
        at_end.read_to_end(&mut empty).expect("read empty end");
        assert!(empty.is_empty());
        assert_eq!(
            window.get_bytes(11, 2).expect("translated bytes"),
            Bytes::from_static(&[2, 3])
        );
    }
}
