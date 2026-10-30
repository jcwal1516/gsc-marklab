use std::{
    io::{Read, Seek, SeekFrom},
    str::FromStr,
};

use arrow_ipc::convert::try_fb_to_schema;
use marklab_project::ContentDigest;

use crate::ExpectedCellSet;

use super::super::{
    enforce_file_budget, ArrowIpcFailure, CellEmbeddingTablePhysicalBindings, ColumnarError,
    EmbeddingColumnarBudgets,
};
use super::{
    physical::{decode_footer, read_footer, read_record_batch_block, validate_schema_message},
    preflight::{validate_record_batch_message, CellEmbeddingArrowPreflight},
    profile::{
        arrow_failure, component_bytes, embedding_schema, enforce_decoded_budget,
        validate_flatbuffer_schema, validate_shape, EOS_BYTES, MAXIMUM_MESSAGE_BYTES,
        METADATA_KEYS, RECORD_BATCH_ROWS,
    },
};

pub(super) fn preflight_cell_embedding_table_arrow_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    content_digest: ContentDigest,
    expected: &ExpectedCellSet,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingArrowPreflight, ColumnarError> {
    let encoded_byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    let file_len = usize::try_from(encoded_byte_len).map_err(|_| ColumnarError::SizeOverflow)?;
    let (footer_start, footer_length, footer_bytes) = read_footer(reader, file_len, budgets)?;
    let footer = decode_footer(&footer_bytes)?;
    if footer
        .custom_metadata()
        .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::FileCustomMetadata));
    }
    if footer
        .dictionaries()
        .is_some_and(|dictionaries| !dictionaries.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::DictionaryBatch));
    }

    let footer_schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    let dimension = validate_flatbuffer_schema(footer_schema, bindings)?;
    if try_fb_to_schema(footer_schema).map_err(|_| arrow_failure(ArrowIpcFailure::InvalidSchema))?
        != embedding_schema(dimension, bindings)?
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_shape(expected.cells().len(), dimension)?;
    enforce_decoded_budget(component_bytes(expected.cells().len(), dimension)?, budgets)?;

    let expected_schema = embedding_schema(dimension, bindings)?;
    let schema_end = validate_schema_message(
        reader,
        file_len,
        footer_start,
        &expected_schema,
        |schema| {
            if validate_flatbuffer_schema(schema, bindings)? != dimension {
                return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
            }
            Ok(())
        },
        |_bytes| Ok(()),
    )?;
    let record_batches = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    let record_batch_count = record_batches.len();
    if record_batch_count != expected.cells().len().div_ceil(RECORD_BATCH_ROWS) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }

    let footer_retained = footer_length
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    let mut retained_required = footer_retained;
    let mut next_offset = schema_end;
    let mut aggregate_rows = 0_usize;
    let mut decoded_bytes = 0_u64;
    let mut maximum_batch_decoded_bytes = 0_u64;
    for (batch_index, block) in record_batches.iter().enumerate() {
        let physical = read_record_batch_block(
            reader,
            file_len,
            footer_start,
            footer_length,
            next_offset,
            block.offset(),
            block.metaDataLength(),
            block.bodyLength(),
            budgets,
        )?;
        retained_required = retained_required.max(physical.retained_preflight_bytes);
        let (message_bytes, body_bytes) = physical.message_and_body();
        let batch_start = batch_index
            .checked_mul(RECORD_BATCH_ROWS)
            .ok_or(ColumnarError::SizeOverflow)?;
        let expected_rows = expected
            .cells()
            .len()
            .saturating_sub(batch_start)
            .min(RECORD_BATCH_ROWS);
        let batch_end = batch_start
            .checked_add(expected_rows)
            .ok_or(ColumnarError::SizeOverflow)?;
        let expected_cells = expected
            .cells()
            .get(batch_start..batch_end)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?;
        let decoded_before = decoded_bytes;
        validate_record_batch_message(
            message_bytes,
            body_bytes,
            block.bodyLength(),
            expected_rows,
            expected_cells,
            dimension,
            &mut decoded_bytes,
        )?;
        maximum_batch_decoded_bytes = maximum_batch_decoded_bytes.max(
            decoded_bytes
                .checked_sub(decoded_before)
                .ok_or(ColumnarError::SizeOverflow)?,
        );
        aggregate_rows = aggregate_rows
            .checked_add(expected_rows)
            .ok_or(ColumnarError::SizeOverflow)?;
        next_offset = physical.end;
    }
    if next_offset != footer_start - EOS_BYTES || aggregate_rows != expected.cells().len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    if decoded_bytes > budgets.maximum_decoded_bytes() {
        return Err(ColumnarError::DecodedByteBudgetExceeded {
            required: decoded_bytes,
            maximum: budgets.maximum_decoded_bytes(),
        });
    }

    Ok(CellEmbeddingArrowPreflight::new(
        u64::try_from(aggregate_rows).map_err(|_| ColumnarError::SizeOverflow)?,
        dimension,
        u32::try_from(record_batch_count).map_err(|_| ColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_required,
        usize::try_from(maximum_batch_decoded_bytes).map_err(|_| ColumnarError::SizeOverflow)?,
    ))
}

pub(super) fn declared_table_logical_digest_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ContentDigest, ColumnarError> {
    let encoded_byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(ColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    let file_len = usize::try_from(encoded_byte_len).map_err(|_| ColumnarError::SizeOverflow)?;
    let (_, _, footer_bytes) = read_footer(reader, file_len, budgets)?;
    let footer = decode_footer(&footer_bytes)?;
    let schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    let metadata = schema
        .custom_metadata()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
    if metadata.len() != METADATA_KEYS.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    let entry = metadata.get(2);
    if entry.key() != Some("marklab.logical_digest") {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    ContentDigest::from_str(
        entry
            .value()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?,
    )
    .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))
}
