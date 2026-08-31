use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow_ipc::{
    convert::fb_to_schema, root_as_footer_with_opts, root_as_message_with_opts, MessageHeader,
    MetadataVersion,
};
use marklab_project::ContentDigest;

use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

use super::physical::{
    align, checked_range, nonnegative_i32, nonnegative_i64, read_exact_at, read_footer,
    read_record_batch_block, read_vec_at, validate_record_batch_features, validity_bytes,
};
use super::{
    super::profile::{
        footer_verifier_options, message_verifier_options, schema_message_verifier_options,
        ALIGNMENT, CONTINUATION_MARKER, EOS_BYTES, HEADER_BYTES, MAXIMUM_MESSAGE_BYTES,
        RECORD_BATCH_ROWS,
    },
    profile::{arrow_failure, expected_schema, validate_flatbuffer_schema, SpatialArrowProfile},
};
use crate::columnar::{
    multiscale::{enforce_decoded_budget, enforce_file_budget, enforce_retained_budget},
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
};

macro_rules! preflight_summary {
    ($name:ident) => {
        #[doc = "Validated structural declaration for one canonical C-05 Arrow table."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name {
            row_count: u64,
            record_batch_count: u32,
            encoded_byte_len: u64,
            content_digest: ContentDigest,
            pub(super) retained_preflight_bytes: usize,
            pub(super) maximum_batch_decoded_bytes: usize,
        }

        impl $name {
            pub(super) fn from_common(common: CommonArrowPreflight) -> Self {
                Self {
                    row_count: common.row_count,
                    record_batch_count: common.record_batch_count,
                    encoded_byte_len: common.encoded_byte_len,
                    content_digest: common.content_digest,
                    retained_preflight_bytes: common.retained_preflight_bytes,
                    maximum_batch_decoded_bytes: common.maximum_batch_decoded_bytes,
                }
            }

            /// Exact canonical row count.
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
    };
}

preflight_summary!(PatchFootprintArrowPreflight);
preflight_summary!(PatchOverlapArrowPreflight);

#[derive(Clone, Copy)]
pub(super) struct CommonArrowPreflight {
    pub(super) row_count: u64,
    pub(super) record_batch_count: u32,
    pub(super) encoded_byte_len: u64,
    pub(super) content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) maximum_batch_decoded_bytes: usize,
}

/// Validate borrowed footprint Arrow bytes through raw bounded preflight only.
pub fn preflight_patch_footprint_set_arrow_bytes(
    bytes: &[u8],
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintArrowPreflight, MultiscaleColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    preflight_reader(
        &mut Cursor::new(bytes),
        ContentDigest::from_bytes(bytes),
        SpatialArrowProfile::Footprint,
        expected,
        context,
        footprints,
        None,
        budgets,
    )
    .map(PatchFootprintArrowPreflight::from_common)
}

/// Validate borrowed overlap Arrow bytes through raw bounded preflight only.
pub fn preflight_patch_overlap_graph_arrow_bytes(
    bytes: &[u8],
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapArrowPreflight, MultiscaleColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    preflight_reader(
        &mut Cursor::new(bytes),
        ContentDigest::from_bytes(bytes),
        SpatialArrowProfile::Overlap,
        expected,
        context,
        footprints,
        Some(overlap),
        budgets,
    )
    .map(PatchOverlapArrowPreflight::from_common)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn preflight_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    content_digest: ContentDigest,
    profile: SpatialArrowProfile,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CommonArrowPreflight, MultiscaleColumnarError> {
    let encoded_byte_len = reader
        .seek(SeekFrom::End(0))
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
    validate_footer_features(footer)?;
    let footer_schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidSchema))?;
    validate_flatbuffer_schema(
        footer_schema,
        profile,
        expected,
        context,
        footprints,
        overlap,
    )?;
    let schema = expected_schema(profile, expected, context, footprints, overlap)?;
    if fb_to_schema(footer_schema) != schema {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    let row_count = match profile {
        SpatialArrowProfile::Footprint => footprints.row_count(),
        SpatialArrowProfile::Overlap => overlap
            .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
            .edge_count(),
    };
    let expected_decoded_bytes = decoded_bytes(profile, footprints, overlap, RECORD_BATCH_ROWS)?;
    enforce_decoded_budget(expected_decoded_bytes, budgets)?;
    let schema_end = validate_schema_message(
        reader,
        file_len,
        footer_start,
        profile,
        expected,
        context,
        footprints,
        overlap,
        &schema,
    )?;
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
        let batch_start = batch_index
            .checked_mul(RECORD_BATCH_ROWS)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let batch_rows = row_count.saturating_sub(batch_start).min(RECORD_BATCH_ROWS);
        let before = decoded_total;
        validate_record_message(
            message,
            body,
            block.bodyLength(),
            profile,
            batch_start,
            batch_rows,
            footprints,
            overlap,
            &mut decoded_total,
        )?;
        maximum_batch_decoded = maximum_batch_decoded.max(
            decoded_total
                .checked_sub(before)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
        aggregate_rows = aggregate_rows
            .checked_add(batch_rows)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        next_offset = physical.end;
    }
    if next_offset != footer_start.saturating_sub(EOS_BYTES) || aggregate_rows != row_count {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    if decoded_total != expected_decoded_bytes {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(CommonArrowPreflight {
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

#[allow(clippy::too_many_arguments)]
fn validate_schema_message<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    profile: SpatialArrowProfile,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    expected_schema_value: &arrow::datatypes::Schema,
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
    let declared = i32::from_le_bytes(
        prefix[4..]
            .try_into()
            .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?,
    );
    let declared = nonnegative_i32(declared, SpatialArrowFailure::InvalidMessage)?;
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
    validate_flatbuffer_schema(schema, profile, expected, context, footprints, overlap)?;
    if fb_to_schema(schema) != *expected_schema_value {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    Ok(end)
}

#[allow(clippy::too_many_arguments)]
fn validate_record_message(
    message_bytes: &[u8],
    body: &[u8],
    declared_body_len: i64,
    profile: SpatialArrowProfile,
    start: usize,
    rows: usize,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    decoded_total: &mut u64,
) -> Result<(), MultiscaleColumnarError> {
    if message_bytes.len() < 8 || &message_bytes[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMessage));
    }
    let declared_metadata = i32::from_le_bytes(
        message_bytes[4..8]
            .try_into()
            .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidMessage))?,
    );
    let declared_metadata =
        nonnegative_i32(declared_metadata, SpatialArrowFailure::InvalidMessage)?;
    if declared_metadata == 0
        || declared_metadata.checked_add(8) != Some(message_bytes.len())
        || declared_metadata > MAXIMUM_MESSAGE_BYTES - 8
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
    if nodes.len() != profile.field_count()
        || nodes.iter().any(|node| {
            nonnegative_i64(node.length(), SpatialArrowFailure::InvalidFieldNodes).ok()
                != Some(rows)
                || node.null_count() != 0
        })
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFieldNodes));
    }
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?;
    if buffers.len() != profile.buffer_count() {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    let validity = validity_bytes(rows)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let expected_lengths = match profile {
        SpatialArrowProfile::Footprint => {
            let patch_bytes = footprint_text_bytes(footprints, start, rows)?;
            let value_bytes = rows
                .checked_mul(8)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            vec![
                validity,
                offsets,
                patch_bytes,
                validity,
                value_bytes,
                validity,
                value_bytes,
            ]
        }
        SpatialArrowProfile::Overlap => {
            let overlap = overlap.ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?;
            let (left, right) = overlap_text_bytes(overlap, start, rows)?;
            vec![validity, offsets, left, validity, offsets, right]
        }
    };
    let mut ranges = Vec::new();
    let range_bytes = buffers
        .len()
        .checked_mul(size_of::<std::ops::Range<usize>>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    ranges.try_reserve_exact(buffers.len()).map_err(|_| {
        MultiscaleColumnarError::AllocationFailed {
            requested: range_bytes,
        }
    })?;
    let mut next = 0_usize;
    let mut decoded = 0_u64;
    for (buffer, expected_len) in buffers.iter().zip(expected_lengths) {
        let offset = nonnegative_i64(buffer.offset(), SpatialArrowFailure::InvalidBuffers)?;
        let length = nonnegative_i64(buffer.length(), SpatialArrowFailure::InvalidBuffers)?;
        let aligned = align(next)?;
        if offset != aligned || offset % ALIGNMENT != 0 || length != expected_len {
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
        decoded = decoded
            .checked_add(u64::try_from(length).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    let aligned_end = align(next)?;
    if aligned_end != body.len() || body[next..aligned_end].iter().any(|byte| *byte != 0) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    match profile {
        SpatialArrowProfile::Footprint => {
            validate_all_ones(&body[ranges[0].clone()])?;
            validate_utf8_rows(
                &body[ranges[1].clone()],
                &body[ranges[2].clone()],
                footprints.footprints()[checked_range(start, rows)?]
                    .iter()
                    .map(|row| row.patch_id().as_str()),
            )?;
            validate_all_ones(&body[ranges[3].clone()])?;
            validate_i64_rows(
                &body[ranges[4].clone()],
                footprints.footprints()[checked_range(start, rows)?]
                    .iter()
                    .map(|row| row.origin_px()[0]),
            )?;
            validate_all_ones(&body[ranges[5].clone()])?;
            validate_i64_rows(
                &body[ranges[6].clone()],
                footprints.footprints()[checked_range(start, rows)?]
                    .iter()
                    .map(|row| row.origin_px()[1]),
            )?;
        }
        SpatialArrowProfile::Overlap => {
            let edges = &overlap
                .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
                .edges()[checked_range(start, rows)?];
            validate_all_ones(&body[ranges[0].clone()])?;
            validate_utf8_rows(
                &body[ranges[1].clone()],
                &body[ranges[2].clone()],
                edges.iter().map(|edge| edge.left_patch_id().as_str()),
            )?;
            validate_all_ones(&body[ranges[3].clone()])?;
            validate_utf8_rows(
                &body[ranges[4].clone()],
                &body[ranges[5].clone()],
                edges.iter().map(|edge| edge.right_patch_id().as_str()),
            )?;
        }
    }
    *decoded_total = decoded_total
        .checked_add(decoded)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    Ok(())
}

fn validate_all_ones(bytes: &[u8]) -> Result<(), MultiscaleColumnarError> {
    if bytes.iter().any(|byte| *byte != u8::MAX) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(())
}

fn validate_utf8_rows<'a>(
    offsets: &[u8],
    values: &[u8],
    rows: impl Iterator<Item = &'a str>,
) -> Result<(), MultiscaleColumnarError> {
    let mut expected_offset = 0_usize;
    let mut row_count = 0_usize;
    for (index, expected) in rows.enumerate() {
        let observed_start = read_i32(offsets, index)?;
        if observed_start != expected_offset {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        let end = expected_offset
            .checked_add(expected.len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if values.get(expected_offset..end) != Some(expected.as_bytes()) {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        expected_offset = end;
        row_count = index
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    let expected_offset_bytes = row_count
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if offsets.len() != expected_offset_bytes
        || read_i32(offsets, row_count)? != expected_offset
        || expected_offset != values.len()
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
    }
    Ok(())
}

fn validate_i64_rows(
    values: &[u8],
    rows: impl Iterator<Item = i64>,
) -> Result<(), MultiscaleColumnarError> {
    let mut count = 0_usize;
    for (index, expected) in rows.enumerate() {
        let start = index
            .checked_mul(8)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let end = start
            .checked_add(8)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let observed = i64::from_le_bytes(
            values
                .get(start..end)
                .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidBuffers))?,
        );
        if observed != expected {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        count = index + 1;
    }
    if values.len()
        != count
            .checked_mul(8)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(())
}

fn read_i32(bytes: &[u8], index: usize) -> Result<usize, MultiscaleColumnarError> {
    let start = index
        .checked_mul(4)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let end = start
        .checked_add(4)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let value = i32::from_le_bytes(
        bytes
            .get(start..end)
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?
            .try_into()
            .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidBuffers))?,
    );
    nonnegative_i32(value, SpatialArrowFailure::InvalidBuffers)
}

fn footprint_text_bytes(
    footprints: &PatchFootprintSet,
    start: usize,
    rows: usize,
) -> Result<usize, MultiscaleColumnarError> {
    footprints.footprints()[checked_range(start, rows)?]
        .iter()
        .try_fold(0_usize, |total, row| {
            total
                .checked_add(row.patch_id().as_str().len())
                .ok_or(MultiscaleColumnarError::SizeOverflow)
        })
}

fn overlap_text_bytes(
    overlap: &PatchOverlapGraph,
    start: usize,
    rows: usize,
) -> Result<(usize, usize), MultiscaleColumnarError> {
    overlap.edges()[checked_range(start, rows)?]
        .iter()
        .try_fold((0_usize, 0_usize), |(left, right), edge| {
            Ok((
                left.checked_add(edge.left_patch_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                right
                    .checked_add(edge.right_patch_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
            ))
        })
}

fn decoded_bytes(
    profile: SpatialArrowProfile,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    chunk_rows: usize,
) -> Result<u64, MultiscaleColumnarError> {
    let mut total = 0_u64;
    match profile {
        SpatialArrowProfile::Footprint => {
            for rows in footprints.footprints().chunks(chunk_rows) {
                let text = rows.iter().try_fold(0_usize, |sum, row| {
                    sum.checked_add(row.patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)
                })?;
                total = total
                    .checked_add(
                        u64::try_from(decoded_chunk(rows.len(), 1, text, 2)?)
                            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
                    )
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            }
        }
        SpatialArrowProfile::Overlap => {
            for rows in overlap
                .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
                .edges()
                .chunks(chunk_rows)
            {
                let text = rows.iter().try_fold(0_usize, |sum, row| {
                    sum.checked_add(row.left_patch_id().as_str().len())
                        .and_then(|value| value.checked_add(row.right_patch_id().as_str().len()))
                        .ok_or(MultiscaleColumnarError::SizeOverflow)
                })?;
                total = total
                    .checked_add(
                        u64::try_from(decoded_chunk(rows.len(), 2, text, 0)?)
                            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
                    )
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            }
        }
    }
    Ok(total)
}

fn decoded_chunk(
    rows: usize,
    utf8_columns: usize,
    text: usize,
    i64_columns: usize,
) -> Result<usize, MultiscaleColumnarError> {
    validity_bytes(rows)?
        .checked_mul(utf8_columns + i64_columns)
        .and_then(|value| {
            value.checked_add(
                rows.checked_add(1)?
                    .checked_mul(size_of::<i32>())?
                    .checked_mul(utf8_columns)?,
            )
        })
        .and_then(|value| value.checked_add(text))
        .and_then(|value| {
            value.checked_add(
                rows.checked_mul(size_of::<i64>())?
                    .checked_mul(i64_columns)?,
            )
        })
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn validate_footer_features(footer: arrow_ipc::Footer<'_>) -> Result<(), MultiscaleColumnarError> {
    if footer
        .custom_metadata()
        .is_some_and(|metadata| !metadata.is_empty())
        || footer
            .dictionaries()
            .is_some_and(|dictionaries| !dictionaries.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::ForbiddenFeature));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use arrow_ipc::{
        root_as_footer_with_opts, root_as_message_with_opts, Block, BodyCompression,
        BodyCompressionArgs, Footer, FooterArgs, KeyValue, KeyValueArgs, Message, MessageArgs,
        MessageHeader, MetadataVersion, RecordBatch, RecordBatchArgs,
    };
    use flatbuffers::FlatBufferBuilder;

    use super::*;
    use crate::columnar::arrow::profile::{ARROW_MAGIC, MAXIMUM_FOOTER_BYTES, TRAILER_BYTES};

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

    fn record_message(compression: bool, variadic: bool, metadata_tables: usize) -> Vec<u8> {
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
        let message = Message::create(
            &mut builder,
            &MessageArgs {
                version: MetadataVersion::V5,
                header_type: MessageHeader::RecordBatch,
                header: Some(batch.as_union_value()),
                bodyLength: 0,
                custom_metadata,
            },
        );
        builder.finish(message, None);
        builder.finished_data().to_vec()
    }

    #[test]
    fn c05_arrow_feature_guards_reject_dictionaries_compression_and_variadic_buffers() {
        let dictionary = footer_bytes(true, 0);
        let footer = arrow_ipc::root_as_footer(&dictionary).expect("dictionary footer");
        assert!(matches!(
            validate_footer_features(footer),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::ForbiddenFeature,
            })
        ));

        for bytes in [
            record_message(true, false, 0),
            record_message(false, true, 0),
        ] {
            let message = arrow_ipc::root_as_message(&bytes).expect("record message");
            let batch = message.header_as_record_batch().expect("record batch");
            assert!(matches!(
                validate_record_batch_features(batch),
                Err(MultiscaleColumnarError::Arrow {
                    reason: SpatialArrowFailure::ForbiddenFeature,
                })
            ));
        }
    }

    #[test]
    fn c05_arrow_verifier_and_footer_limits_reject_hostile_declarations() {
        let table_heavy_footer = footer_bytes(false, 40);
        assert!(
            root_as_footer_with_opts(&footer_verifier_options(), &table_heavy_footer,).is_err()
        );

        let table_heavy_message = record_message(false, false, 40);
        assert!(
            root_as_message_with_opts(&message_verifier_options(), &table_heavy_message,).is_err()
        );

        let mut oversized = vec![0_u8; HEADER_BYTES + EOS_BYTES + TRAILER_BYTES];
        oversized[..ARROW_MAGIC.len()].copy_from_slice(ARROW_MAGIC);
        let trailer = oversized.len() - TRAILER_BYTES;
        oversized[trailer..trailer + 4]
            .copy_from_slice(&((MAXIMUM_FOOTER_BYTES + 1) as i32).to_le_bytes());
        oversized[trailer + 4..].copy_from_slice(ARROW_MAGIC);
        let length = oversized.len();
        assert!(matches!(
            read_footer(
                &mut Cursor::new(oversized),
                length,
                EmbeddingColumnarBudgets::new(u64::MAX, usize::MAX, usize::MAX, u64::MAX),
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidFooterLength,
            })
        ));
    }
}
