use std::io::{Cursor, Read, Seek};

use arrow::array::{Array, FixedSizeListArray, Float32Array, StringArray};
use arrow_ipc::reader::FileReaderBuilder;
use marklab_project::{
    ArtifactRecord, LocalArtifactStore, TableColumnType, TableFormat, TableScalarType,
    VerifiedReaderError,
};

use crate::{
    table::EmbeddingQcAccumulator, CellEmbeddingRowLink, CellEmbeddingTable, EmbeddingError,
    EmbeddingQcSummary, EmbeddingStatus, ExpectedCellSet, VerifiedCellEmbeddingArtifactGraph,
    VerifiedCellEmbeddingTableArtifact,
};

use super::{
    super::{
        estimate_materialized_table_bytes, ArrowIpcFailure, CellEmbeddingTablePhysicalBindings,
        EmbeddingColumnarBudgets, EmbeddingColumnarError,
    },
    preflight::{
        declared_table_logical_digest, preflight_cell_embedding_table_arrow_bytes,
        CellEmbeddingArrowPreflight,
    },
    preflight_reader::{
        declared_table_logical_digest_reader, preflight_cell_embedding_table_arrow_reader,
    },
    profile::{
        arrow_failure, embedding_schema, enforce_retained_budget, validate_shape, ENCODING_VERSION,
        SCHEMA_ID,
    },
};

/// Materialize a fully verified canonical Arrow table after provenance-graph validation.
pub fn read_cell_embedding_table_arrow_bytes(
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
    let manifest_dimension =
        validate_embedding_record(record, encoded_byte_len, expected, row_link, graph)?;
    let declared_logical_digest = declared_table_logical_digest(bytes, budgets)?;
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_artifact_id,
        graph.row_link_logical_digest,
        declared_logical_digest,
    )?;
    let preflight = preflight_cell_embedding_table_arrow_bytes(bytes, expected, bindings, budgets)?;
    if preflight.dimension() != manifest_dimension
        || preflight.content_digest() != record.content().digest()
        || preflight.encoded_byte_len() != record.content().byte_len()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    materialize_cell_embedding_table_arrow(
        Cursor::new(bytes),
        expected,
        row_link,
        graph,
        bindings,
        manifest_dimension,
        declared_logical_digest,
        preflight,
        budgets,
    )
}

/// Materialize a managed Arrow table through one pre/post-verified store descriptor.
pub fn read_cell_embedding_table_arrow_from_store(
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
    let manifest_dimension =
        validate_embedding_record(record, encoded_byte_len, expected, row_link, graph)
            .map_err(VerifiedReaderError::Callback)?;
    store.with_verified_reader(record, |reader| {
        let declared_logical_digest = declared_table_logical_digest_reader(reader, budgets)?;
        let bindings = CellEmbeddingTablePhysicalBindings::new(
            graph.expected_cells_artifact_id,
            graph.provenance_artifact_id,
            graph.row_link_artifact_id,
            graph.row_link_logical_digest,
            declared_logical_digest,
        )?;
        let preflight = preflight_cell_embedding_table_arrow_reader(
            reader,
            record.content().digest(),
            expected,
            bindings,
            budgets,
        )?;
        if preflight.dimension() != manifest_dimension
            || preflight.content_digest() != record.content().digest()
            || preflight.encoded_byte_len() != record.content().byte_len()
        {
            return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
        }
        materialize_cell_embedding_table_arrow(
            reader,
            expected,
            row_link,
            graph,
            bindings,
            manifest_dimension,
            declared_logical_digest,
            preflight,
            budgets,
        )
    })
}

/// Stream-scan fully preflighted borrowed Arrow bytes without retaining the table.
pub fn scan_cell_embedding_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EmbeddingQcSummary, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    let dimension = validate_embedding_record(record, encoded_byte_len, expected, row_link, graph)?;
    let declared_logical_digest = declared_table_logical_digest(bytes, budgets)?;
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_artifact_id,
        graph.row_link_logical_digest,
        declared_logical_digest,
    )?;
    let preflight = preflight_cell_embedding_table_arrow_bytes(bytes, expected, bindings, budgets)?;
    if preflight.dimension() != dimension
        || preflight.content_digest() != record.content().digest()
        || preflight.encoded_byte_len() != record.content().byte_len()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    scan_cell_embedding_table_arrow(
        Cursor::new(bytes),
        expected,
        row_link,
        graph,
        bindings,
        dimension,
        declared_logical_digest,
        preflight,
        budgets,
    )
}

/// Stream-scan a managed Arrow table through one pre/post-verified descriptor.
pub fn scan_cell_embedding_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EmbeddingQcSummary, VerifiedReaderError<EmbeddingColumnarError>> {
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
        let declared_logical_digest = declared_table_logical_digest_reader(reader, budgets)?;
        let bindings = CellEmbeddingTablePhysicalBindings::new(
            graph.expected_cells_artifact_id,
            graph.provenance_artifact_id,
            graph.row_link_artifact_id,
            graph.row_link_logical_digest,
            declared_logical_digest,
        )?;
        let preflight = preflight_cell_embedding_table_arrow_reader(
            reader,
            record.content().digest(),
            expected,
            bindings,
            budgets,
        )?;
        if preflight.dimension() != dimension
            || preflight.content_digest() != record.content().digest()
            || preflight.encoded_byte_len() != record.content().byte_len()
        {
            return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
        }
        scan_cell_embedding_table_arrow(
            reader,
            expected,
            row_link,
            graph,
            bindings,
            dimension,
            declared_logical_digest,
            preflight,
            budgets,
        )
    })
}

/// Fully verify borrowed Arrow bytes and return a receipt bound to the exact artifact record.
pub fn verify_cell_embedding_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingTableArtifact, EmbeddingColumnarError> {
    let qc_summary =
        scan_cell_embedding_table_arrow_bytes(bytes, record, expected, row_link, graph, budgets)?;
    Ok(VerifiedCellEmbeddingTableArtifact::new(
        record.id(),
        graph,
        qc_summary,
    ))
}

/// Fully verify a managed Arrow table and return an exact artifact-bound receipt.
pub fn verify_cell_embedding_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingTableArtifact, VerifiedReaderError<EmbeddingColumnarError>> {
    let qc_summary = scan_cell_embedding_table_arrow_from_store(
        store, record, expected, row_link, graph, budgets,
    )?;
    Ok(VerifiedCellEmbeddingTableArtifact::new(
        record.id(),
        graph,
        qc_summary,
    ))
}

#[allow(clippy::too_many_arguments)]
fn materialize_cell_embedding_table_arrow<R: Read + Seek>(
    source: R,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    bindings: CellEmbeddingTablePhysicalBindings,
    manifest_dimension: u32,
    declared_logical_digest: marklab_project::ContentDigest,
    preflight: CellEmbeddingArrowPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, EmbeddingColumnarError> {
    let final_retained_bytes = estimate_materialized_table_bytes(expected, manifest_dimension)?;
    let peak_retained_bytes = final_retained_bytes
        .checked_add(preflight.retained_preflight_bytes)
        .and_then(|value| value.checked_add(preflight.maximum_batch_decoded_bytes))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    enforce_retained_budget(peak_retained_bytes, budgets)?;

    let component_count = expected
        .cells()
        .len()
        .checked_mul(
            usize::try_from(manifest_dimension)
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        )
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let component_bytes = component_count
        .checked_mul(size_of::<f32>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let mut values = Vec::new();
    values.try_reserve_exact(component_count).map_err(|_| {
        EmbeddingColumnarError::AllocationFailed {
            requested: component_bytes,
        }
    })?;
    let status_bytes = expected
        .cells()
        .len()
        .checked_mul(size_of::<EmbeddingStatus>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let mut statuses = Vec::new();
    statuses
        .try_reserve_exact(expected.cells().len())
        .map_err(|_| EmbeddingColumnarError::AllocationFailed {
            requested: status_bytes,
        })?;

    visit_cell_embedding_table_arrow(
        source,
        expected,
        row_link,
        bindings,
        manifest_dimension,
        |_, status, vector| {
            statuses.push(status);
            values.extend_from_slice(vector);
            Ok(())
        },
    )?;
    if values.len() != component_count || statuses.len() != expected.cells().len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let table = CellEmbeddingTable::from_physical_values(
        manifest_dimension,
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
        return Err(arrow_failure(ArrowIpcFailure::LogicalDigestMismatch));
    }
    Ok(table)
}

#[allow(clippy::too_many_arguments)]
fn scan_cell_embedding_table_arrow<R: Read + Seek>(
    source: R,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    bindings: CellEmbeddingTablePhysicalBindings,
    dimension: u32,
    declared_logical_digest: marklab_project::ContentDigest,
    preflight: CellEmbeddingArrowPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EmbeddingQcSummary, EmbeddingColumnarError> {
    let retained = preflight
        .retained_preflight_bytes
        .checked_add(preflight.maximum_batch_decoded_bytes)
        .and_then(|value| value.checked_add(size_of::<EmbeddingQcAccumulator>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let mut accumulator = EmbeddingQcAccumulator::new(
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_logical_digest,
        dimension,
        expected.cells().len(),
    )
    .map_err(map_table_construction_error)?;
    visit_cell_embedding_table_arrow(
        source,
        expected,
        row_link,
        bindings,
        dimension,
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
        return Err(arrow_failure(ArrowIpcFailure::LogicalDigestMismatch));
    }
    Ok(summary)
}

fn visit_cell_embedding_table_arrow<R, F>(
    source: R,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    bindings: CellEmbeddingTablePhysicalBindings,
    dimension: u32,
    mut visit: F,
) -> Result<(), EmbeddingColumnarError>
where
    R: Read + Seek,
    F: FnMut(&marklab_data::CellId, EmbeddingStatus, &[f32]) -> Result<(), EmbeddingColumnarError>,
{
    let mut reader = FileReaderBuilder::new()
        .with_max_footer_fb_depth(8)
        .with_max_footer_fb_tables(32)
        .build(source)
        .map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
    if reader.schema().as_ref() != &embedding_schema(dimension, bindings)?
        || !reader.custom_metadata().is_empty()
    {
        return Err(arrow_failure(ArrowIpcFailure::StockDecode));
    }
    let mut global_row = 0_usize;
    for decoded in &mut reader {
        let batch = decoded.map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
        if batch.num_columns() != 3 {
            return Err(arrow_failure(ArrowIpcFailure::StockDecode));
        }
        for column in batch.columns() {
            column
                .to_data()
                .validate_full()
                .map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
        }
        let cells = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        let embeddings = batch
            .column(1)
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        let components = embeddings
            .values()
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        let status_values = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        if cells.null_count() != 0
            || embeddings.null_count() != 0
            || components.null_count() != 0
            || status_values.null_count() != 0
            || embeddings.value_length()
                != i32::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?
        {
            return Err(arrow_failure(ArrowIpcFailure::StockDecode));
        }
        for local_row in 0..batch.num_rows() {
            let expected_cell = expected
                .cells()
                .get(global_row)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidCellOrder))?;
            let link = row_link
                .entries()
                .get(global_row)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidCellOrder))?;
            if cells.value(local_row) != expected_cell.as_str() || link.cell_id() != expected_cell {
                return Err(arrow_failure(ArrowIpcFailure::InvalidCellOrder));
            }
            let status = parse_embedding_status(status_values.value(local_row))?;
            if status != link.status() {
                return Err(arrow_failure(ArrowIpcFailure::InvalidStatus));
            }
            let start = usize::try_from(embeddings.value_offset(local_row))
                .map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
            let end = start
                .checked_add(
                    usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                )
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let vector = components
                .values()
                .get(start..end)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
            if vector.iter().any(|value| {
                !value.is_finite()
                    || (*value == 0.0 && value.to_bits() != 0)
                    || (status != EmbeddingStatus::Present && value.to_bits() != 0)
            }) {
                return Err(arrow_failure(ArrowIpcFailure::InvalidComponent));
            }
            visit(expected_cell, status, vector)?;
            global_row = global_row
                .checked_add(1)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
    }
    if global_row != expected.cells().len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
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
    if record.content().kind() != "application/vnd.marklab.cell-embedding-table.v1+arrow"
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
    if manifest.format() != TableFormat::ArrowIpcFile
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
    if dimension != graph.output_dimension {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    validate_shape(expected.cells().len(), dimension)?;
    Ok(dimension)
}

fn parse_embedding_status(value: &str) -> Result<EmbeddingStatus, EmbeddingColumnarError> {
    match value {
        "present" => Ok(EmbeddingStatus::Present),
        "missing_vector" => Ok(EmbeddingStatus::MissingVector),
        "extraction_failed" => Ok(EmbeddingStatus::ExtractionFailed),
        "qc_rejected" => Ok(EmbeddingStatus::QcRejected),
        _ => Err(arrow_failure(ArrowIpcFailure::InvalidStatus)),
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
        _ => arrow_failure(ArrowIpcFailure::InvalidComponent),
    }
}
