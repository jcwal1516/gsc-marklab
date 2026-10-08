use std::{
    io::{Read, Seek, SeekFrom},
    str::FromStr,
};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::ParquetMetaDataReader,
    format::{
        ColumnChunk, ColumnMetaData, ColumnOrder, Encoding, FileMetaData, KeyValue,
        PageEncodingStats, RowGroup, SchemaElement,
    },
    thrift::TSerializable,
};

use crate::{
    columnar::{
        enforce_retained_budget, CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets,
        EmbeddingColumnarError, ParquetFailure,
    },
    ExpectedCellSet,
};

use super::super::{
    compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
    profile::{
        CREATED_BY, MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_DIMENSION, MAXIMUM_FOOTER_BYTES,
        MAXIMUM_PAGE_HEADER_BYTES, MAXIMUM_ROWS, METADATA_KEYS, PARQUET_MAGIC, ROW_GROUP_ROWS,
        TRAILER_BYTES,
    },
};
use super::{
    file_metadata::validate_file_metadata, pages::validate_pages, parquet_failure,
    CellEmbeddingParquetPreflight, PreparedCellEmbeddingParquet,
};

pub(in crate::columnar::parquet) fn preflight_cell_embedding_table_parquet_reader<
    R: Read + Seek + ?Sized,
>(
    reader: &mut R,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    expected: &ExpectedCellSet,
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingParquetPreflight, EmbeddingColumnarError> {
    prepare_cell_embedding_table_parquet_reader(
        reader,
        encoded_byte_len,
        content_digest,
        expected,
        dimension,
        bindings,
        budgets,
    )
    .map(|prepared| prepared.summary)
}

pub(in crate::columnar::parquet) fn prepare_cell_embedding_table_parquet_reader<
    R: Read + Seek + ?Sized,
>(
    reader: &mut R,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    expected: &ExpectedCellSet,
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PreparedCellEmbeddingParquet, EmbeddingColumnarError> {
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    if dimension == 0 || dimension > MAXIMUM_DIMENSION || expected.cells().len() > MAXIMUM_ROWS {
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    if encoded_byte_len
        < u64::try_from(PARQUET_MAGIC.len() + TRAILER_BYTES)
            .map_err(|_| EmbeddingColumnarError::SizeOverflow)?
    {
        return Err(parquet_failure(ParquetFailure::InvalidMagic));
    }
    let actual_length = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
    if actual_length != encoded_byte_len {
        return Err(parquet_failure(ParquetFailure::ArtifactRead));
    }
    let mut leading_magic = [0_u8; 4];
    read_exact_at(reader, 0, &mut leading_magic)?;
    let mut trailer = [0_u8; TRAILER_BYTES];
    read_exact_at(
        reader,
        encoded_byte_len
            .checked_sub(
                u64::try_from(TRAILER_BYTES).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?,
        &mut trailer,
    )?;
    if &leading_magic != PARQUET_MAGIC || trailer.get(size_of::<u32>()..) != Some(PARQUET_MAGIC) {
        return Err(parquet_failure(ParquetFailure::InvalidMagic));
    }
    let footer_length_offset = usize::try_from(encoded_byte_len)
        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?
        .checked_sub(TRAILER_BYTES)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?;
    let footer_length = u32::from_le_bytes(
        trailer
            .get(..size_of::<u32>())
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?
            .try_into()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidFooterLength))?,
    );
    let footer_length =
        usize::try_from(footer_length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if footer_length == 0 || footer_length > MAXIMUM_FOOTER_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    let footer_start = footer_length_offset
        .checked_sub(footer_length)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?;
    if footer_start < PARQUET_MAGIC.len() {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    let expected_row_groups = expected.cells().len().div_ceil(ROW_GROUP_ROWS);
    let limits = footer_compact_limits(expected_row_groups)?;
    let raw_retained_bytes = estimate_raw_preflight_bytes(footer_length, limits)?;
    let stock_retained_bytes = estimate_stock_metadata_bytes(expected_row_groups)?;
    let mut retained_preflight_bytes = raw_retained_bytes.max(stock_retained_bytes);
    enforce_retained_budget(retained_preflight_bytes, budgets)?;
    let mut footer_bytes = Vec::new();
    footer_bytes.try_reserve_exact(footer_length).map_err(|_| {
        EmbeddingColumnarError::AllocationFailed {
            requested: footer_length,
        }
    })?;
    footer_bytes.resize(footer_length, 0);
    read_exact_at(
        reader,
        u64::try_from(footer_start).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        &mut footer_bytes,
    )?;
    let mut protocol = BoundedCompactProtocol::new(&footer_bytes, limits);
    let metadata = FileMetaData::read_from_in_protocol(&mut protocol)
        .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?;
    if protocol
        .consumed_bytes()
        .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?
        != footer_bytes.len()
    {
        return Err(parquet_failure(ParquetFailure::InvalidFooter));
    }
    if !is_canonical_compact(&metadata, &footer_bytes)
        .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?
    {
        return Err(parquet_failure(ParquetFailure::InvalidFooter));
    }
    validate_file_metadata(
        &metadata,
        footer_start,
        expected,
        dimension,
        bindings,
        budgets,
        |start, end, column_index, rows, dimension| {
            let chunk_bytes = end
                .checked_sub(start)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let peak = raw_retained_bytes
                .checked_add(chunk_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            retained_preflight_bytes = retained_preflight_bytes.max(peak);
            enforce_retained_budget(peak, budgets)?;
            let mut chunk = Vec::new();
            chunk.try_reserve_exact(chunk_bytes).map_err(|_| {
                EmbeddingColumnarError::AllocationFailed {
                    requested: chunk_bytes,
                }
            })?;
            chunk.resize(chunk_bytes, 0);
            read_exact_at(
                reader,
                u64::try_from(start).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                &mut chunk,
            )?;
            validate_pages(&chunk, start, start, end, column_index, rows, dimension)
        },
    )?;
    let row_group_count = metadata.row_groups.len();
    drop(metadata);
    let stock_metadata = ParquetMetaDataReader::decode_metadata(&footer_bytes)
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    if stock_metadata.memory_size() > stock_retained_bytes
        || stock_metadata.file_metadata().version() != 2
        || usize::try_from(stock_metadata.file_metadata().num_rows()).ok()
            != Some(expected.cells().len())
        || stock_metadata.num_row_groups() != expected_row_groups
        || stock_metadata.file_metadata().created_by() != Some(CREATED_BY)
        || stock_metadata.column_index().is_some()
        || stock_metadata.offset_index().is_some()
    {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    Ok(PreparedCellEmbeddingParquet {
        summary: CellEmbeddingParquetPreflight {
            row_count: u64::try_from(expected.cells().len())
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            dimension,
            row_group_count: u32::try_from(row_group_count)
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            encoded_byte_len,
            content_digest,
            retained_preflight_bytes,
        },
        metadata: stock_metadata,
    })
}

fn read_exact_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    offset: u64,
    buffer: &mut [u8],
) -> Result<(), EmbeddingColumnarError> {
    reader
        .seek(SeekFrom::Start(offset))
        .and_then(|_| reader.read_exact(buffer))
        .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))
}

pub(in crate::columnar::parquet) fn declared_table_logical_digest_parquet_reader<
    R: Read + Seek + ?Sized,
>(
    reader: &mut R,
    encoded_byte_len: u64,
    expected: &ExpectedCellSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ContentDigest, EmbeddingColumnarError> {
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    let minimum = u64::try_from(PARQUET_MAGIC.len() + TRAILER_BYTES)
        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len < minimum {
        return Err(parquet_failure(ParquetFailure::InvalidMagic));
    }
    let actual_length = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
    if actual_length != encoded_byte_len {
        return Err(parquet_failure(ParquetFailure::ArtifactRead));
    }
    let mut leading_magic = [0_u8; 4];
    read_exact_at(reader, 0, &mut leading_magic)?;
    let mut trailer = [0_u8; TRAILER_BYTES];
    read_exact_at(
        reader,
        encoded_byte_len
            .checked_sub(
                u64::try_from(TRAILER_BYTES).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?,
        &mut trailer,
    )?;
    if &leading_magic != PARQUET_MAGIC || trailer.get(size_of::<u32>()..) != Some(PARQUET_MAGIC) {
        return Err(parquet_failure(ParquetFailure::InvalidMagic));
    }
    let footer_length = usize::try_from(u32::from_le_bytes(
        trailer[..size_of::<u32>()]
            .try_into()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidFooterLength))?,
    ))
    .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if footer_length == 0 || footer_length > MAXIMUM_FOOTER_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    let footer_length_offset = usize::try_from(encoded_byte_len)
        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?
        .checked_sub(TRAILER_BYTES)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?;
    let footer_start = footer_length_offset
        .checked_sub(footer_length)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?;
    if footer_start < PARQUET_MAGIC.len() {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    let limits = footer_compact_limits(expected.cells().len().div_ceil(ROW_GROUP_ROWS))?;
    enforce_retained_budget(
        estimate_raw_preflight_bytes(footer_length, limits)?,
        budgets,
    )?;
    let mut footer = Vec::new();
    footer.try_reserve_exact(footer_length).map_err(|_| {
        EmbeddingColumnarError::AllocationFailed {
            requested: footer_length,
        }
    })?;
    footer.resize(footer_length, 0);
    read_exact_at(
        reader,
        u64::try_from(footer_start).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        &mut footer,
    )?;
    let mut protocol = BoundedCompactProtocol::new(&footer, limits);
    let metadata = FileMetaData::read_from_in_protocol(&mut protocol)
        .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?;
    if protocol
        .consumed_bytes()
        .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?
        != footer.len()
        || !is_canonical_compact(&metadata, &footer)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?
    {
        return Err(parquet_failure(ParquetFailure::InvalidFooter));
    }
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    if entries.len() != METADATA_KEYS.len()
        || entries
            .iter()
            .zip(METADATA_KEYS)
            .any(|(entry, key)| entry.key != key || entry.value.is_none())
    {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let declared = entries[2]
        .value
        .as_deref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    ContentDigest::from_str(declared).map_err(|_| parquet_failure(ParquetFailure::InvalidMetadata))
}

fn footer_compact_limits(
    expected_row_groups: usize,
) -> Result<CompactLimits, EmbeddingColumnarError> {
    footer_compact_limits_with_metadata(expected_row_groups, METADATA_KEYS.len())
}

pub(in crate::columnar::parquet) fn footer_compact_limits_with_metadata(
    expected_row_groups: usize,
    metadata_entries: usize,
) -> Result<CompactLimits, EmbeddingColumnarError> {
    let maximum_collection_elements = expected_row_groups.max(metadata_entries).max(6);
    let maximum_total_elements = expected_row_groups
        .checked_mul(18)
        .and_then(|value| value.checked_add(16))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let maximum_fields = expected_row_groups
        .checked_mul(256)
        .and_then(|value| value.checked_add(256))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    Ok(CompactLimits {
        maximum_depth: 16,
        maximum_fields,
        maximum_collection_elements,
        maximum_total_elements,
        maximum_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
        maximum_total_string_bytes: MAXIMUM_FOOTER_BYTES,
    })
}

pub(in crate::columnar::parquet) fn estimate_raw_preflight_bytes(
    footer_length: usize,
    limits: CompactLimits,
) -> Result<usize, EmbeddingColumnarError> {
    let maximum_inline_element = [
        size_of::<SchemaElement>(),
        size_of::<RowGroup>(),
        size_of::<ColumnChunk>(),
        size_of::<ColumnMetaData>(),
        size_of::<KeyValue>(),
        size_of::<PageEncodingStats>(),
        size_of::<ColumnOrder>(),
        size_of::<Encoding>(),
        size_of::<String>(),
    ]
    .into_iter()
    .max()
    .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let metadata_tree = limits
        .maximum_total_elements
        .checked_mul(maximum_inline_element)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(footer_length))
        .and_then(|value| value.checked_add(size_of::<FileMetaData>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let protocol_state = limits
        .maximum_depth
        .checked_mul(64)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let footer_replay = footer_length;
    let page_workspace = MAXIMUM_PAGE_HEADER_BYTES
        .checked_mul(2)
        .and_then(|value| value.checked_add(64 * maximum_inline_element))
        .and_then(|value| value.checked_add(protocol_state))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    metadata_tree
        .checked_add(protocol_state)
        .and_then(|value| value.checked_add(footer_replay))
        .and_then(|value| value.checked_add(page_workspace))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

pub(in crate::columnar::parquet) fn estimate_stock_metadata_bytes(
    expected_row_groups: usize,
) -> Result<usize, EmbeddingColumnarError> {
    let raw_equivalent = estimate_raw_preflight_bytes(
        MAXIMUM_FOOTER_BYTES,
        footer_compact_limits(expected_row_groups)?,
    )?;
    raw_equivalent
        .checked_mul(2)
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}
