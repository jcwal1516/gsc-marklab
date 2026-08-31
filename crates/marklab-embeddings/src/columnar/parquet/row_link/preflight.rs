use std::io::{Cursor, Read, Seek, SeekFrom};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{
        ColumnOrder, CompressionCodec, ConvertedType, Encoding, FieldRepetitionType, FileMetaData,
        LogicalType, PageHeader, PageType, SchemaElement, Type,
    },
    thrift::TSerializable,
};

use crate::columnar::parquet::schema::is_required_utf8 as is_utf8;
use crate::{CellEmbeddingRowLink, ExpectedCellSet};

use super::super::super::{
    enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, ParquetFailure,
};
use super::{
    super::{
        compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
        preflight::{
            estimate_raw_preflight_bytes, estimate_stock_metadata_bytes,
            footer_compact_limits_with_metadata, read_level_varint,
        },
        profile::{
            CREATED_BY, MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_FOOTER_BYTES,
            MAXIMUM_PAGE_BYTES, MAXIMUM_PAGE_HEADER_BYTES, MAXIMUM_ROWS, PARQUET_MAGIC,
            ROW_GROUP_ROWS, TRAILER_BYTES,
        },
    },
    profile::{metadata_values, METADATA_KEYS, ROOT_NAME},
    writer::estimate_decoded_bytes,
};

/// Validated structural declaration for one canonical row-link Parquet file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingRowLinkParquetPreflight {
    row_count: u64,
    row_group_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
}

pub(super) struct PreparedCellEmbeddingRowLinkParquet {
    pub(super) summary: CellEmbeddingRowLinkParquetPreflight,
    pub(super) metadata: ParquetMetaData,
}

impl CellEmbeddingRowLinkParquetPreflight {
    /// Declared canonical rows.
    pub fn row_count(self) -> u64 {
        self.row_count
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

/// Validate hostile borrowed row-link Parquet bytes before stock decoding.
pub fn preflight_cell_embedding_row_link_parquet_bytes(
    bytes: &[u8],
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkParquetPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    preflight_cell_embedding_row_link_parquet_reader(
        &mut Cursor::new(bytes),
        encoded_byte_len,
        ContentDigest::from_bytes(bytes),
        expected,
        row_link,
        budgets,
    )
}

pub(super) fn preflight_cell_embedding_row_link_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkParquetPreflight, EmbeddingColumnarError> {
    prepare_cell_embedding_row_link_parquet_reader(
        reader,
        encoded_byte_len,
        content_digest,
        expected,
        row_link,
        budgets,
    )
    .map(|prepared| prepared.summary)
}

pub(super) fn prepare_cell_embedding_row_link_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PreparedCellEmbeddingRowLinkParquet, EmbeddingColumnarError> {
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    if row_link.entries().len() > MAXIMUM_ROWS
        || row_link.entries().len() != expected.cells().len()
        || row_link.expected_cells_logical_digest() != expected.logical_digest()
        || row_link
            .entries()
            .iter()
            .zip(expected.cells())
            .any(|(entry, cell)| entry.cell_id() != cell)
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    enforce_decoded_budget(estimate_decoded_bytes(row_link)?, budgets)?;
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
    let footer_length = usize::try_from(u32::from_le_bytes(
        trailer[..4]
            .try_into()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidFooterLength))?,
    ))
    .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if footer_length == 0 || footer_length > MAXIMUM_FOOTER_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    let footer_start = footer_length_offset
        .checked_sub(footer_length)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?;
    if footer_start < PARQUET_MAGIC.len() {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    let expected_groups = row_link.entries().len().div_ceil(ROW_GROUP_ROWS);
    let limits = footer_compact_limits_with_metadata(expected_groups, METADATA_KEYS.len())?;
    let raw_retained = estimate_raw_preflight_bytes(footer_length, limits)?;
    let stock_retained = estimate_stock_metadata_bytes(expected_groups)?;
    let mut retained_preflight_bytes = raw_retained.max(stock_retained);
    enforce_retained_budget(retained_preflight_bytes, budgets)?;
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
    validate_metadata_tree(
        footer_start,
        &metadata,
        expected,
        row_link,
        budgets,
        |start, end, column_index, group_start_row, rows, row_link| {
            let chunk_bytes = end
                .checked_sub(start)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let peak = raw_retained
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
            validate_pages(
                &chunk,
                start,
                start,
                end,
                column_index,
                group_start_row,
                rows,
                row_link,
            )
        },
    )?;
    let row_group_count = metadata.row_groups.len();
    drop(metadata);
    let stock = ParquetMetaDataReader::decode_metadata(&footer)
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    if stock.memory_size() > stock_retained
        || stock.file_metadata().version() != 2
        || usize::try_from(stock.file_metadata().num_rows()).ok() != Some(row_link.entries().len())
        || stock.num_row_groups() != expected_groups
        || stock.file_metadata().created_by() != Some(CREATED_BY)
        || stock.column_index().is_some()
        || stock.offset_index().is_some()
    {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    Ok(PreparedCellEmbeddingRowLinkParquet {
        summary: CellEmbeddingRowLinkParquetPreflight {
            row_count: row_link.row_count(),
            row_group_count: u32::try_from(row_group_count)
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            encoded_byte_len,
            content_digest,
            retained_preflight_bytes,
        },
        metadata: stock,
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

fn validate_metadata_tree<F>(
    footer_start: usize,
    metadata: &FileMetaData,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
    mut page_validator: F,
) -> Result<(), EmbeddingColumnarError>
where
    F: FnMut(
        usize,
        usize,
        usize,
        usize,
        usize,
        &CellEmbeddingRowLink,
    ) -> Result<usize, EmbeddingColumnarError>,
{
    let expected_rows = row_link.entries().len();
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema)?;
    validate_metadata(metadata, row_link)?;
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
    let mut next_offset = PARQUET_MAGIC.len();
    for (group_index, group) in metadata.row_groups.iter().enumerate() {
        let rows = expected_rows
            .checked_sub(aggregate_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?
            .min(ROW_GROUP_ROWS);
        if usize::try_from(group.num_rows).ok() != Some(rows)
            || group.columns.len() != 3
            || group.sorting_columns.is_some()
            || group.ordinal != i16::try_from(group_index).ok()
            || group
                .file_offset
                .and_then(|value| usize::try_from(value).ok())
                != Some(next_offset)
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        let paths: [&[&str]; 3] = [
            &["cell_id"],
            &["source_cell_row"],
            &["source_embedding_row"],
        ];
        let types = [Type::BYTE_ARRAY, Type::INT64, Type::INT64];
        let mut group_bytes = 0_usize;
        for (column_index, column) in group.columns.iter().enumerate() {
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
            if column_metadata.codec != CompressionCodec::UNCOMPRESSED {
                return Err(parquet_failure(ParquetFailure::UnsupportedCompression));
            }
            if column_metadata.encodings != [Encoding::PLAIN, Encoding::RLE] {
                return Err(parquet_failure(ParquetFailure::UnsupportedEncoding));
            }
            if column_metadata.key_value_metadata.is_some()
                || column_metadata.index_page_offset.is_some()
                || column_metadata.dictionary_page_offset.is_some()
                || column_metadata.statistics.is_some()
                || column_metadata.bloom_filter_offset.is_some()
                || column_metadata.bloom_filter_length.is_some()
                || column_metadata.size_statistics.is_some()
                || column_metadata.geospatial_statistics.is_some()
            {
                return Err(parquet_failure(ParquetFailure::ForbiddenAuxiliaryData));
            }
            if column_metadata.type_ != types[column_index]
                || column_metadata
                    .path_in_schema
                    .iter()
                    .map(String::as_str)
                    .ne(paths[column_index].iter().copied())
                || usize::try_from(column_metadata.num_values).ok() != Some(rows)
                || column_metadata.total_compressed_size <= 0
                || column_metadata.total_compressed_size != column_metadata.total_uncompressed_size
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
            let start = usize::try_from(column_metadata.data_page_offset)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            let length = usize::try_from(column_metadata.total_compressed_size)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            if start != next_offset {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            next_offset = start
                .checked_add(length)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if next_offset > footer_start {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            let pages = page_validator(
                start,
                next_offset,
                column_index,
                aggregate_rows,
                rows,
                row_link,
            )?;
            if column_metadata
                .encoding_stats
                .as_ref()
                .is_none_or(|statistics| i32::try_from(pages).ok() != Some(statistics[0].count))
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            group_bytes = group_bytes
                .checked_add(length)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
        if usize::try_from(group.total_byte_size).ok() != Some(group_bytes)
            || group
                .total_compressed_size
                .and_then(|value| usize::try_from(value).ok())
                != Some(group_bytes)
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        enforce_row_group_budget(group_bytes, budgets)?;
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    if aggregate_rows != expected.cells().len() || next_offset != footer_start {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_pages(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    row_link: &CellEmbeddingRowLink,
) -> Result<usize, EmbeddingColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut page_start_row = 0_usize;
    while cursor < end {
        let header_limit = cursor
            .checked_add(MAXIMUM_PAGE_HEADER_BYTES)
            .map(|value| value.min(end))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let window = absolute_slice(bytes, base_offset, cursor, header_limit)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let mut protocol = BoundedCompactProtocol::new(window, page_limits());
        let header = PageHeader::read_from_in_protocol(&mut protocol)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let header_bytes = protocol
            .consumed_bytes()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        if header_bytes == 0
            || header_bytes > MAXIMUM_PAGE_HEADER_BYTES
            || !is_canonical_compact(
                &header,
                window
                    .get(..header_bytes)
                    .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?,
            )
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?
        {
            return Err(parquet_failure(ParquetFailure::InvalidPageHeader));
        }
        let body_bytes = usize::try_from(header.compressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let uncompressed_bytes = usize::try_from(header.uncompressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let body_start = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let page_end = body_start
            .checked_add(body_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let page = header
            .data_page_header_v2
            .as_ref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let rows = usize::try_from(page.num_rows)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let values = usize::try_from(page.num_values)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let nulls = usize::try_from(page.num_nulls)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let definitions = usize::try_from(page.definition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let repetitions = usize::try_from(page.repetition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        if body_bytes > MAXIMUM_PAGE_BYTES
            || body_bytes != uncompressed_bytes
            || page_end > end
            || header.type_ != PageType::DATA_PAGE_V2
            || header.crc.is_some()
            || header.data_page_header.is_some()
            || header.index_page_header.is_some()
            || header.dictionary_page_header.is_some()
            || page.encoding != Encoding::PLAIN
            || page.is_compressed != Some(false)
            || page.statistics.is_some()
            || rows == 0
            || values != rows
            || page_start_row
                .checked_add(rows)
                .is_none_or(|value| value > group_rows)
            || repetitions != 0
            || (column_index < 2 && (definitions != 0 || nulls != 0))
            || (column_index == 2 && definitions == 0)
        {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body = absolute_slice(bytes, base_offset, body_start, page_end)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let data = body
            .get(definitions..)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        match column_index {
            0 => {
                let absolute_start = group_start_row
                    .checked_add(page_start_row)
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                validate_plain_cell_ids(data, row_link, absolute_start, rows)?;
            }
            1 => {
                if data.len()
                    != values
                        .checked_mul(size_of::<u64>())
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?
                {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            2 => {
                let absolute_start = group_start_row
                    .checked_add(page_start_row)
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                let entries = row_link
                    .entries()
                    .get(absolute_start..absolute_start + rows)
                    .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
                let expected_present = entries
                    .iter()
                    .map(|entry| entry.source_embedding_row().is_some())
                    .collect::<Vec<_>>();
                let expected_nulls = expected_present.iter().filter(|present| !**present).count();
                if nulls != expected_nulls {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
                validate_optional_levels(
                    body.get(..definitions)
                        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?,
                    &expected_present,
                )?;
                if data.len()
                    != values
                        .checked_sub(nulls)
                        .and_then(|value| value.checked_mul(size_of::<u64>()))
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?
                {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            _ => return Err(parquet_failure(ParquetFailure::InvalidPage)),
        }
        page_start_row = page_start_row
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        page_count = page_count
            .checked_add(1)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        cursor = page_end;
    }
    if cursor != end || page_start_row != group_rows {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(page_count)
}

fn validate_plain_cell_ids(
    bytes: &[u8],
    row_link: &CellEmbeddingRowLink,
    start_row: usize,
    rows: usize,
) -> Result<(), EmbeddingColumnarError> {
    let entries = row_link
        .entries()
        .get(
            start_row
                ..start_row
                    .checked_add(rows)
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?,
        )
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
    let mut cursor = 0_usize;
    for entry in entries {
        let length_end = cursor
            .checked_add(size_of::<u32>())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let length = usize::try_from(u32::from_le_bytes(
            bytes
                .get(cursor..length_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        ))
        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
        let value_end = length_end
            .checked_add(length)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let value = bytes
            .get(length_end..value_end)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        if value != entry.cell_id().as_str().as_bytes() {
            return Err(parquet_failure(ParquetFailure::InvalidCellOrder));
        }
        cursor = value_end;
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

fn absolute_slice(bytes: &[u8], base_offset: usize, start: usize, end: usize) -> Option<&[u8]> {
    bytes.get(start.checked_sub(base_offset)?..end.checked_sub(base_offset)?)
}

fn validate_optional_levels(bytes: &[u8], expected: &[bool]) -> Result<(), EmbeddingColumnarError> {
    let mut cursor = 0_usize;
    let mut produced = 0_usize;
    while cursor < bytes.len() {
        if produced == expected.len() {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let (header, header_bytes) = read_level_varint(&bytes[cursor..])?;
        cursor = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if header == 0 {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        if header & 1 == 0 {
            let run = header >> 1;
            if run == 0
                || produced
                    .checked_add(run)
                    .is_none_or(|value| value > expected.len())
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let value = *bytes
                .get(cursor)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            cursor += 1;
            if value > 1
                || expected[produced..produced + run]
                    .iter()
                    .any(|present| u8::from(*present) != value)
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            produced += run;
        } else {
            let groups = header >> 1;
            let run = groups
                .checked_mul(8)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let remaining = expected
                .len()
                .checked_sub(produced)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if groups == 0 || run > remaining.saturating_add(7) {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let packed = bytes
                .get(cursor..cursor + groups)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            for index in 0..run {
                let value = (packed[index / 8] >> (index % 8)) & 1;
                if index < remaining {
                    if value != u8::from(expected[produced + index]) {
                        return Err(parquet_failure(ParquetFailure::InvalidPage));
                    }
                } else if value != 0 {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            cursor += groups;
            produced += run.min(remaining);
        }
    }
    if produced != expected.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

fn validate_schema(schema: &[SchemaElement]) -> Result<(), EmbeddingColumnarError> {
    if schema.len() != 4
        || !is_group(&schema[0], ROOT_NAME, 3)
        || !is_utf8(&schema[1], "cell_id")
        || !is_u64(&schema[2], "source_cell_row", FieldRepetitionType::REQUIRED)
        || !is_u64(
            &schema[3],
            "source_embedding_row",
            FieldRepetitionType::OPTIONAL,
        )
    {
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    Ok(())
}

fn is_group(element: &SchemaElement, name: &str, children: i32) -> bool {
    element.type_.is_none()
        && element.type_length.is_none()
        && element.repetition_type.is_none()
        && element.name == name
        && element.num_children == Some(children)
        && element.converted_type.is_none()
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && element.logical_type.is_none()
}

fn is_u64(element: &SchemaElement, name: &str, repetition: FieldRepetitionType) -> bool {
    element.type_ == Some(Type::INT64)
        && element.type_length.is_none()
        && element.repetition_type == Some(repetition)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type == Some(ConvertedType::UINT_64)
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && matches!(
            element.logical_type,
            Some(LogicalType::INTEGER(ref integer))
                if integer.bit_width == 64 && !integer.is_signed
        )
}

fn validate_metadata(
    metadata: &FileMetaData,
    row_link: &CellEmbeddingRowLink,
) -> Result<(), EmbeddingColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    if entries.len() != METADATA_KEYS.len() {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let values = metadata_values(row_link);
    let mut total = 0_usize;
    for ((entry, key), value) in entries.iter().zip(METADATA_KEYS).zip(values) {
        let observed = entry
            .value
            .as_deref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
        total = total
            .checked_add(entry.key.len())
            .and_then(|value| value.checked_add(observed.len()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if entry.key != key || observed != value {
            return Err(parquet_failure(ParquetFailure::InvalidMetadata));
        }
    }
    if total > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    Ok(())
}

fn page_limits() -> CompactLimits {
    CompactLimits {
        maximum_depth: 8,
        maximum_fields: 64,
        maximum_collection_elements: 16,
        maximum_total_elements: 64,
        maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
    }
}

fn parquet_failure(reason: ParquetFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Parquet { reason }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use marklab_data::{
        CellId, CohortHierarchy, HierarchyId, HierarchyNode, PatientId, ReplicationRole, SlideId,
    };
    use marklab_project::{ArtifactId, ContentDigest};
    use thrift::protocol::TCompactOutputProtocol;

    use crate::CellEmbeddingRowLinkEntry;

    use super::super::writer::write_cell_embedding_row_link_parquet;
    use super::*;

    fn artifact_id(label: &[u8]) -> ArtifactId {
        ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
    }

    fn fixture() -> (
        ExpectedCellSet,
        CellEmbeddingRowLink,
        EmbeddingColumnarBudgets,
        Vec<u8>,
    ) {
        let (expected, row_link) = logical_fixture(["cell-a", "cell-b", "cell-c", "cell-d"]);
        let budgets = EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            16 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let mut bytes = Vec::new();
        write_cell_embedding_row_link_parquet(&mut bytes, &row_link, budgets)
            .expect("write row link");
        (expected, row_link, budgets, bytes)
    }

    fn logical_fixture(cell_names: [&str; 4]) -> (ExpectedCellSet, CellEmbeddingRowLink) {
        let cells = cell_names
            .into_iter()
            .map(|value| CellId::new(value).expect("cell"))
            .collect::<Vec<_>>();
        let expected =
            ExpectedCellSet::new("row-link-levels.v1", cells.clone()).expect("expected cells");
        let patient = HierarchyId::from(PatientId::new("patient").expect("patient"));
        let slide = HierarchyId::from(SlideId::new("slide").expect("slide"));
        let mut nodes = vec![
            HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
            HierarchyNode::new(
                slide.clone(),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient,
                },
            ),
        ];
        nodes.extend(cells.iter().cloned().map(|cell| {
            HierarchyNode::new(
                HierarchyId::from(cell),
                Some(slide.clone()),
                ReplicationRole::Structural,
            )
        }));
        let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");
        let row_link = CellEmbeddingRowLink::new(
            artifact_id(b"source-cells"),
            artifact_id(b"source-vectors"),
            artifact_id(b"expected"),
            artifact_id(b"identity-map"),
            artifact_id(b"converter"),
            &expected,
            &hierarchy,
            vec![
                CellEmbeddingRowLinkEntry::present(cells[0].clone(), 2, 1),
                CellEmbeddingRowLinkEntry::missing_vector(cells[1].clone(), 0),
                CellEmbeddingRowLinkEntry::extraction_failed(cells[2].clone(), 3),
                CellEmbeddingRowLinkEntry::qc_rejected(cells[3].clone(), 1, 0),
            ],
            1024 * 1024,
        )
        .expect("row link");
        (expected, row_link)
    }

    fn raw_footer(bytes: &[u8]) -> (usize, FileMetaData) {
        let footer_offset = bytes.len() - TRAILER_BYTES;
        let footer_length = u32::from_le_bytes(
            bytes[footer_offset..footer_offset + 4]
                .try_into()
                .expect("footer length"),
        ) as usize;
        let footer_start = footer_offset - footer_length;
        let mut protocol = BoundedCompactProtocol::new(
            &bytes[footer_start..footer_offset],
            footer_compact_limits_with_metadata(1, METADATA_KEYS.len()).expect("limits"),
        );
        let metadata = FileMetaData::read_from_in_protocol(&mut protocol).expect("footer metadata");
        (footer_start, metadata)
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

    fn optional_definition_range(bytes: &[u8]) -> std::ops::Range<usize> {
        let footer_offset = bytes.len() - TRAILER_BYTES;
        let footer_length = u32::from_le_bytes(
            bytes[footer_offset..footer_offset + 4]
                .try_into()
                .expect("footer length"),
        ) as usize;
        let footer_start = footer_offset - footer_length;
        let mut footer_protocol = BoundedCompactProtocol::new(
            &bytes[footer_start..footer_offset],
            footer_compact_limits_with_metadata(1, METADATA_KEYS.len()).expect("limits"),
        );
        let metadata =
            FileMetaData::read_from_in_protocol(&mut footer_protocol).expect("footer metadata");
        let page_start = usize::try_from(
            metadata.row_groups[0].columns[2]
                .meta_data
                .as_ref()
                .expect("column")
                .data_page_offset,
        )
        .expect("page start");
        let mut page_protocol = BoundedCompactProtocol::new(&bytes[page_start..], page_limits());
        let header = PageHeader::read_from_in_protocol(&mut page_protocol).expect("page header");
        let header_bytes = page_protocol.consumed_bytes().expect("header bytes");
        let definitions = usize::try_from(
            header
                .data_page_header_v2
                .expect("page v2")
                .definition_levels_byte_length,
        )
        .expect("definition bytes");
        assert!(definitions > 0);
        let body_start = page_start + header_bytes;
        body_start..body_start + definitions
    }

    #[test]
    fn preflight_rejects_row_link_schema_metadata_codec_encoding_and_auxiliary_drift() {
        let (expected, row_link, budgets, canonical) = fixture();
        let assert_rejected = |metadata: &FileMetaData, reason| {
            assert!(matches!(
                preflight_cell_embedding_row_link_parquet_bytes(
                    &replace_footer(&canonical, metadata),
                    &expected,
                    &row_link,
                    budgets,
                ),
                Err(EmbeddingColumnarError::Parquet { reason: observed }) if observed == reason
            ));
        };

        let (_, mut wrong_schema) = raw_footer(&canonical);
        wrong_schema.schema[0].name = "wrong_root".to_owned();
        assert_rejected(&wrong_schema, ParquetFailure::InvalidSchema);

        let (_, mut wrong_metadata) = raw_footer(&canonical);
        wrong_metadata.created_by = Some("wrong-writer".to_owned());
        assert_rejected(&wrong_metadata, ParquetFailure::InvalidMetadata);

        let (_, mut compressed) = raw_footer(&canonical);
        compressed.row_groups[0].columns[0]
            .meta_data
            .as_mut()
            .expect("column")
            .codec = CompressionCodec::SNAPPY;
        assert_rejected(&compressed, ParquetFailure::UnsupportedCompression);

        let (_, mut dictionary_encoding) = raw_footer(&canonical);
        dictionary_encoding.row_groups[0].columns[0]
            .meta_data
            .as_mut()
            .expect("column")
            .encodings
            .push(Encoding::RLE_DICTIONARY);
        assert_rejected(&dictionary_encoding, ParquetFailure::UnsupportedEncoding);

        let (_, mut dictionary_page) = raw_footer(&canonical);
        dictionary_page.row_groups[0].columns[0]
            .meta_data
            .as_mut()
            .expect("column")
            .dictionary_page_offset = Some(4);
        assert_rejected(&dictionary_page, ParquetFailure::ForbiddenAuxiliaryData);

        let (_, mut indexes) = raw_footer(&canonical);
        indexes.row_groups[0].columns[0].offset_index_offset = Some(4);
        indexes.row_groups[0].columns[0].offset_index_length = Some(8);
        assert_rejected(&indexes, ParquetFailure::ForbiddenAuxiliaryData);
    }

    #[test]
    fn preflight_rejects_optional_levels_that_disagree_with_the_logical_row_link() {
        let (expected, row_link, budgets, mut bytes) = fixture();
        let levels = optional_definition_range(&bytes);
        bytes[levels.end - 1] ^= 1;
        assert!(matches!(
            preflight_cell_embedding_row_link_parquet_bytes(&bytes, &expected, &row_link, budgets,),
            Err(EmbeddingColumnarError::Parquet {
                reason: ParquetFailure::InvalidPage,
            })
        ));
    }

    #[test]
    fn preflight_rejects_larger_physical_cell_ids_before_stock_decode_at_exact_budget() {
        let (expected, row_link, budgets, canonical) = fixture();
        let (_, longer_row_link) = logical_fixture([
            "cell-a-hostile-physical-payload",
            "cell-b-hostile-physical-payload",
            "cell-c-hostile-physical-payload",
            "cell-d-hostile-physical-payload",
        ]);
        let mut longer = Vec::new();
        write_cell_embedding_row_link_parquet(&mut longer, &longer_row_link, budgets)
            .expect("write longer row link");
        assert!(longer.len() > canonical.len());

        let (_, mut metadata) = raw_footer(&longer);
        let digest = metadata
            .key_value_metadata
            .as_mut()
            .expect("metadata")
            .iter_mut()
            .find(|entry| entry.key == "marklab.row_link_digest")
            .expect("row-link digest");
        digest.value = Some(row_link.logical_digest().to_string());
        let hostile = replace_footer(&longer, &metadata);
        let required = estimate_decoded_bytes(&row_link).expect("decoded bytes");
        let exact = EmbeddingColumnarBudgets::new(
            budgets.maximum_file_bytes(),
            budgets.maximum_retained_bytes(),
            budgets.maximum_row_group_bytes(),
            required,
        );
        assert!(matches!(
            preflight_cell_embedding_row_link_parquet_bytes(&hostile, &expected, &row_link, exact,),
            Err(EmbeddingColumnarError::Parquet {
                reason: ParquetFailure::InvalidCellOrder,
            })
        ));

        let one_short = EmbeddingColumnarBudgets::new(
            budgets.maximum_file_bytes(),
            budgets.maximum_retained_bytes(),
            budgets.maximum_row_group_bytes(),
            required - 1,
        );
        assert_eq!(
            preflight_cell_embedding_row_link_parquet_bytes(
                &hostile, &expected, &row_link, one_short,
            ),
            Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
                required,
                maximum: required - 1,
            })
        );
    }
}
