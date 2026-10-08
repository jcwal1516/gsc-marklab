use super::{file_metadata::validate_metadata_tree, pages::validate_pages, *};

pub(in crate::columnar::parquet::row_link) fn preflight_cell_embedding_row_link_parquet_reader<
    R: Read + Seek + ?Sized,
>(
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

pub(in crate::columnar::parquet::row_link) fn prepare_cell_embedding_row_link_parquet_reader<
    R: Read + Seek + ?Sized,
>(
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
