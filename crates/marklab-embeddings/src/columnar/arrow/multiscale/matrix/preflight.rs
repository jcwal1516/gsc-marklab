use std::{io::Cursor, mem::size_of};

use arrow_ipc::{
    convert::try_fb_to_schema,
    writer::{DictionaryTracker, IpcDataGenerator, IpcWriteOptions},
    Buffer, FieldNode, MessageBuilder, MessageHeader, MetadataVersion, RecordBatchBuilder,
};
use flatbuffers::FlatBufferBuilder;
use marklab_project::ContentDigest;

use crate::{
    columnar::{
        multiscale::{
            enforce_decoded_budget, enforce_file_budget, matrix_decoded_bytes,
            validate_matrix_domain, MatrixPhysicalAccumulator, MultiscaleMatrixTable,
        },
        ArrowIpcFailure, ColumnarError, EmbeddingColumnarBudgets,
    },
    EmbeddingEntityKind, MultiscaleEmbeddingQcSummary, PatchEmbeddingTable, RegionEmbeddingTable,
    SlideEmbeddingTable,
};

use super::super::super::physical::{
    align, decode_footer, decode_record_batch_message, nonnegative_i64, read_exact_at,
    read_footer_deferred_eos, traverse_record_batches, validate_footer_features,
    validate_record_batch_features, validity_bytes, BufferLength,
};
use super::profile::{
    arrow_failure, entity_kind, schema, validate_flatbuffer_schema, BUFFER_COUNT, NODE_COUNT,
};
use crate::columnar::arrow::profile::{
    ALIGNMENT, EOS_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
};

/// Raw bounded declaration and factual QC for one canonical multiscale Arrow matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiscaleMatrixArrowPreflight {
    entity_kind: EmbeddingEntityKind,
    qc_summary: MultiscaleEmbeddingQcSummary,
    row_count: u64,
    dimension: u32,
    record_batch_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) maximum_batch_decoded_bytes: usize,
}

impl MultiscaleMatrixArrowPreflight {
    /// Exact matrix entity kind.
    pub fn entity_kind(self) -> EmbeddingEntityKind {
        self.entity_kind
    }

    /// Factual QC and format-independent logical identity proven by raw rows.
    pub fn qc_summary(self) -> MultiscaleEmbeddingQcSummary {
        self.qc_summary
    }

    /// Exact canonical row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Exact fixed vector dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Number of canonical 8,192-row record batches.
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

macro_rules! typed_preflight {
    ($name:ident, $table:ty) => {
        #[doc = "Validate borrowed bytes through the raw bounded multiscale Arrow matrix preflight."]
        pub fn $name(
            bytes: &[u8],
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixArrowPreflight, ColumnarError> {
            let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
            enforce_file_budget(encoded, budgets)?;
            preflight_reader(
                &mut Cursor::new(bytes),
                ContentDigest::from_bytes(bytes),
                table,
                budgets,
            )
        }
    };
}

typed_preflight!(
    preflight_patch_embedding_table_arrow_bytes,
    PatchEmbeddingTable
);
typed_preflight!(
    preflight_region_embedding_table_arrow_bytes,
    RegionEmbeddingTable
);
typed_preflight!(
    preflight_slide_embedding_table_arrow_bytes,
    SlideEmbeddingTable
);

pub(super) fn preflight_reader<R: std::io::Read + std::io::Seek + ?Sized>(
    reader: &mut R,
    content_digest: ContentDigest,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<MultiscaleMatrixArrowPreflight, ColumnarError> {
    validate_matrix_domain(table)?;
    let encoded_byte_len = reader
        .seek(std::io::SeekFrom::End(0))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    let file_len = usize::try_from(encoded_byte_len).map_err(|_| ColumnarError::SizeOverflow)?;
    let (footer_start, footer_len, footer_bytes) =
        read_footer_deferred_eos(reader, file_len, budgets)?;
    let footer = decode_footer(&footer_bytes)?;
    validate_footer_features(footer)?;
    let footer_schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    validate_flatbuffer_schema(footer_schema, table)?;
    let expected_schema = schema(table)?;
    if try_fb_to_schema(footer_schema).map_err(|_| arrow_failure(ArrowIpcFailure::InvalidSchema))?
        != expected_schema
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    let schema_end = super::super::super::physical::validate_schema_message(
        reader,
        file_len,
        footer_start,
        &expected_schema,
        |observed| validate_flatbuffer_schema(observed, table),
        |bytes| {
            validate_zero_message_padding(bytes, canonical_schema_message_len(&expected_schema)?)
        },
    )?;
    let blocks = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    if blocks.len() != table.row_count().div_ceil(RECORD_BATCH_ROWS) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let expected_decoded = aggregate_decoded(table)?;
    enforce_decoded_budget(expected_decoded, budgets)?;
    let mut accumulator = MatrixPhysicalAccumulator::new(table)?;
    let traversal = traverse_record_batches(
        reader,
        file_len,
        footer_start,
        footer_len,
        schema_end,
        blocks.iter(),
        table.row_count(),
        budgets,
        |message, body, declared_body_len, start, rows| {
            validate_record_batch(
                message,
                body,
                declared_body_len,
                table,
                start,
                rows,
                &mut accumulator,
            )
        },
    )?;
    validate_eos(reader, file_len, traversal.end_offset, footer_start)?;
    let qc_summary = accumulator.finish()?;
    if traversal.row_count != table.row_count()
        || traversal.decoded_bytes != expected_decoded
        || qc_summary != table.qc_summary()
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    Ok(MultiscaleMatrixArrowPreflight {
        entity_kind: entity_kind(table.profile()),
        qc_summary,
        row_count: u64::try_from(table.row_count()).map_err(|_| ColumnarError::SizeOverflow)?,
        dimension: table.dimension(),
        record_batch_count: u32::try_from(blocks.len()).map_err(|_| ColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: traversal.retained_preflight_bytes,
        maximum_batch_decoded_bytes: usize::try_from(traversal.maximum_batch_decoded_bytes)
            .map_err(|_| ColumnarError::SizeOverflow)?,
    })
}

fn aggregate_decoded(table: &dyn MultiscaleMatrixTable) -> Result<u64, ColumnarError> {
    let mut total = 0_u64;
    let mut start = 0_usize;
    while start < table.row_count() {
        let end = start
            .checked_add(RECORD_BATCH_ROWS)
            .map(|value| value.min(table.row_count()))
            .ok_or(ColumnarError::SizeOverflow)?;
        total = total
            .checked_add(
                u64::try_from(matrix_decoded_bytes(table, start, end)?)
                    .map_err(|_| ColumnarError::SizeOverflow)?,
            )
            .ok_or(ColumnarError::SizeOverflow)?;
        start = end;
    }
    Ok(total)
}

#[allow(clippy::too_many_arguments)]
fn validate_record_batch(
    message_bytes: &[u8],
    body: &[u8],
    declared_body_len: i64,
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    rows: usize,
    accumulator: &mut MatrixPhysicalAccumulator,
) -> Result<u64, ColumnarError> {
    let batch = decode_record_batch_message(message_bytes, body, declared_body_len)?;
    if usize::try_from(batch.length()).ok() != Some(rows) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    validate_record_batch_features(batch)?;
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFieldNodes))?;
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    let components = rows
        .checked_mul(usize::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?)
        .ok_or(ColumnarError::SizeOverflow)?;
    if nodes.len() != NODE_COUNT
        || buffers.len() != BUFFER_COUNT
        || ![(rows, 0), (rows, 0), (components, 0), (rows, 0)]
            .into_iter()
            .zip(nodes.iter())
            .all(|((length, nulls), node)| {
                usize::try_from(node.length()).ok() == Some(length)
                    && usize::try_from(node.null_count()).ok() == Some(nulls)
            })
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFieldNodes));
    }
    let row_validity = BufferLength::Validity(validity_bytes(rows)?);
    let expected_lengths = [
        row_validity,
        BufferLength::Exact(offsets_bytes(rows)?),
        BufferLength::Exact(id_bytes(table, start, rows)?),
        row_validity,
        BufferLength::Validity(validity_bytes(components)?),
        BufferLength::Exact(
            components
                .checked_mul(size_of::<f32>())
                .ok_or(ColumnarError::SizeOverflow)?,
        ),
        row_validity,
        BufferLength::Exact(offsets_bytes(rows)?),
        BufferLength::Exact(status_bytes(table, start, rows)?),
    ];
    let mut ranges = [(0_usize, 0_usize); BUFFER_COUNT];
    let mut next = 0_usize;
    let mut decoded = 0_u64;
    for (index, (buffer, expected)) in buffers.iter().zip(expected_lengths).enumerate() {
        let offset = nonnegative_i64(buffer.offset(), ArrowIpcFailure::InvalidBuffers)?;
        let length = nonnegative_i64(buffer.length(), ArrowIpcFailure::InvalidBuffers)?;
        if offset != align(next)?
            || offset % ALIGNMENT != 0
            || !expected.accepts(length)
            || body
                .get(next..offset)
                .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
        {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        let end = offset
            .checked_add(length)
            .ok_or(ColumnarError::SizeOverflow)?;
        if end > body.len() {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        ranges[index] = (offset, end);
        next = end;
        decoded = decoded
            .checked_add(
                u64::try_from(expected.decoded_bytes()).map_err(|_| ColumnarError::SizeOverflow)?,
            )
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    if align(next)? != body.len() || body[next..].iter().any(|byte| *byte != 0) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    for index in [0, 3, 4, 6] {
        let (begin, end) = ranges[index];
        if body[begin..end].iter().any(|byte| *byte != 0xff) {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
    }
    validate_string_column(body, ranges[1], ranges[2], rows, |local| {
        table.row(start + local).ok().map(|row| row.id())
    })?;
    validate_string_column(body, ranges[7], ranges[8], rows, |local| {
        table
            .row(start + local)
            .ok()
            .map(|row| row.status().wire_name())
    })?;
    let (values_start, values_end) = ranges[5];
    let value_bytes = &body[values_start..values_end];
    let dimension = usize::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?;
    for local in 0..rows {
        let row = table
            .row(start + local)
            .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?;
        for column in 0..dimension {
            let index = local
                .checked_mul(dimension)
                .and_then(|value| value.checked_add(column))
                .ok_or(ColumnarError::SizeOverflow)?;
            let begin = index
                .checked_mul(size_of::<f32>())
                .ok_or(ColumnarError::SizeOverflow)?;
            let end = begin
                .checked_add(size_of::<f32>())
                .ok_or(ColumnarError::SizeOverflow)?;
            let observed = f32::from_le_bytes(
                value_bytes
                    .get(begin..end)
                    .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?
                    .try_into()
                    .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?,
            );
            let expected = row.vector().map_or(0.0, |vector| vector[column]);
            if observed.to_bits() != expected.to_bits() {
                return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
            }
        }
        accumulator.push(row.id(), row.status(), row.vector())?;
    }
    validate_zero_message_padding(
        message_bytes,
        canonical_record_message_len(batch, declared_body_len)?,
    )?;
    Ok(decoded)
}

fn validate_string_column<'a>(
    body: &[u8],
    offset_range: (usize, usize),
    value_range: (usize, usize),
    rows: usize,
    mut expected: impl FnMut(usize) -> Option<&'a str>,
) -> Result<(), ColumnarError> {
    let offsets = &body[offset_range.0..offset_range.1];
    let values = &body[value_range.0..value_range.1];
    let mut prior = 0_usize;
    for index in 0..=rows {
        let begin = index
            .checked_mul(size_of::<i32>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let end = begin
            .checked_add(size_of::<i32>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let current = usize::try_from(i32::from_le_bytes(
            offsets
                .get(begin..end)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        ))
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
        if index == 0 {
            if current != 0 {
                return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
            }
        } else {
            let value = expected(index - 1)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?;
            if values.get(prior..current) != Some(value.as_bytes()) {
                return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
            }
        }
        if current < prior || current > values.len() {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        prior = current;
    }
    if prior != values.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    Ok(())
}

fn validate_zero_message_padding(
    bytes: &[u8],
    canonical_flatbuffer_len: usize,
) -> Result<(), ColumnarError> {
    let padding_start = 8_usize
        .checked_add(canonical_flatbuffer_len)
        .ok_or(ColumnarError::SizeOverflow)?;
    if bytes
        .get(padding_start..)
        .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    Ok(())
}

fn canonical_schema_message_len(schema: &arrow::datatypes::Schema) -> Result<usize, ColumnarError> {
    let options = IpcWriteOptions::try_new(ALIGNMENT, false, MetadataVersion::V5)
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    let mut tracker = DictionaryTracker::new(true);
    let encoded =
        IpcDataGenerator {}.schema_to_bytes_with_dictionary_tracker(schema, &mut tracker, &options);
    if !encoded.arrow_data.is_empty() || encoded.ipc_message.len() > MAXIMUM_MESSAGE_BYTES {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    Ok(encoded.ipc_message.len())
}

fn canonical_record_message_len(
    batch: arrow_ipc::RecordBatch<'_>,
    body_len: i64,
) -> Result<usize, ColumnarError> {
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFieldNodes))?
        .iter()
        .map(|node| FieldNode::new(node.length(), node.null_count()))
        .collect::<Vec<_>>();
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?
        .iter()
        .map(|buffer| Buffer::new(buffer.offset(), buffer.length()))
        .collect::<Vec<_>>();
    let mut builder = FlatBufferBuilder::new();
    let buffers = builder.create_vector(&buffers);
    let nodes = builder.create_vector(&nodes);
    let header = {
        let mut record = RecordBatchBuilder::new(&mut builder);
        record.add_length(batch.length());
        record.add_nodes(nodes);
        record.add_buffers(buffers);
        record.finish().as_union_value()
    };
    let message = {
        let mut message = MessageBuilder::new(&mut builder);
        message.add_version(MetadataVersion::V5);
        message.add_header_type(MessageHeader::RecordBatch);
        message.add_bodyLength(body_len);
        message.add_header(header);
        message.finish()
    };
    builder.finish(message, None);
    let len = builder.finished_data().len();
    if len > MAXIMUM_MESSAGE_BYTES {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    Ok(len)
}

fn validate_eos<R: std::io::Read + std::io::Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    footer_start: usize,
) -> Result<(), ColumnarError> {
    if offset.checked_add(EOS_BYTES) != Some(footer_start) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBlock));
    }
    let mut eos = [0_u8; EOS_BYTES];
    read_exact_at(
        reader,
        file_len,
        offset,
        &mut eos,
        ArrowIpcFailure::InvalidMessage,
    )?;
    if eos != [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0] {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    Ok(())
}

fn id_bytes(
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    rows: usize,
) -> Result<usize, ColumnarError> {
    (0..rows).try_fold(0_usize, |total, local| {
        total
            .checked_add(
                table
                    .row(start + local)
                    .map_err(|_| ColumnarError::ArtifactBindingMismatch)?
                    .id()
                    .len(),
            )
            .ok_or(ColumnarError::SizeOverflow)
    })
}

fn status_bytes(
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    rows: usize,
) -> Result<usize, ColumnarError> {
    (0..rows).try_fold(0_usize, |total, local| {
        total
            .checked_add(
                table
                    .row(start + local)
                    .map_err(|_| ColumnarError::ArtifactBindingMismatch)?
                    .status()
                    .wire_name()
                    .len(),
            )
            .ok_or(ColumnarError::SizeOverflow)
    })
}

fn offsets_bytes(rows: usize) -> Result<usize, ColumnarError> {
    rows.checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(ColumnarError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use arrow_ipc::{
        root_as_message, BodyCompressionBuilder, CompressionType, MessageBuilder, MessageHeader,
        MetadataVersion, RecordBatchBuilder,
    };
    use flatbuffers::{FlatBufferBuilder, WIPOffset};

    use super::{validate_record_batch_features, ArrowIpcFailure, ColumnarError};

    #[test]
    fn matrix_record_batch_feature_gate_rejects_compression_and_variadic_buffers() {
        for (with_compression, with_variadic) in [(true, false), (false, true)] {
            let mut builder = FlatBufferBuilder::new();
            let variadic = with_variadic.then(|| builder.create_vector(&[1_i64]));
            let compression = with_compression.then(|| {
                let mut compression = BodyCompressionBuilder::new(&mut builder);
                compression.add_codec(CompressionType::LZ4_FRAME);
                compression.finish()
            });
            let batch = {
                let mut batch = RecordBatchBuilder::new(&mut builder);
                if let Some(compression) = compression {
                    batch.add_compression(compression);
                }
                if let Some(variadic) = variadic {
                    batch.add_variadicBufferCounts(variadic);
                }
                batch.finish()
            };
            let message = {
                let mut message = MessageBuilder::new(&mut builder);
                message.add_version(MetadataVersion::V5);
                message.add_header_type(MessageHeader::RecordBatch);
                message.add_header(WIPOffset::new(batch.value()));
                message.finish()
            };
            builder.finish(message, None);
            let batch = root_as_message(builder.finished_data())
                .expect("message")
                .header_as_record_batch()
                .expect("record batch");
            assert_eq!(
                validate_record_batch_features(batch),
                Err(ColumnarError::Arrow {
                    reason: ArrowIpcFailure::ForbiddenFeature,
                })
            );
        }
    }
}
