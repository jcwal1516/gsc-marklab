use std::{io::Cursor, mem::size_of};

use arrow_ipc::{
    convert::fb_to_schema,
    root_as_footer_with_opts, root_as_message_with_opts,
    writer::{DictionaryTracker, IpcDataGenerator, IpcWriteOptions},
    Buffer, FieldNode, MessageBuilder, MessageHeader, MetadataVersion, RecordBatchBuilder,
};
use flatbuffers::FlatBufferBuilder;
use marklab_project::ContentDigest;

use crate::{
    columnar::{
        multiscale::{
            enforce_decoded_budget, enforce_file_budget, enforce_retained_budget,
            matrix_decoded_bytes, validate_matrix_domain, MatrixPhysicalAccumulator,
            MultiscaleMatrixTable,
        },
        EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
    },
    EmbeddingEntityKind, MultiscaleEmbeddingQcSummary, PatchEmbeddingTable, RegionEmbeddingTable,
    SlideEmbeddingTable,
};

use super::super::physical::read_record_batch_block;
use super::profile::{
    arrow_failure, entity_kind, schema, validate_flatbuffer_schema, BUFFER_COUNT, NODE_COUNT,
};
use crate::columnar::arrow::profile::{
    footer_verifier_options, message_verifier_options, schema_message_verifier_options, ALIGNMENT,
    ARROW_MAGIC, CONTINUATION_MARKER, EOS_BYTES, HEADER_BYTES, MAXIMUM_FOOTER_BYTES,
    MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS, TRAILER_BYTES,
};

/// Raw bounded declaration and factual QC for one canonical C-05 Arrow matrix.
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
        #[doc = "Validate borrowed bytes through the raw bounded C-05 Arrow matrix preflight."]
        pub fn $name(
            bytes: &[u8],
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixArrowPreflight, MultiscaleColumnarError> {
            let encoded =
                u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
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
) -> Result<MultiscaleMatrixArrowPreflight, MultiscaleColumnarError> {
    validate_matrix_domain(table)?;
    let encoded_byte_len = reader
        .seek(std::io::SeekFrom::End(0))
        .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    let file_len =
        usize::try_from(encoded_byte_len).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let (footer_start, footer_len, footer_bytes) = read_footer(reader, file_len, budgets)?;
    let footer = root_as_footer_with_opts(&footer_verifier_options(), &footer_bytes)
        .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidFooter))?;
    if footer.version() != MetadataVersion::V5 {
        return Err(arrow_failure(SpatialArrowFailure::UnsupportedVersion));
    }
    if footer
        .custom_metadata()
        .is_some_and(|metadata| !metadata.is_empty())
        || footer
            .dictionaries()
            .is_some_and(|dictionaries| !dictionaries.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::ForbiddenFeature));
    }
    let footer_schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidSchema))?;
    validate_flatbuffer_schema(footer_schema, table)?;
    let expected_schema = schema(table)?;
    if fb_to_schema(footer_schema) != expected_schema {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    let schema_end =
        validate_schema_message(reader, file_len, footer_start, table, &expected_schema)?;
    let blocks = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFooter))?;
    if blocks.len() != table.row_count().div_ceil(RECORD_BATCH_ROWS) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    let expected_decoded = aggregate_decoded(table)?;
    enforce_decoded_budget(expected_decoded, budgets)?;
    let footer_peak = footer_len
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(footer_peak, budgets)?;
    let mut retained = footer_peak;
    let mut maximum_batch_decoded = 0_u64;
    let mut decoded_total = 0_u64;
    let mut aggregate_rows = 0_usize;
    let mut next_offset = schema_end;
    let mut accumulator = MatrixPhysicalAccumulator::new(table)?;
    for (batch_index, block) in blocks.iter().enumerate() {
        let physical = read_record_batch_block(
            reader,
            file_len,
            footer_start,
            footer_len,
            next_offset,
            block.offset(),
            block.metaDataLength(),
            block.bodyLength(),
            budgets,
        )?;
        retained = retained.max(physical.retained_preflight_bytes);
        let (message, body) = physical.message_and_body();
        let start = batch_index
            .checked_mul(RECORD_BATCH_ROWS)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let rows = table
            .row_count()
            .saturating_sub(start)
            .min(RECORD_BATCH_ROWS);
        let before = decoded_total;
        validate_record_batch(
            message,
            body,
            block.bodyLength(),
            table,
            start,
            rows,
            &mut decoded_total,
            &mut accumulator,
        )?;
        maximum_batch_decoded = maximum_batch_decoded.max(
            decoded_total
                .checked_sub(before)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        next_offset = physical.end;
    }
    validate_eos(reader, file_len, next_offset, footer_start)?;
    let qc_summary = accumulator.finish()?;
    if aggregate_rows != table.row_count()
        || decoded_total != expected_decoded
        || qc_summary != table.qc_summary()
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    Ok(MultiscaleMatrixArrowPreflight {
        entity_kind: entity_kind(table.profile()),
        qc_summary,
        row_count: u64::try_from(table.row_count())
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        dimension: table.dimension(),
        record_batch_count: u32::try_from(blocks.len())
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: retained,
        maximum_batch_decoded_bytes: usize::try_from(maximum_batch_decoded)
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
    })
}

fn aggregate_decoded(table: &dyn MultiscaleMatrixTable) -> Result<u64, MultiscaleColumnarError> {
    let mut total = 0_u64;
    let mut start = 0_usize;
    while start < table.row_count() {
        let end = start
            .checked_add(RECORD_BATCH_ROWS)
            .map(|value| value.min(table.row_count()))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        total = total
            .checked_add(
                u64::try_from(matrix_decoded_bytes(table, start, end)?)
                    .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
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
    decoded_total: &mut u64,
    accumulator: &mut MatrixPhysicalAccumulator,
) -> Result<(), MultiscaleColumnarError> {
    validate_message_padding(message_bytes)?;
    let message = root_as_message_with_opts(&message_verifier_options(), &message_bytes[8..])
        .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?;
    if message.version() != MetadataVersion::V5
        || message.header_type() != MessageHeader::RecordBatch
        || message.bodyLength() != declared_body_len
        || usize::try_from(declared_body_len).ok() != Some(body.len())
        || message
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let batch = message
        .header_as_record_batch()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidMessage))?;
    if usize::try_from(batch.length()).ok() != Some(rows) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    validate_record_batch_features(batch)?;
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFieldNodes))?;
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?;
    let components = rows
        .checked_mul(
            usize::try_from(table.dimension())
                .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        )
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
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
        return Err(arrow_failure(SpatialArrowFailure::InvalidFieldNodes));
    }
    let expected_lengths = [
        validity_bytes(rows)?,
        offsets_bytes(rows)?,
        id_bytes(table, start, rows)?,
        validity_bytes(rows)?,
        validity_bytes(components)?,
        components
            .checked_mul(size_of::<f32>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        validity_bytes(rows)?,
        offsets_bytes(rows)?,
        status_bytes(table, start, rows)?,
    ];
    let mut ranges = [(0_usize, 0_usize); BUFFER_COUNT];
    let mut next = 0_usize;
    let mut decoded = 0_u64;
    for (index, (buffer, expected)) in buffers.iter().zip(expected_lengths).enumerate() {
        let offset = nonnegative_i64(buffer.offset(), SpatialArrowFailure::InvalidBuffers)?;
        let length = nonnegative_i64(buffer.length(), SpatialArrowFailure::InvalidBuffers)?;
        if offset != align(next)?
            || offset % ALIGNMENT != 0
            || length != expected
            || body
                .get(next..offset)
                .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
        {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
        }
        let end = offset
            .checked_add(length)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if end > body.len() {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
        }
        ranges[index] = (offset, end);
        next = end;
        decoded = decoded
            .checked_add(u64::try_from(length).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    if align(next)? != body.len() || body[next..].iter().any(|byte| *byte != 0) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    for index in [0, 3, 4, 6] {
        let (begin, end) = ranges[index];
        if body[begin..end].iter().any(|byte| *byte != 0xff) {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
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
    let dimension =
        usize::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    for local in 0..rows {
        let row = table
            .row(start + local)
            .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidCanonicalRows))?;
        for column in 0..dimension {
            let index = local
                .checked_mul(dimension)
                .and_then(|value| value.checked_add(column))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let begin = index
                .checked_mul(size_of::<f32>())
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let end = begin
                .checked_add(size_of::<f32>())
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let observed = f32::from_le_bytes(
                value_bytes
                    .get(begin..end)
                    .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidCanonicalRows))?
                    .try_into()
                    .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidCanonicalRows))?,
            );
            let expected = row.vector().map_or(0.0, |vector| vector[column]);
            if observed.to_bits() != expected.to_bits() {
                return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
            }
        }
        accumulator.push(row.id(), row.status(), row.vector())?;
    }
    validate_zero_message_padding(
        message_bytes,
        canonical_record_message_len(batch, declared_body_len)?,
    )?;
    *decoded_total = decoded_total
        .checked_add(decoded)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    Ok(())
}

fn validate_record_batch_features(
    batch: arrow_ipc::RecordBatch<'_>,
) -> Result<(), MultiscaleColumnarError> {
    if batch.compression().is_some()
        || batch
            .variadicBufferCounts()
            .is_some_and(|counts| !counts.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::ForbiddenFeature));
    }
    Ok(())
}

fn validate_string_column<'a>(
    body: &[u8],
    offset_range: (usize, usize),
    value_range: (usize, usize),
    rows: usize,
    mut expected: impl FnMut(usize) -> Option<&'a str>,
) -> Result<(), MultiscaleColumnarError> {
    let offsets = &body[offset_range.0..offset_range.1];
    let values = &body[value_range.0..value_range.1];
    let mut prior = 0_usize;
    for index in 0..=rows {
        let begin = index
            .checked_mul(size_of::<i32>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let end = begin
            .checked_add(size_of::<i32>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let current = usize::try_from(i32::from_le_bytes(
            offsets
                .get(begin..end)
                .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidBuffers))?,
        ))
        .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidBuffers))?;
        if index == 0 {
            if current != 0 {
                return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
            }
        } else {
            let value = expected(index - 1)
                .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidCanonicalRows))?;
            if values.get(prior..current) != Some(value.as_bytes()) {
                return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
            }
        }
        if current < prior || current > values.len() {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
        }
        prior = current;
    }
    if prior != values.len() {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(())
}

fn validate_schema_message<R: std::io::Read + std::io::Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    table: &dyn MultiscaleMatrixTable,
    expected_schema: &arrow::datatypes::Schema,
) -> Result<usize, MultiscaleColumnarError> {
    let mut prefix = [0_u8; 8];
    read_exact_at(
        reader,
        file_len,
        HEADER_BYTES,
        &mut prefix,
        SpatialArrowFailure::InvalidMessage,
    )?;
    if &prefix[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let declared = nonnegative_i32(
        i32::from_le_bytes(
            prefix[4..]
                .try_into()
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?,
        ),
        SpatialArrowFailure::InvalidMessage,
    )?;
    let metadata_len = declared
        .checked_add(8)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let end = HEADER_BYTES
        .checked_add(metadata_len)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if declared == 0
        || metadata_len > MAXIMUM_MESSAGE_BYTES
        || metadata_len % ALIGNMENT != 0
        || end > footer_start.saturating_sub(EOS_BYTES)
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let bytes = read_vec_at(
        reader,
        file_len,
        HEADER_BYTES,
        metadata_len,
        SpatialArrowFailure::InvalidMessage,
    )?;
    let message = root_as_message_with_opts(&schema_message_verifier_options(), &bytes[8..])
        .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?;
    if message.version() != MetadataVersion::V5
        || message.header_type() != MessageHeader::Schema
        || message.bodyLength() != 0
        || message
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    validate_message_padding(&bytes)?;
    let observed = message
        .header_as_schema()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidSchema))?;
    validate_flatbuffer_schema(observed, table)?;
    if fb_to_schema(observed) != *expected_schema {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    validate_zero_message_padding(&bytes, canonical_schema_message_len(expected_schema)?)?;
    Ok(end)
}

fn validate_message_padding(bytes: &[u8]) -> Result<(), MultiscaleColumnarError> {
    if bytes.len() < 8 || &bytes[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let declared = nonnegative_i32(
        i32::from_le_bytes(
            bytes[4..8]
                .try_into()
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?,
        ),
        SpatialArrowFailure::InvalidMessage,
    )?;
    let end = 8_usize
        .checked_add(declared)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if declared == 0 || end != bytes.len() {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    Ok(())
}

fn validate_zero_message_padding(
    bytes: &[u8],
    canonical_flatbuffer_len: usize,
) -> Result<(), MultiscaleColumnarError> {
    let padding_start = 8_usize
        .checked_add(canonical_flatbuffer_len)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if bytes
        .get(padding_start..)
        .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    Ok(())
}

fn canonical_schema_message_len(
    schema: &arrow::datatypes::Schema,
) -> Result<usize, MultiscaleColumnarError> {
    let options = IpcWriteOptions::try_new(ALIGNMENT, false, MetadataVersion::V5)
        .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?;
    let mut tracker = DictionaryTracker::new(true);
    let encoded =
        IpcDataGenerator {}.schema_to_bytes_with_dictionary_tracker(schema, &mut tracker, &options);
    if !encoded.arrow_data.is_empty() || encoded.ipc_message.len() > MAXIMUM_MESSAGE_BYTES {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    Ok(encoded.ipc_message.len())
}

fn canonical_record_message_len(
    batch: arrow_ipc::RecordBatch<'_>,
    body_len: i64,
) -> Result<usize, MultiscaleColumnarError> {
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFieldNodes))?
        .iter()
        .map(|node| FieldNode::new(node.length(), node.null_count()))
        .collect::<Vec<_>>();
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?
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
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    Ok(len)
}

fn read_footer<R: std::io::Read + std::io::Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(usize, usize, Vec<u8>), MultiscaleColumnarError> {
    let minimum = HEADER_BYTES
        .checked_add(EOS_BYTES)
        .and_then(|value| value.checked_add(TRAILER_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if file_len < minimum {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMagic));
    }
    let mut header = [0_u8; HEADER_BYTES];
    read_exact_at(
        reader,
        file_len,
        0,
        &mut header,
        SpatialArrowFailure::InvalidMagic,
    )?;
    if header.get(..ARROW_MAGIC.len()) != Some(ARROW_MAGIC)
        || header[ARROW_MAGIC.len()..].iter().any(|byte| *byte != 0)
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMagic));
    }
    let trailer_start = file_len
        .checked_sub(TRAILER_BYTES)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let mut trailer = [0_u8; TRAILER_BYTES];
    read_exact_at(
        reader,
        file_len,
        trailer_start,
        &mut trailer,
        SpatialArrowFailure::InvalidFooterLength,
    )?;
    if trailer.get(4..) != Some(ARROW_MAGIC) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMagic));
    }
    let footer_len = usize::try_from(i32::from_le_bytes(
        trailer[..4]
            .try_into()
            .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidFooterLength))?,
    ))
    .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidFooterLength))?;
    if footer_len == 0 || footer_len > MAXIMUM_FOOTER_BYTES {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFooterLength));
    }
    let footer_start = trailer_start
        .checked_sub(footer_len)
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFooterLength))?;
    let retained = footer_len
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let footer = read_vec_at(
        reader,
        file_len,
        footer_start,
        footer_len,
        SpatialArrowFailure::InvalidFooter,
    )?;
    Ok((footer_start, footer_len, footer))
}

fn validate_eos<R: std::io::Read + std::io::Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    footer_start: usize,
) -> Result<(), MultiscaleColumnarError> {
    if offset.checked_add(EOS_BYTES) != Some(footer_start) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBlock));
    }
    let mut eos = [0_u8; EOS_BYTES];
    read_exact_at(
        reader,
        file_len,
        offset,
        &mut eos,
        SpatialArrowFailure::InvalidMessage,
    )?;
    if eos != [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0] {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    Ok(())
}

fn id_bytes(
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    rows: usize,
) -> Result<usize, MultiscaleColumnarError> {
    (0..rows).try_fold(0_usize, |total, local| {
        total
            .checked_add(
                table
                    .row(start + local)
                    .map_err(|_| MultiscaleColumnarError::ArtifactBindingMismatch)?
                    .id()
                    .len(),
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })
}

fn status_bytes(
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    rows: usize,
) -> Result<usize, MultiscaleColumnarError> {
    (0..rows).try_fold(0_usize, |total, local| {
        total
            .checked_add(
                table
                    .row(start + local)
                    .map_err(|_| MultiscaleColumnarError::ArtifactBindingMismatch)?
                    .status()
                    .wire_name()
                    .len(),
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })
}

fn validity_bytes(bits: usize) -> Result<usize, MultiscaleColumnarError> {
    bits.checked_add(7)
        .map(|value| value / 8)
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn offsets_bytes(rows: usize) -> Result<usize, MultiscaleColumnarError> {
    rows.checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn align(value: usize) -> Result<usize, MultiscaleColumnarError> {
    value
        .checked_add(ALIGNMENT - 1)
        .map(|value| value & !(ALIGNMENT - 1))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn nonnegative_i32(
    value: i32,
    reason: SpatialArrowFailure,
) -> Result<usize, MultiscaleColumnarError> {
    usize::try_from(value).map_err(|_| arrow_failure(reason))
}

fn nonnegative_i64(
    value: i64,
    reason: SpatialArrowFailure,
) -> Result<usize, MultiscaleColumnarError> {
    usize::try_from(value).map_err(|_| arrow_failure(reason))
}

fn read_vec_at<R: std::io::Read + std::io::Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    length: usize,
    reason: SpatialArrowFailure,
) -> Result<Vec<u8>, MultiscaleColumnarError> {
    let end = offset
        .checked_add(length)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if end > file_len {
        return Err(arrow_failure(reason));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| MultiscaleColumnarError::AllocationFailed { requested: length })?;
    bytes.resize(length, 0);
    read_exact_at(reader, file_len, offset, &mut bytes, reason)?;
    Ok(bytes)
}

fn read_exact_at<R: std::io::Read + std::io::Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    bytes: &mut [u8],
    reason: SpatialArrowFailure,
) -> Result<(), MultiscaleColumnarError> {
    let end = offset
        .checked_add(bytes.len())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if end > file_len {
        return Err(arrow_failure(reason));
    }
    reader
        .seek(std::io::SeekFrom::Start(
            u64::try_from(offset).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        ))
        .and_then(|_| reader.read_exact(bytes))
        .map_err(|_| arrow_failure(reason))
}

#[cfg(test)]
mod tests {
    use arrow_ipc::{
        root_as_message, BodyCompressionBuilder, CompressionType, MessageBuilder, MessageHeader,
        MetadataVersion, RecordBatchBuilder,
    };
    use flatbuffers::{FlatBufferBuilder, WIPOffset};

    use super::{validate_record_batch_features, MultiscaleColumnarError, SpatialArrowFailure};

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
                Err(MultiscaleColumnarError::Arrow {
                    reason: SpatialArrowFailure::ForbiddenFeature,
                })
            );
        }
    }
}
