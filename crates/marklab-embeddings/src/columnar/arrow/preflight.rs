use std::io::Cursor;

use arrow_ipc::{root_as_message_with_opts, MessageHeader, MetadataVersion};
use marklab_project::ContentDigest;

use crate::{EmbeddingStatus, ExpectedCellSet};

use super::{
    super::{
        enforce_file_budget, ArrowIpcFailure, CellEmbeddingTablePhysicalBindings,
        EmbeddingColumnarBudgets, EmbeddingColumnarError,
    },
    preflight_reader::{
        declared_table_logical_digest_reader, preflight_cell_embedding_table_arrow_reader,
    },
    profile::{
        align, arrow_failure, message_verifier_options, nonnegative_i32_usize, nonnegative_usize,
        validate_all_valid_bitmap, validity_bytes, ALIGNMENT, CONTINUATION_MARKER,
        MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
    },
};

/// Validated structural declaration for one canonical Arrow embedding table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingArrowPreflight {
    row_count: u64,
    dimension: u32,
    record_batch_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) maximum_batch_decoded_bytes: usize,
}

impl CellEmbeddingArrowPreflight {
    pub(super) fn new(
        row_count: u64,
        dimension: u32,
        record_batch_count: u32,
        encoded_byte_len: u64,
        content_digest: ContentDigest,
        retained_preflight_bytes: usize,
        maximum_batch_decoded_bytes: usize,
    ) -> Self {
        Self {
            row_count,
            dimension,
            record_batch_count,
            encoded_byte_len,
            content_digest,
            retained_preflight_bytes,
            maximum_batch_decoded_bytes,
        }
    }

    /// Declared canonical rows validated across every record batch.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Exact nonzero fixed-list dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Number of canonical record batches.
    pub fn record_batch_count(self) -> u32 {
        self.record_batch_count
    }

    /// Exact encoded Arrow file length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// SHA-256 of the exact preflighted bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }
}

/// Validate hostile Arrow bytes through the shared bounded seek-based preflight.
pub fn preflight_cell_embedding_table_arrow_bytes(
    bytes: &[u8],
    expected: &ExpectedCellSet,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingArrowPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    preflight_cell_embedding_table_arrow_reader(
        &mut Cursor::new(bytes),
        ContentDigest::from_bytes(bytes),
        expected,
        bindings,
        budgets,
    )
}

pub(super) fn declared_table_logical_digest(
    bytes: &[u8],
    budgets: EmbeddingColumnarBudgets,
) -> Result<ContentDigest, EmbeddingColumnarError> {
    declared_table_logical_digest_reader(&mut Cursor::new(bytes), budgets)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_record_batch_message(
    message_bytes: &[u8],
    body_bytes: &[u8],
    declared_body_length: i64,
    expected_rows: usize,
    expected_cells: &[marklab_data::CellId],
    dimension: u32,
    aggregate_decoded_bytes: &mut u64,
) -> Result<(), EmbeddingColumnarError> {
    if message_bytes.len() < 8 || &message_bytes[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let declared_metadata = i32::from_le_bytes(
        message_bytes[4..8]
            .try_into()
            .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?,
    );
    let declared_metadata =
        nonnegative_i32_usize(declared_metadata, ArrowIpcFailure::InvalidMessage)?;
    if declared_metadata == 0
        || declared_metadata.checked_add(8) != Some(message_bytes.len())
        || declared_metadata > MAXIMUM_MESSAGE_BYTES - 8
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let message = root_as_message_with_opts(&message_verifier_options(), &message_bytes[8..])
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    if message.version() != MetadataVersion::V5
        || message.header_type() != MessageHeader::RecordBatch
        || message.bodyLength() != declared_body_length
        || message.bodyLength()
            != i64::try_from(body_bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?
        || message
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let batch = message
        .header_as_record_batch()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    if batch.compression().is_some()
        || batch
            .variadicBufferCounts()
            .is_some_and(|counts| !counts.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let rows = nonnegative_usize(batch.length(), ArrowIpcFailure::InvalidRowCount)?;
    if rows == 0 || rows > RECORD_BATCH_ROWS || rows != expected_rows {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFieldNodes))?;
    if nodes.len() != 4 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFieldNodes));
    }
    let element_count = rows
        .checked_mul(usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let expected_node_lengths = [rows, rows, element_count, rows];
    for (node, expected_length) in nodes.iter().zip(expected_node_lengths) {
        let length = nonnegative_usize(node.length(), ArrowIpcFailure::InvalidFieldNodes)?;
        let null_count = nonnegative_usize(node.null_count(), ArrowIpcFailure::InvalidFieldNodes)?;
        if length != expected_length || null_count != 0 {
            return Err(arrow_failure(ArrowIpcFailure::InvalidFieldNodes));
        }
    }

    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    if buffers.len() != 9 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    let offset_bytes = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let cell_bytes = expected_cells.iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(cell.as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    let value_bytes = element_count
        .checked_mul(size_of::<f32>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let minimum_status_bytes = rows
        .checked_mul(EmbeddingStatus::Present.wire_name().len())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let maximum_status_bytes = rows
        .checked_mul(EmbeddingStatus::ExtractionFailed.wire_name().len())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let row_validity_bytes = validity_bytes(rows)?;
    let value_validity_bytes = validity_bytes(element_count)?;
    let exact_lengths = [
        Some(row_validity_bytes),
        Some(offset_bytes),
        Some(cell_bytes),
        Some(row_validity_bytes),
        Some(value_validity_bytes),
        Some(value_bytes),
        Some(row_validity_bytes),
        Some(offset_bytes),
        None,
    ];
    let mut next_offset = 0_usize;
    let mut decoded_bytes = 0_u64;
    for (index, (buffer, exact_length)) in buffers.iter().zip(exact_lengths).enumerate() {
        let offset = nonnegative_usize(buffer.offset(), ArrowIpcFailure::InvalidBuffers)?;
        let length = nonnegative_usize(buffer.length(), ArrowIpcFailure::InvalidBuffers)?;
        if offset != align(next_offset)? || offset % ALIGNMENT != 0 {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        if exact_length.is_some_and(|expected_length| length != expected_length)
            || (index == 8 && !(minimum_status_bytes..=maximum_status_bytes).contains(&length))
        {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        let end = offset
            .checked_add(length)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
        if end > body_bytes.len() {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        if matches!(index, 0 | 3 | 4 | 6) {
            let bit_count = if index == 4 { element_count } else { rows };
            validate_all_valid_bitmap(&body_bytes[offset..end], bit_count)?;
        }
        next_offset = end;
        decoded_bytes = decoded_bytes
            .checked_add(u64::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    if align(next_offset)? != body_bytes.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    *aggregate_decoded_bytes = aggregate_decoded_bytes
        .checked_add(decoded_bytes)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use arrow_ipc::{
        root_as_message_with_opts, Block, BodyCompression, BodyCompressionArgs, Footer, FooterArgs,
        KeyValue, KeyValueArgs, Message, MessageArgs, MessageHeader, MetadataVersion, RecordBatch,
        RecordBatchArgs,
    };
    use flatbuffers::FlatBufferBuilder;
    use marklab_data::CellId;
    use marklab_project::{ArtifactId, ContentDigest};

    use super::*;

    fn artifact_id(label: &[u8]) -> ArtifactId {
        ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
    }

    fn bindings() -> CellEmbeddingTablePhysicalBindings {
        CellEmbeddingTablePhysicalBindings::new(
            artifact_id(b"expected"),
            artifact_id(b"provenance"),
            artifact_id(b"row-link"),
            ContentDigest::from_bytes(b"row-link-logical"),
            ContentDigest::from_bytes(b"table-logical"),
        )
        .expect("bindings")
    }

    fn budgets() -> EmbeddingColumnarBudgets {
        EmbeddingColumnarBudgets::new(
            2 * 1024 * 1024,
            2 * 1024 * 1024,
            2 * 1024 * 1024,
            2 * 1024 * 1024,
        )
    }

    fn framed_record_batch_message(compression: bool, variadic: bool) -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let compression = compression
            .then(|| BodyCompression::create(&mut builder, &BodyCompressionArgs::default()));
        let variadic_buffer_counts = variadic.then(|| builder.create_vector(&[1_i64]));
        let batch = RecordBatch::create(
            &mut builder,
            &RecordBatchArgs {
                length: 1,
                nodes: None,
                buffers: None,
                compression,
                variadicBufferCounts: variadic_buffer_counts,
            },
        );
        let message = Message::create(
            &mut builder,
            &MessageArgs {
                version: MetadataVersion::V5,
                header_type: MessageHeader::RecordBatch,
                header: Some(batch.as_union_value()),
                bodyLength: 0,
                custom_metadata: None,
            },
        );
        builder.finish(message, None);
        let payload = builder.finished_data();
        let mut framed = Vec::with_capacity(payload.len() + 8);
        framed.extend_from_slice(CONTINUATION_MARKER);
        framed.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        framed.extend_from_slice(payload);
        framed
    }

    fn wrap_footer(footer: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0_u8; 64];
        bytes[..6].copy_from_slice(b"ARROW1");
        bytes.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]);
        bytes.extend_from_slice(footer);
        bytes.extend_from_slice(&(footer.len() as i32).to_le_bytes());
        bytes.extend_from_slice(b"ARROW1");
        bytes
    }

    fn footer_bytes(dictionary: bool, metadata_tables: usize) -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let dictionaries = dictionary.then(|| builder.create_vector(&[Block::new(64, 64, 0)]));
        let empty_blocks: [Block; 0] = [];
        let record_batches = builder.create_vector(&empty_blocks);
        let mut entries = Vec::with_capacity(metadata_tables);
        for index in 0..metadata_tables {
            let key = builder.create_string(&format!("key-{index}"));
            let value = builder.create_string("value");
            entries.push(KeyValue::create(
                &mut builder,
                &KeyValueArgs {
                    key: Some(key),
                    value: Some(value),
                },
            ));
        }
        let custom_metadata = (!entries.is_empty()).then(|| builder.create_vector(&entries));
        let footer = Footer::create(
            &mut builder,
            &FooterArgs {
                version: MetadataVersion::V5,
                schema: None,
                dictionaries,
                recordBatches: Some(record_batches),
                custom_metadata,
            },
        );
        builder.finish(footer, None);
        builder.finished_data().to_vec()
    }

    #[test]
    fn shared_record_batch_validator_rejects_compression_and_variadic_declarations() {
        let cell = CellId::new("cell-a").expect("cell ID");
        for message in [
            framed_record_batch_message(true, false),
            framed_record_batch_message(false, true),
        ] {
            let mut decoded_bytes = 0;
            assert!(matches!(
                validate_record_batch_message(
                    &message,
                    &[],
                    0,
                    1,
                    std::slice::from_ref(&cell),
                    1,
                    &mut decoded_bytes,
                ),
                Err(EmbeddingColumnarError::Arrow {
                    reason: ArrowIpcFailure::InvalidMessage,
                })
            ));
        }
    }

    #[test]
    fn public_preflight_rejects_dictionary_and_table_heavy_footers_before_schema_decode() {
        let expected = ExpectedCellSet::new("all.v1", Vec::new()).expect("expected cells");
        let dictionary = wrap_footer(&footer_bytes(true, 0));
        assert!(matches!(
            preflight_cell_embedding_table_arrow_bytes(
                &dictionary,
                &expected,
                bindings(),
                budgets(),
            ),
            Err(EmbeddingColumnarError::Arrow {
                reason: ArrowIpcFailure::DictionaryBatch,
            })
        ));

        let table_heavy = wrap_footer(&footer_bytes(false, 40));
        assert!(matches!(
            preflight_cell_embedding_table_arrow_bytes(
                &table_heavy,
                &expected,
                bindings(),
                budgets(),
            ),
            Err(EmbeddingColumnarError::Arrow {
                reason: ArrowIpcFailure::InvalidFooter,
            })
        ));
    }

    #[test]
    fn verifier_limits_reject_oversized_footer_and_table_heavy_message() {
        let expected = ExpectedCellSet::new("all.v1", Vec::new()).expect("expected cells");
        let footer_length = 1024 * 1024 + 1;
        let mut oversized = vec![0_u8; 64];
        oversized[..6].copy_from_slice(b"ARROW1");
        oversized.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]);
        oversized.resize(64 + 8 + footer_length, 0);
        oversized.extend_from_slice(&(footer_length as i32).to_le_bytes());
        oversized.extend_from_slice(b"ARROW1");
        assert!(matches!(
            preflight_cell_embedding_table_arrow_bytes(
                &oversized,
                &expected,
                bindings(),
                budgets(),
            ),
            Err(EmbeddingColumnarError::Arrow {
                reason: ArrowIpcFailure::InvalidFooterLength,
            })
        ));

        let mut builder = FlatBufferBuilder::new();
        let mut entries = Vec::new();
        for index in 0..40 {
            let key = builder.create_string(&format!("key-{index}"));
            let value = builder.create_string("value");
            entries.push(KeyValue::create(
                &mut builder,
                &KeyValueArgs {
                    key: Some(key),
                    value: Some(value),
                },
            ));
        }
        let custom_metadata = builder.create_vector(&entries);
        let message = Message::create(
            &mut builder,
            &MessageArgs {
                version: MetadataVersion::V5,
                header_type: MessageHeader::Schema,
                header: None,
                bodyLength: 0,
                custom_metadata: Some(custom_metadata),
            },
        );
        builder.finish(message, None);
        assert!(root_as_message_with_opts(
            &super::super::profile::schema_message_verifier_options(),
            builder.finished_data(),
        )
        .is_err());
    }
}
