use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow_ipc::{
    convert::fb_to_schema, root_as_footer_with_opts, root_as_message_with_opts, MessageHeader,
    MetadataVersion,
};
use marklab_project::ContentDigest;

use crate::PatchRegionLink;

use super::super::physical::{
    align, checked_range, nonnegative_i32, nonnegative_i64, read_exact_at, read_footer,
    read_record_batch_block, read_vec_at, validity_bytes,
};
use super::profile::{
    arrow_failure, schema, validate_flatbuffer_schema, BUFFER_COUNT, FIELD_COUNT,
};
use crate::columnar::{
    arrow::profile::{
        footer_verifier_options, message_verifier_options, schema_message_verifier_options,
        ALIGNMENT, CONTINUATION_MARKER, EOS_BYTES, HEADER_BYTES, MAXIMUM_MESSAGE_BYTES,
        RECORD_BATCH_ROWS,
    },
    multiscale::{
        enforce_decoded_budget, enforce_file_budget, enforce_retained_budget,
        patch_region_decoded_bytes, validate_patch_region_domain,
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
};

/// Validated structural declaration for one canonical patch-region Arrow table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchRegionArrowPreflight {
    row_count: u64,
    record_batch_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) maximum_batch_decoded_bytes: usize,
}

impl PatchRegionArrowPreflight {
    /// Exact canonical sparse nonzero row count.
    pub fn row_count(self) -> u64 {
        self.row_count
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

/// Validate borrowed patch-region Arrow bytes through raw bounded preflight only.
pub fn preflight_patch_region_link_arrow_bytes(
    bytes: &[u8],
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionArrowPreflight, MultiscaleColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    preflight_reader(
        &mut Cursor::new(bytes),
        ContentDigest::from_bytes(bytes),
        link,
        budgets,
    )
}

pub(super) fn preflight_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    content_digest: ContentDigest,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionArrowPreflight, MultiscaleColumnarError> {
    let encoded_byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    validate_patch_region_domain(link)?;
    let row_count = link.nonzero_relation_count();
    let expected_decoded = aggregate_decoded(link)?;
    enforce_decoded_budget(expected_decoded, budgets)?;
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
    validate_flatbuffer_schema(footer_schema, link)?;
    let expected_schema = schema(link)?;
    if fb_to_schema(footer_schema) != expected_schema {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    let schema_end =
        validate_schema_message(reader, file_len, footer_start, link, &expected_schema)?;
    let blocks = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFooter))?;
    if blocks.len() != row_count.div_ceil(RECORD_BATCH_ROWS) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    let footer_peak = footer_len
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(footer_peak, budgets)?;
    let mut retained = footer_peak;
    let mut decoded_total = 0_u64;
    let mut maximum_batch_decoded = 0_u64;
    let mut next_offset = schema_end;
    let mut aggregate_rows = 0_usize;
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
        let rows = row_count.saturating_sub(start).min(RECORD_BATCH_ROWS);
        let decoded =
            validate_record_message(message, body, block.bodyLength(), link, start, rows)?;
        decoded_total = decoded_total
            .checked_add(decoded)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum_batch_decoded = maximum_batch_decoded.max(decoded);
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        next_offset = physical.end;
    }
    if next_offset != footer_start.saturating_sub(EOS_BYTES) || aggregate_rows != row_count {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    if decoded_total != expected_decoded {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(PatchRegionArrowPreflight {
        row_count: u64::try_from(row_count).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        record_batch_count: u32::try_from(blocks.len())
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: retained,
        maximum_batch_decoded_bytes: usize::try_from(maximum_batch_decoded)
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
    })
}

fn validate_schema_message<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    link: &PatchRegionLink,
    expected: &arrow::datatypes::Schema,
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
    let schema = message
        .header_as_schema()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidSchema))?;
    validate_flatbuffer_schema(schema, link)?;
    if fb_to_schema(schema) != *expected {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    Ok(end)
}

fn validate_record_message(
    message_bytes: &[u8],
    body: &[u8],
    declared_body_len: i64,
    link: &PatchRegionLink,
    start: usize,
    rows: usize,
) -> Result<u64, MultiscaleColumnarError> {
    if message_bytes.len() < 8 || &message_bytes[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let declared = nonnegative_i32(
        i32::from_le_bytes(
            message_bytes[4..8]
                .try_into()
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?,
        ),
        SpatialArrowFailure::InvalidMessage,
    )?;
    if declared == 0
        || declared.checked_add(8) != Some(message_bytes.len())
        || declared > MAXIMUM_MESSAGE_BYTES - 8
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let message = root_as_message_with_opts(&message_verifier_options(), &message_bytes[8..])
        .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?;
    if message.version() != MetadataVersion::V5
        || message.header_type() != MessageHeader::RecordBatch
        || message.bodyLength() != declared_body_len
        || usize::try_from(message.bodyLength()).ok() != Some(body.len())
        || message
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let batch = message
        .header_as_record_batch()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidMessage))?;
    validate_record_batch_features(batch)?;
    if nonnegative_i64(batch.length(), SpatialArrowFailure::InvalidRowCount)? != rows
        || rows == 0
        || rows > RECORD_BATCH_ROWS
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFieldNodes))?;
    if nodes.len() != FIELD_COUNT
        || nodes
            .iter()
            .any(|node| usize::try_from(node.length()).ok() != Some(rows) || node.null_count() != 0)
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFieldNodes));
    }
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?;
    if buffers.len() != BUFFER_COUNT {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    let lengths = expected_lengths(link, start, rows)?;
    let ranges = validate_ranges(buffers, body, &lengths)?;
    validate_rows(body, &ranges, link, start, rows)?;
    lengths.iter().try_fold(0_u64, |total, length| {
        total
            .checked_add(u64::try_from(*length).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })
}

fn expected_lengths(
    link: &PatchRegionLink,
    start: usize,
    rows: usize,
) -> Result<[usize; BUFFER_COUNT], MultiscaleColumnarError> {
    let selected = link
        .nonzero_relations()
        .get(checked_range(start, rows)?)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let validity = validity_bytes(rows)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(size_of::<u64>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (patches, regions, relations) =
        selected
            .iter()
            .try_fold((0_usize, 0_usize, 0_usize), |totals, row| {
                Ok::<_, MultiscaleColumnarError>((
                    totals
                        .0
                        .checked_add(row.patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    totals
                        .1
                        .checked_add(row.region_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    totals
                        .2
                        .checked_add(row.relation().wire_name().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                ))
            })?;
    Ok([
        validity, offsets, patches, validity, offsets, regions, validity, offsets, relations,
        validity, values, validity, values,
    ])
}

fn validate_ranges(
    buffers: flatbuffers::Vector<'_, arrow_ipc::Buffer>,
    body: &[u8],
    lengths: &[usize],
) -> Result<Vec<std::ops::Range<usize>>, MultiscaleColumnarError> {
    let mut ranges = Vec::new();
    ranges.try_reserve_exact(buffers.len()).map_err(|_| {
        MultiscaleColumnarError::AllocationFailed {
            requested: buffers
                .len()
                .saturating_mul(size_of::<std::ops::Range<usize>>()),
        }
    })?;
    let mut next = 0_usize;
    for (buffer, expected) in buffers.iter().zip(lengths) {
        let offset = nonnegative_i64(buffer.offset(), SpatialArrowFailure::InvalidBuffers)?;
        let length = nonnegative_i64(buffer.length(), SpatialArrowFailure::InvalidBuffers)?;
        let aligned = align(next)?;
        if offset != aligned || offset % ALIGNMENT != 0 || length != *expected {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
        }
        if offset > body.len() || body[next..offset].iter().any(|byte| *byte != 0) {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
        }
        let end = offset
            .checked_add(length)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if end > body.len() {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
        }
        ranges.push(offset..end);
        next = end;
    }
    let aligned = align(next)?;
    if aligned != body.len() || body[next..aligned].iter().any(|byte| *byte != 0) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(ranges)
}

fn validate_rows(
    body: &[u8],
    ranges: &[std::ops::Range<usize>],
    link: &PatchRegionLink,
    start: usize,
    rows: usize,
) -> Result<(), MultiscaleColumnarError> {
    let selected = link
        .nonzero_relations()
        .get(checked_range(start, rows)?)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    for index in [0, 3, 6, 9, 11] {
        if body[ranges[index].clone()]
            .iter()
            .any(|byte| *byte != u8::MAX)
        {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
        }
    }
    validate_utf8(
        &body[ranges[1].clone()],
        &body[ranges[2].clone()],
        selected.iter().map(|row| row.patch_id().as_str()),
    )?;
    validate_utf8(
        &body[ranges[4].clone()],
        &body[ranges[5].clone()],
        selected.iter().map(|row| row.region_id().as_str()),
    )?;
    validate_utf8(
        &body[ranges[7].clone()],
        &body[ranges[8].clone()],
        selected.iter().map(|row| row.relation().wire_name()),
    )?;
    validate_u64(
        &body[ranges[10].clone()],
        selected.iter().map(|row| row.numerator()),
    )?;
    validate_u64(
        &body[ranges[12].clone()],
        selected.iter().map(|row| row.denominator()),
    )
}

fn validate_utf8<'a>(
    offsets: &[u8],
    values: &[u8],
    expected: impl Iterator<Item = &'a str>,
) -> Result<(), MultiscaleColumnarError> {
    let mut offset = 0_usize;
    let mut count = 0_usize;
    for (index, value) in expected.enumerate() {
        let at = index
            .checked_mul(size_of::<i32>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let observed = read_i32(offsets, at)?;
        if usize::try_from(observed).ok() != Some(offset) {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        let end = offset
            .checked_add(value.len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if values.get(offset..end) != Some(value.as_bytes()) {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        offset = end;
        count = index
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    let terminal = count
        .checked_mul(size_of::<i32>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if usize::try_from(read_i32(offsets, terminal)?).ok() != Some(offset)
        || terminal.checked_add(size_of::<i32>()) != Some(offsets.len())
        || offset != values.len()
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
    }
    Ok(())
}

fn validate_u64(
    bytes: &[u8],
    expected: impl Iterator<Item = u64>,
) -> Result<(), MultiscaleColumnarError> {
    let mut count = 0_usize;
    for (index, expected) in expected.enumerate() {
        let at = index
            .checked_mul(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let end = at
            .checked_add(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let observed = u64::from_le_bytes(
            bytes
                .get(at..end)
                .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidBuffers))?,
        );
        if observed != expected {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        count = index + 1;
    }
    if bytes.len()
        != count
            .checked_mul(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(())
}

fn read_i32(bytes: &[u8], at: usize) -> Result<i32, MultiscaleColumnarError> {
    let end = at
        .checked_add(size_of::<i32>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    Ok(i32::from_le_bytes(
        bytes
            .get(at..end)
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?
            .try_into()
            .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidBuffers))?,
    ))
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

fn aggregate_decoded(link: &PatchRegionLink) -> Result<u64, MultiscaleColumnarError> {
    link.nonzero_relations()
        .chunks(RECORD_BATCH_ROWS)
        .try_fold(0_u64, |total, rows| {
            total
                .checked_add(
                    u64::try_from(patch_region_decoded_bytes(rows)?)
                        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
                )
                .ok_or(MultiscaleColumnarError::SizeOverflow)
        })
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
    fn feature_guard_rejects_compression_and_variadic_buffers() {
        for (compression, variadic) in [(true, false), (false, true)] {
            let mut builder = FlatBufferBuilder::new();
            let variadic = variadic.then(|| builder.create_vector(&[1_i64]));
            let compression = compression.then(|| {
                let mut value = BodyCompressionBuilder::new(&mut builder);
                value.add_codec(CompressionType::LZ4_FRAME);
                value.finish()
            });
            let batch = {
                let mut value = RecordBatchBuilder::new(&mut builder);
                if let Some(compression) = compression {
                    value.add_compression(compression);
                }
                if let Some(variadic) = variadic {
                    value.add_variadicBufferCounts(variadic);
                }
                value.finish()
            };
            let message = {
                let mut value = MessageBuilder::new(&mut builder);
                value.add_version(MetadataVersion::V5);
                value.add_header_type(MessageHeader::RecordBatch);
                value.add_header(WIPOffset::new(batch.value()));
                value.finish()
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
