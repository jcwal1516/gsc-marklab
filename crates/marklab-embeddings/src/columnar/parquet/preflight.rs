use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    str::FromStr,
};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{
        ColumnChunk, ColumnMetaData, ColumnOrder, CompressionCodec, ConvertedType, Encoding,
        FieldRepetitionType, FileMetaData, KeyValue, LogicalType, PageEncodingStats, PageHeader,
        PageType, RowGroup, SchemaElement, Type,
    },
    thrift::TSerializable,
};

use crate::ExpectedCellSet;

use super::{
    super::{
        CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets, EmbeddingColumnarError,
        ParquetFailure,
    },
    compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
    profile::{
        embedding_decoded_bytes, metadata_values, CREATED_BY, MAXIMUM_APPLICATION_METADATA_BYTES,
        MAXIMUM_DIMENSION, MAXIMUM_FOOTER_BYTES, MAXIMUM_PAGE_BYTES, MAXIMUM_PAGE_HEADER_BYTES,
        MAXIMUM_ROWS, METADATA_KEYS, PARQUET_MAGIC, ROOT_NAME, ROW_GROUP_ROWS, TRAILER_BYTES,
    },
};

/// Validated structural declaration for one canonical embedding Parquet file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingParquetPreflight {
    row_count: u64,
    dimension: u32,
    row_group_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
}

pub(super) struct PreparedCellEmbeddingParquet {
    pub(super) summary: CellEmbeddingParquetPreflight,
    pub(super) metadata: ParquetMetaData,
}

impl CellEmbeddingParquetPreflight {
    /// Declared canonical rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Declared embedding width supplied by the exact table manifest.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Number of canonical Parquet row groups.
    pub fn row_group_count(self) -> u32 {
        self.row_group_count
    }

    /// Exact encoded Parquet byte length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// SHA-256 of the exact preflighted bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }
}

/// Validate hostile borrowed Parquet bytes before any stock Parquet decoder sees them.
pub fn preflight_cell_embedding_table_parquet_bytes(
    bytes: &[u8],
    expected: &ExpectedCellSet,
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingParquetPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    preflight_cell_embedding_table_parquet_reader(
        &mut Cursor::new(bytes),
        encoded_byte_len,
        ContentDigest::from_bytes(bytes),
        expected,
        dimension,
        bindings,
        budgets,
    )
}

pub(super) fn preflight_cell_embedding_table_parquet_reader<R: Read + Seek + ?Sized>(
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

pub(super) fn prepare_cell_embedding_table_parquet_reader<R: Read + Seek + ?Sized>(
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

pub(super) fn declared_table_logical_digest_parquet_reader<R: Read + Seek + ?Sized>(
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

pub(super) fn footer_compact_limits(
    expected_row_groups: usize,
) -> Result<CompactLimits, EmbeddingColumnarError> {
    footer_compact_limits_with_metadata(expected_row_groups, METADATA_KEYS.len())
}

pub(super) fn footer_compact_limits_with_metadata(
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

pub(super) fn estimate_raw_preflight_bytes(
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

pub(super) fn estimate_stock_metadata_bytes(
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

fn validate_file_metadata<F>(
    metadata: &FileMetaData,
    footer_start: usize,
    expected: &ExpectedCellSet,
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
    mut page_validator: F,
) -> Result<(), EmbeddingColumnarError>
where
    F: FnMut(usize, usize, usize, usize, u32) -> Result<ValidatedPages, EmbeddingColumnarError>,
{
    let expected_rows = expected.cells().len();
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema)?;
    validate_metadata(metadata, bindings)?;
    if metadata.created_by.as_deref() != Some(CREATED_BY)
        || metadata.encryption_algorithm.is_some()
        || metadata.footer_signing_key_metadata.is_some()
        || metadata.column_orders.as_ref().is_none_or(|orders| {
            orders.len() != 3
                || orders
                    .iter()
                    .any(|order| !matches!(order, ColumnOrder::TYPEORDER(_)))
        })
    {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }

    let mut aggregate_rows = 0_usize;
    let mut identifier_bytes = 0_usize;
    let mut status_bytes = 0_usize;
    let mut next_chunk_offset = PARQUET_MAGIC.len();
    for (group_index, row_group) in metadata.row_groups.iter().enumerate() {
        let remaining = expected_rows
            .checked_sub(aggregate_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let rows = remaining.min(ROW_GROUP_ROWS);
        if usize::try_from(row_group.num_rows).ok() != Some(rows)
            || row_group.columns.len() != 3
            || row_group.sorting_columns.is_some()
            || row_group.ordinal != i16::try_from(group_index).ok()
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        let group_start = row_group
            .file_offset
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowGroup))?;
        if group_start != next_chunk_offset {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        let expected_paths: [&[&str]; 3] = [
            &["cell_id"],
            &["embedding", "list", "element"],
            &["embedding_status"],
        ];
        let expected_types = [Type::BYTE_ARRAY, Type::FLOAT, Type::BYTE_ARRAY];
        let expected_values = [
            rows,
            rows.checked_mul(
                usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or(EmbeddingColumnarError::SizeOverflow)?,
            rows,
        ];
        let mut group_compressed_bytes = 0_usize;
        let mut group_uncompressed_bytes = 0_usize;
        for (column_index, column) in row_group.columns.iter().enumerate() {
            if column.file_path.is_some()
                || column.file_offset != 0
                || column.offset_index_offset.is_some()
                || column.offset_index_length.is_some()
                || column.column_index_offset.is_some()
                || column.column_index_length.is_some()
                || column.crypto_metadata.is_some()
                || column.encrypted_column_metadata.is_some()
            {
                return Err(parquet_failure(ParquetFailure::ForbiddenAuxiliaryData));
            }
            let column_metadata = column
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            let expected_path = expected_paths[column_index];
            if column_metadata.type_ != expected_types[column_index]
                || column_metadata.encodings != [Encoding::PLAIN, Encoding::RLE]
                || column_metadata.path_in_schema.len() != expected_path.len()
                || column_metadata
                    .path_in_schema
                    .iter()
                    .map(String::as_str)
                    .ne(expected_path.iter().copied())
                || column_metadata.codec != CompressionCodec::UNCOMPRESSED
                || usize::try_from(column_metadata.num_values).ok()
                    != Some(expected_values[column_index])
                || column_metadata.total_compressed_size <= 0
                || column_metadata.total_compressed_size != column_metadata.total_uncompressed_size
                || column_metadata.key_value_metadata.is_some()
                || column_metadata.index_page_offset.is_some()
                || column_metadata.dictionary_page_offset.is_some()
                || column_metadata.statistics.is_some()
                || column_metadata.bloom_filter_offset.is_some()
                || column_metadata.bloom_filter_length.is_some()
                || column_metadata.size_statistics.is_some()
                || column_metadata.geospatial_statistics.is_some()
                || column_metadata
                    .encoding_stats
                    .as_ref()
                    .is_none_or(|statistics| {
                        statistics.len() != 1
                            || statistics[0].page_type != PageType::DATA_PAGE_V2
                            || statistics[0].encoding != Encoding::PLAIN
                            || statistics[0].count <= 0
                    })
            {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            let data_offset = usize::try_from(column_metadata.data_page_offset)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            let compressed_bytes = usize::try_from(column_metadata.total_compressed_size)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            let uncompressed_bytes = usize::try_from(column_metadata.total_uncompressed_size)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            if data_offset != next_chunk_offset {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            next_chunk_offset = data_offset
                .checked_add(compressed_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if next_chunk_offset > footer_start {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            let pages = page_validator(
                data_offset,
                next_chunk_offset,
                column_index,
                rows,
                dimension,
            )?;
            if column_metadata
                .encoding_stats
                .as_ref()
                .is_none_or(|statistics| {
                    i32::try_from(pages.page_count).ok() != Some(statistics[0].count)
                })
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            match column_index {
                0 => {
                    identifier_bytes = identifier_bytes
                        .checked_add(pages.string_payload_bytes)
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                }
                2 => {
                    status_bytes = status_bytes
                        .checked_add(pages.string_payload_bytes)
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                }
                _ => {}
            }
            group_compressed_bytes = group_compressed_bytes
                .checked_add(compressed_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            group_uncompressed_bytes = group_uncompressed_bytes
                .checked_add(uncompressed_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
        if usize::try_from(row_group.total_byte_size).ok() != Some(group_uncompressed_bytes)
            || row_group
                .total_compressed_size
                .and_then(|value| usize::try_from(value).ok())
                != Some(group_compressed_bytes)
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        enforce_row_group_budget(group_compressed_bytes, budgets)?;
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    if aggregate_rows != expected_rows || next_chunk_offset != footer_start {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    enforce_decoded_budget(
        embedding_decoded_bytes(expected_rows, dimension, identifier_bytes, status_bytes)?,
        budgets,
    )
}

#[derive(Clone, Copy)]
struct ValidatedPages {
    page_count: usize,
    string_payload_bytes: usize,
}

fn validate_pages(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    column_index: usize,
    rows: usize,
    dimension: u32,
) -> Result<ValidatedPages, EmbeddingColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut aggregate_rows = 0_usize;
    let mut aggregate_values = 0_usize;
    let mut string_payload_bytes = 0_usize;
    while cursor < end {
        let header_limit = cursor
            .checked_add(MAXIMUM_PAGE_HEADER_BYTES)
            .map(|value| value.min(end))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let header_window = absolute_slice(bytes, base_offset, cursor, header_limit)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let limits = CompactLimits {
            maximum_depth: 8,
            maximum_fields: 64,
            maximum_collection_elements: 16,
            maximum_total_elements: 64,
            maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
            maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        };
        let mut protocol = BoundedCompactProtocol::new(header_window, limits);
        let header = PageHeader::read_from_in_protocol(&mut protocol)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let header_bytes = protocol
            .consumed_bytes()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        if header_bytes == 0 || header_bytes > MAXIMUM_PAGE_HEADER_BYTES {
            return Err(parquet_failure(ParquetFailure::InvalidPageHeader));
        }
        let header_bytes_slice = header_window
            .get(..header_bytes)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        if !is_canonical_compact(&header, header_bytes_slice)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?
        {
            return Err(parquet_failure(ParquetFailure::InvalidPageHeader));
        }
        let body_bytes = usize::try_from(header.compressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let uncompressed_bytes = usize::try_from(header.uncompressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        if body_bytes > MAXIMUM_PAGE_BYTES || body_bytes != uncompressed_bytes {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body_start = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let page_end = body_start
            .checked_add(body_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if page_end > end {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let page = header
            .data_page_header_v2
            .as_ref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let page_rows = usize::try_from(page.num_rows)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let page_values = usize::try_from(page.num_values)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let page_nulls = usize::try_from(page.num_nulls)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let definition_bytes = usize::try_from(page.definition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let repetition_bytes = usize::try_from(page.repetition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let level_bytes = definition_bytes
            .checked_add(repetition_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let expected_page_values = if column_index == 1 {
            page_rows
                .checked_mul(
                    usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                )
                .ok_or(EmbeddingColumnarError::SizeOverflow)?
        } else {
            page_rows
        };
        if header.type_ != PageType::DATA_PAGE_V2
            || header.crc.is_some()
            || header.data_page_header.is_some()
            || header.index_page_header.is_some()
            || header.dictionary_page_header.is_some()
            || page.encoding != Encoding::PLAIN
            || page.is_compressed != Some(false)
            || page.statistics.is_some()
            || page_rows == 0
            || page_rows > ROW_GROUP_ROWS
            || page_values != expected_page_values
            || page_nulls != 0
            || level_bytes > body_bytes
            || (column_index == 1 && (definition_bytes == 0 || repetition_bytes == 0))
            || (column_index != 1 && level_bytes != 0)
        {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body = absolute_slice(bytes, base_offset, body_start, page_end)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let data_start = level_bytes;
        if column_index == 1 {
            let repetition = body
                .get(..repetition_bytes)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            let definition = body
                .get(repetition_bytes..level_bytes)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            validate_level_stream(
                repetition,
                page_values,
                LevelPattern::FixedListRepetition {
                    dimension: usize::try_from(dimension)
                        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                },
            )?;
            validate_level_stream(definition, page_values, LevelPattern::AllOne)?;
            let values = body
                .get(data_start..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            if values.len()
                != page_values
                    .checked_mul(size_of::<f32>())
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
        } else {
            let values = body
                .get(data_start..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            string_payload_bytes = string_payload_bytes
                .checked_add(validate_plain_byte_arrays(values, page_values)?)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
        aggregate_rows = aggregate_rows
            .checked_add(page_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        aggregate_values = aggregate_values
            .checked_add(page_values)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        page_count = page_count
            .checked_add(1)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        cursor = page_end;
    }
    let expected_values = if column_index == 1 {
        rows.checked_mul(
            usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        )
        .ok_or(EmbeddingColumnarError::SizeOverflow)?
    } else {
        rows
    };
    if cursor != end || aggregate_rows != rows || aggregate_values != expected_values {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(ValidatedPages {
        page_count,
        string_payload_bytes,
    })
}

fn absolute_slice(bytes: &[u8], base_offset: usize, start: usize, end: usize) -> Option<&[u8]> {
    let local_start = start.checked_sub(base_offset)?;
    let local_end = end.checked_sub(base_offset)?;
    bytes.get(local_start..local_end)
}

#[derive(Clone, Copy)]
enum LevelPattern {
    AllOne,
    FixedListRepetition { dimension: usize },
}

fn validate_level_stream(
    bytes: &[u8],
    expected_values: usize,
    pattern: LevelPattern,
) -> Result<(), EmbeddingColumnarError> {
    let mut cursor = 0_usize;
    let mut produced = 0_usize;
    while cursor < bytes.len() {
        if produced == expected_values {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let (header, header_bytes) = read_level_varint(
            bytes
                .get(cursor..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?,
        )?;
        cursor = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if header == 0 {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        if header & 1 == 0 {
            let run = header >> 1;
            if run == 0
                || run
                    > expected_values
                        .checked_sub(produced)
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let value = *bytes
                .get(cursor)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            cursor = cursor
                .checked_add(1)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if value > 1 {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            for index in produced..produced + run {
                if value != expected_level(pattern, index)? {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            produced = produced
                .checked_add(run)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        } else {
            let groups = header >> 1;
            let run = groups
                .checked_mul(8)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let remaining = expected_values
                .checked_sub(produced)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if groups == 0 || run > remaining.saturating_add(7) {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let group_bytes = groups;
            let packed = bytes
                .get(
                    cursor
                        ..cursor
                            .checked_add(group_bytes)
                            .ok_or(EmbeddingColumnarError::SizeOverflow)?,
                )
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            for index in 0..run {
                let value = (packed[index / 8] >> (index % 8)) & 1;
                if index < remaining {
                    if value != expected_level(pattern, produced + index)? {
                        return Err(parquet_failure(ParquetFailure::InvalidPage));
                    }
                } else if value != 0 {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            cursor = cursor
                .checked_add(group_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            produced = produced
                .checked_add(run.min(remaining))
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
    }
    if produced != expected_values {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

fn expected_level(pattern: LevelPattern, index: usize) -> Result<u8, EmbeddingColumnarError> {
    match pattern {
        LevelPattern::AllOne => Ok(1),
        LevelPattern::FixedListRepetition { dimension } => {
            if dimension == 0 {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            Ok(u8::from(!index.is_multiple_of(dimension)))
        }
    }
}

pub(super) fn read_level_varint(bytes: &[u8]) -> Result<(usize, usize), EmbeddingColumnarError> {
    let mut value = 0_u32;
    for index in 0..5_usize {
        let byte = *bytes
            .get(index)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let payload = u32::from(byte & 0x7f);
        if index == 4 && payload > 0x0f {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        value |= payload
            .checked_shl(
                u32::try_from(index * 7).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        if byte & 0x80 == 0 {
            if index != 0 && payload == 0 {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            return Ok((
                usize::try_from(value).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                index + 1,
            ));
        }
    }
    Err(parquet_failure(ParquetFailure::InvalidPage))
}

pub(super) fn validate_plain_byte_arrays(
    bytes: &[u8],
    expected_values: usize,
) -> Result<usize, EmbeddingColumnarError> {
    let mut cursor = 0_usize;
    let mut payload_bytes = 0_usize;
    for _ in 0..expected_values {
        let length_end = cursor
            .checked_add(size_of::<u32>())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let length = u32::from_le_bytes(
            bytes
                .get(cursor..length_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        );
        payload_bytes = payload_bytes
            .checked_add(usize::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        cursor = length_end
            .checked_add(usize::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if cursor > bytes.len() {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(payload_bytes)
}

fn validate_schema(schema: &[SchemaElement]) -> Result<(), EmbeddingColumnarError> {
    if schema.len() != 6
        || !is_group(&schema[0], ROOT_NAME, None, 3, None, None)
        || !is_utf8(&schema[1], "cell_id")
        || !is_group(
            &schema[2],
            "embedding",
            Some(FieldRepetitionType::REQUIRED),
            1,
            Some(ConvertedType::LIST),
            Some("list"),
        )
        || !is_group(
            &schema[3],
            "list",
            Some(FieldRepetitionType::REPEATED),
            1,
            None,
            None,
        )
        || schema[4].type_ != Some(Type::FLOAT)
        || schema[4].type_length.is_some()
        || schema[4].repetition_type != Some(FieldRepetitionType::REQUIRED)
        || schema[4].name != "element"
        || schema[4].num_children.is_some()
        || schema[4].converted_type.is_some()
        || schema[4].scale.is_some()
        || schema[4].precision.is_some()
        || schema[4].field_id.is_some()
        || schema[4].logical_type.is_some()
        || !is_utf8(&schema[5], "embedding_status")
    {
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    Ok(())
}

fn is_group(
    element: &SchemaElement,
    name: &str,
    repetition: Option<FieldRepetitionType>,
    children: i32,
    converted: Option<ConvertedType>,
    logical: Option<&str>,
) -> bool {
    element.type_.is_none()
        && element.type_length.is_none()
        && element.repetition_type == repetition
        && element.name == name
        && element.num_children == Some(children)
        && element.converted_type == converted
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && match logical {
            Some("list") => matches!(element.logical_type, Some(LogicalType::LIST(_))),
            None => element.logical_type.is_none(),
            _ => false,
        }
}

fn is_utf8(element: &SchemaElement, name: &str) -> bool {
    element.type_ == Some(Type::BYTE_ARRAY)
        && element.type_length.is_none()
        && element.repetition_type == Some(FieldRepetitionType::REQUIRED)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type == Some(ConvertedType::UTF8)
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && matches!(element.logical_type, Some(LogicalType::STRING(_)))
}

fn validate_metadata(
    metadata: &FileMetaData,
    bindings: CellEmbeddingTablePhysicalBindings,
) -> Result<(), EmbeddingColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    if entries.len() != METADATA_KEYS.len() {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let values = metadata_values(bindings);
    let mut total_bytes = 0_usize;
    for ((entry, key), value) in entries.iter().zip(METADATA_KEYS).zip(values) {
        let observed_value = entry
            .value
            .as_deref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
        total_bytes = total_bytes
            .checked_add(entry.key.len())
            .and_then(|total| total.checked_add(observed_value.len()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if entry.key != key || observed_value != value {
            return Err(parquet_failure(ParquetFailure::InvalidMetadata));
        }
    }
    if total_bytes > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    Ok(())
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

fn enforce_decoded_budget(
    required: u64,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_decoded_bytes() {
        return Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_decoded_bytes(),
        });
    }
    Ok(())
}

fn parquet_failure(reason: ParquetFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Parquet { reason }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use marklab_data::CellId;
    use marklab_project::{ArtifactId, ContentDigest};
    use thrift::protocol::TCompactOutputProtocol;

    use crate::{CellEmbeddingRow, CellEmbeddingTable, EmbeddingStatus};

    use super::super::writer::write_cell_embedding_table_parquet;
    use super::*;

    fn artifact_id(label: &[u8]) -> ArtifactId {
        ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
    }

    fn fixture() -> (
        ExpectedCellSet,
        CellEmbeddingTable,
        CellEmbeddingTablePhysicalBindings,
        EmbeddingColumnarBudgets,
        Vec<u8>,
    ) {
        let cells = vec![
            CellId::new("cell-a").expect("cell"),
            CellId::new("cell-b").expect("cell"),
        ];
        let expected = ExpectedCellSet::new("levels.v1", cells.clone()).expect("expected");
        let expected_id = artifact_id(b"levels-expected");
        let provenance_id = artifact_id(b"levels-provenance");
        let row_link_id = artifact_id(b"levels-row-link");
        let row_link_digest = ContentDigest::from_bytes(b"levels-row-link-logical");
        let table = CellEmbeddingTable::from_rows(
            3,
            &expected,
            expected_id,
            provenance_id,
            row_link_digest,
            vec![
                CellEmbeddingRow::present(cells[0].clone(), vec![1.0, 2.0, 3.0]),
                CellEmbeddingRow::non_present(cells[1].clone(), EmbeddingStatus::MissingVector)
                    .expect("missing row"),
            ],
            1024 * 1024,
        )
        .expect("table");
        let bindings = CellEmbeddingTablePhysicalBindings::new(
            expected_id,
            provenance_id,
            row_link_id,
            row_link_digest,
            table.qc_summary().logical_digest(),
        )
        .expect("bindings");
        let budgets = EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let mut bytes = Vec::new();
        write_cell_embedding_table_parquet(&mut bytes, &table, bindings, budgets)
            .expect("write Parquet");
        (expected, table, bindings, budgets, bytes)
    }

    fn first_embedding_level_range(bytes: &[u8]) -> std::ops::Range<usize> {
        let footer_length_offset = bytes.len() - TRAILER_BYTES;
        let footer_length = u32::from_le_bytes(
            bytes[footer_length_offset..footer_length_offset + 4]
                .try_into()
                .expect("footer length"),
        ) as usize;
        let footer_start = footer_length_offset - footer_length;
        let mut footer_protocol = BoundedCompactProtocol::new(
            &bytes[footer_start..footer_length_offset],
            CompactLimits {
                maximum_depth: 16,
                maximum_fields: 1024,
                maximum_collection_elements: 64,
                maximum_total_elements: 1024,
                maximum_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
                maximum_total_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
            },
        );
        let metadata =
            FileMetaData::read_from_in_protocol(&mut footer_protocol).expect("footer metadata");
        let column = metadata.row_groups[0].columns[1]
            .meta_data
            .as_ref()
            .expect("column metadata");
        let page_start = usize::try_from(column.data_page_offset).expect("page offset");
        let mut page_protocol = BoundedCompactProtocol::new(
            &bytes[page_start..],
            CompactLimits {
                maximum_depth: 8,
                maximum_fields: 64,
                maximum_collection_elements: 16,
                maximum_total_elements: 64,
                maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
                maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
            },
        );
        let header = PageHeader::read_from_in_protocol(&mut page_protocol).expect("page header");
        let header_bytes = page_protocol.consumed_bytes().expect("header length");
        let page = header.data_page_header_v2.expect("data page v2");
        let repetition_bytes =
            usize::try_from(page.repetition_levels_byte_length).expect("repetition bytes");
        assert!(repetition_bytes > 0);
        let body_start = page_start + header_bytes;
        body_start..body_start + repetition_bytes
    }

    fn raw_footer(bytes: &[u8]) -> (usize, FileMetaData) {
        let length_offset = bytes.len() - TRAILER_BYTES;
        let length = u32::from_le_bytes(
            bytes[length_offset..length_offset + 4]
                .try_into()
                .expect("footer length"),
        ) as usize;
        let start = length_offset - length;
        let mut protocol = BoundedCompactProtocol::new(
            &bytes[start..length_offset],
            CompactLimits {
                maximum_depth: 16,
                maximum_fields: 4096,
                maximum_collection_elements: 128,
                maximum_total_elements: 4096,
                maximum_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
                maximum_total_string_bytes: MAXIMUM_FOOTER_BYTES,
            },
        );
        let metadata = FileMetaData::read_from_in_protocol(&mut protocol).expect("footer metadata");
        (start, metadata)
    }

    fn first_page(bytes: &[u8], column_index: usize) -> (usize, usize, PageHeader) {
        let (_, metadata) = raw_footer(bytes);
        let page_start = usize::try_from(
            metadata.row_groups[0].columns[column_index]
                .meta_data
                .as_ref()
                .expect("column")
                .data_page_offset,
        )
        .expect("page start");
        let mut protocol = BoundedCompactProtocol::new(
            &bytes[page_start..],
            CompactLimits {
                maximum_depth: 8,
                maximum_fields: 64,
                maximum_collection_elements: 16,
                maximum_total_elements: 64,
                maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
                maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
            },
        );
        let header = PageHeader::read_from_in_protocol(&mut protocol).expect("page header");
        (
            page_start,
            protocol.consumed_bytes().expect("header bytes"),
            header,
        )
    }

    fn replace_footer(bytes: &[u8], metadata: &FileMetaData) -> Vec<u8> {
        let (footer_start, _) = raw_footer(bytes);
        let mut footer = Vec::new();
        let mut protocol = TCompactOutputProtocol::new(&mut footer);
        metadata
            .write_to_out_protocol(&mut protocol)
            .expect("serialize footer");
        let mut rewritten = bytes[..footer_start].to_vec();
        rewritten.extend_from_slice(&footer);
        rewritten.extend_from_slice(&(footer.len() as u32).to_le_bytes());
        rewritten.extend_from_slice(PARQUET_MAGIC);
        rewritten
    }

    #[test]
    fn preflight_rejects_corrupted_fixed_list_level_streams() {
        let (expected, table, bindings, budgets, mut bytes) = fixture();
        let levels = first_embedding_level_range(&bytes);
        let last = levels.end - 1;
        bytes[last] ^= 1;
        assert!(matches!(
            preflight_cell_embedding_table_parquet_bytes(
                &bytes,
                &expected,
                table.dimension(),
                bindings,
                budgets,
            ),
            Err(EmbeddingColumnarError::Parquet {
                reason: ParquetFailure::InvalidPage,
            })
        ));
    }

    #[test]
    fn preflight_rejects_schema_metadata_auxiliary_and_chunk_range_drift() {
        let (expected, table, bindings, budgets, canonical) = fixture();
        let assert_rejected = |bytes: &[u8], reason| {
            assert!(matches!(
                preflight_cell_embedding_table_parquet_bytes(
                    bytes,
                    &expected,
                    table.dimension(),
                    bindings,
                    budgets,
                ),
                Err(EmbeddingColumnarError::Parquet { reason: observed }) if observed == reason
            ));
        };

        let (_, mut wrong_schema) = raw_footer(&canonical);
        wrong_schema.schema[0].name = "wrong_root".to_owned();
        assert_rejected(
            &replace_footer(&canonical, &wrong_schema),
            ParquetFailure::InvalidSchema,
        );

        let (_, mut wrong_metadata) = raw_footer(&canonical);
        wrong_metadata
            .key_value_metadata
            .as_mut()
            .expect("metadata")[0]
            .key = "marklab.invalid_metadata".to_owned();
        assert_rejected(
            &replace_footer(&canonical, &wrong_metadata),
            ParquetFailure::InvalidMetadata,
        );

        let (_, mut auxiliary) = raw_footer(&canonical);
        auxiliary.row_groups[0].columns[0]
            .meta_data
            .as_mut()
            .expect("column")
            .dictionary_page_offset = Some(4);
        assert_rejected(
            &replace_footer(&canonical, &auxiliary),
            ParquetFailure::InvalidColumnChunk,
        );

        let (_, mut gap) = raw_footer(&canonical);
        gap.row_groups[0].columns[1]
            .meta_data
            .as_mut()
            .expect("column")
            .data_page_offset += 1;
        assert_rejected(
            &replace_footer(&canonical, &gap),
            ParquetFailure::InvalidColumnChunk,
        );
    }

    #[test]
    fn preflight_rejects_hostile_plain_lengths_and_page_declarations() {
        let (expected, table, bindings, budgets, canonical) = fixture();
        let (page_start, header_bytes, header) = first_page(&canonical, 0);
        let body_start = page_start + header_bytes;

        let mut bad_length = canonical.clone();
        bad_length[body_start..body_start + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            preflight_cell_embedding_table_parquet_bytes(
                &bad_length,
                &expected,
                table.dimension(),
                bindings,
                budgets,
            ),
            Err(EmbeddingColumnarError::Parquet {
                reason: ParquetFailure::InvalidPage,
            })
        ));

        let mut wrong_values = header;
        wrong_values
            .data_page_header_v2
            .as_mut()
            .expect("data page")
            .num_values -= 1;
        let mut encoded_header = Vec::new();
        let mut protocol = TCompactOutputProtocol::new(&mut encoded_header);
        wrong_values
            .write_to_out_protocol(&mut protocol)
            .expect("serialize page header");
        assert_eq!(encoded_header.len(), header_bytes);
        let mut wrong_values_bytes = canonical;
        wrong_values_bytes[page_start..body_start].copy_from_slice(&encoded_header);
        assert!(matches!(
            preflight_cell_embedding_table_parquet_bytes(
                &wrong_values_bytes,
                &expected,
                table.dimension(),
                bindings,
                budgets,
            ),
            Err(EmbeddingColumnarError::Parquet {
                reason: ParquetFailure::InvalidPage,
            })
        ));
    }
}
