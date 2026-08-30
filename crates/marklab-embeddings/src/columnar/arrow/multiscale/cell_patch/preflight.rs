use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow_ipc::{
    convert::fb_to_schema, root_as_footer_with_opts, root_as_message_with_opts, MessageHeader,
    MetadataVersion,
};
use marklab_project::ContentDigest;

use crate::{CellPatchAssignmentMode, CellPatchLink};

use super::super::physical::{
    align, checked_range, nonnegative_i32, nonnegative_i64, read_exact_at, read_footer,
    read_vec_at, validity_bytes,
};
use super::profile::{
    arrow_failure, expected_schema, validate_flatbuffer_schema, CellPatchArrowProfile,
};
use crate::columnar::{
    arrow::profile::{
        footer_verifier_options, message_verifier_options, schema_message_verifier_options,
        ALIGNMENT, CONTINUATION_MARKER, EOS_BYTES, HEADER_BYTES, MAXIMUM_MESSAGE_BYTES,
        RECORD_BATCH_ROWS,
    },
    multiscale::{
        assignment_decoded_bytes, edge_decoded_bytes, enforce_decoded_budget, enforce_file_budget,
        enforce_retained_budget, enforce_row_group_budget,
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
};

macro_rules! preflight_summary {
    ($name:ident) => {
        #[doc = "Validated structural declaration for one canonical cell-patch Arrow table."]
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

preflight_summary!(CellPatchAssignmentArrowPreflight);
preflight_summary!(CellPatchEdgeArrowPreflight);

#[derive(Clone, Copy)]
pub(super) struct CommonArrowPreflight {
    pub(super) row_count: u64,
    pub(super) record_batch_count: u32,
    pub(super) encoded_byte_len: u64,
    pub(super) content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) maximum_batch_decoded_bytes: usize,
}

/// Validate borrowed cell-patch assignment Arrow bytes through raw bounded preflight only.
pub fn preflight_cell_patch_assignment_table_arrow_bytes(
    bytes: &[u8],
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentArrowPreflight, MultiscaleColumnarError> {
    preflight_bytes(bytes, CellPatchArrowProfile::Assignment, link, budgets)
        .map(CellPatchAssignmentArrowPreflight::from_common)
}

/// Validate borrowed cell-patch edge Arrow bytes through raw bounded preflight only.
pub fn preflight_cell_patch_edge_table_arrow_bytes(
    bytes: &[u8],
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeArrowPreflight, MultiscaleColumnarError> {
    preflight_bytes(bytes, CellPatchArrowProfile::Edge, link, budgets)
        .map(CellPatchEdgeArrowPreflight::from_common)
}

fn preflight_bytes(
    bytes: &[u8],
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CommonArrowPreflight, MultiscaleColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    preflight_reader(
        &mut Cursor::new(bytes),
        ContentDigest::from_bytes(bytes),
        profile,
        link,
        budgets,
    )
}

pub(super) fn preflight_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    content_digest: ContentDigest,
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
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
    validate_flatbuffer_schema(footer_schema, profile, link)?;
    let schema = expected_schema(profile, link)?;
    if fb_to_schema(footer_schema) != schema {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    let row_count = profile.row_count(link);
    let expected_decoded_bytes = decoded_bytes(profile, link)?;
    enforce_decoded_budget(expected_decoded_bytes, budgets)?;
    let schema_end =
        validate_schema_message(reader, file_len, footer_start, profile, link, &schema)?;
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
        let offset = nonnegative_i64(block.offset(), SpatialArrowFailure::InvalidBlock)?;
        let metadata_len =
            nonnegative_i32(block.metaDataLength(), SpatialArrowFailure::InvalidBlock)?;
        let body_len = nonnegative_i64(block.bodyLength(), SpatialArrowFailure::InvalidBlock)?;
        if offset != next_offset
            || offset % ALIGNMENT != 0
            || !(8..=MAXIMUM_MESSAGE_BYTES).contains(&metadata_len)
            || metadata_len % ALIGNMENT != 0
            || body_len % ALIGNMENT != 0
        {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBlock));
        }
        let block_len = metadata_len
            .checked_add(body_len)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        enforce_row_group_budget(block_len, budgets)?;
        let block_peak = footer_len
            .checked_mul(2)
            .and_then(|value| value.checked_add(block_len))
            .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        retained = retained.max(block_peak);
        enforce_retained_budget(retained, budgets)?;
        let block_end = offset
            .checked_add(block_len)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if block_end > footer_start.saturating_sub(EOS_BYTES) {
            return Err(arrow_failure(SpatialArrowFailure::InvalidBlock));
        }
        let physical = read_vec_at(
            reader,
            file_len,
            offset,
            block_len,
            SpatialArrowFailure::InvalidBlock,
        )?;
        let (message, body) = physical.split_at(metadata_len);
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
            link,
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
        next_offset = block_end;
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

fn validate_schema_message<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
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
    validate_flatbuffer_schema(schema, profile, link)?;
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
    profile: CellPatchArrowProfile,
    start: usize,
    rows: usize,
    link: &CellPatchLink,
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
    validate_nodes(nodes, profile, rows, link.mode())?;
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?;
    if buffers.len() != profile.buffer_count() {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    let expected_lengths = expected_buffer_lengths(profile, link, start, rows)?;
    let ranges = validate_buffer_ranges(buffers, body, &expected_lengths)?;
    match profile {
        CellPatchArrowProfile::Assignment => {
            validate_assignment_rows(body, &ranges, link, start, rows)?
        }
        CellPatchArrowProfile::Edge => validate_edge_rows(body, &ranges, link, start, rows)?,
    }
    let decoded = expected_lengths.iter().try_fold(0_u64, |total, length| {
        total
            .checked_add(u64::try_from(*length).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    *decoded_total = decoded_total
        .checked_add(decoded)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    Ok(())
}

fn validate_nodes(
    nodes: flatbuffers::Vector<'_, arrow_ipc::FieldNode>,
    profile: CellPatchArrowProfile,
    rows: usize,
    mode: CellPatchAssignmentMode,
) -> Result<(), MultiscaleColumnarError> {
    if nodes.len() != profile.field_count() {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFieldNodes));
    }
    for (index, node) in nodes.iter().enumerate() {
        let expected_nulls = match (profile, index, mode) {
            (CellPatchArrowProfile::Edge, 2 | 3, CellPatchAssignmentMode::ContainedShared) => rows,
            _ => 0,
        };
        if nonnegative_i64(node.length(), SpatialArrowFailure::InvalidFieldNodes)? != rows
            || nonnegative_i64(node.null_count(), SpatialArrowFailure::InvalidFieldNodes)?
                != expected_nulls
        {
            return Err(arrow_failure(SpatialArrowFailure::InvalidFieldNodes));
        }
    }
    Ok(())
}

fn expected_buffer_lengths(
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
    start: usize,
    rows: usize,
) -> Result<Vec<usize>, MultiscaleColumnarError> {
    let validity = validity_bytes(rows)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(size_of::<u64>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    match profile {
        CellPatchArrowProfile::Assignment => {
            let selected = &link.assignments()[checked_range(start, rows)?];
            let (cells, statuses) = selected.iter().try_fold(
                (0_usize, 0_usize),
                |(cell_total, status_total), row| {
                    Ok::<_, MultiscaleColumnarError>((
                        cell_total
                            .checked_add(row.cell_id().as_str().len())
                            .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                        status_total
                            .checked_add(row.status().wire_name().len())
                            .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    ))
                },
            )?;
            Ok(vec![
                validity, offsets, cells, validity, offsets, statuses, validity, values, validity,
                values, validity, values, validity, values,
            ])
        }
        CellPatchArrowProfile::Edge => {
            let selected = &link.edges()[checked_range(start, rows)?];
            let patches = selected.iter().try_fold(0_usize, |total, row| {
                total
                    .checked_add(row.patch_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)
            })?;
            Ok(vec![
                validity, values, validity, offsets, patches, validity, values, validity, values,
            ])
        }
    }
}

fn validate_buffer_ranges(
    buffers: flatbuffers::Vector<'_, arrow_ipc::Buffer>,
    body: &[u8],
    expected_lengths: &[usize],
) -> Result<Vec<std::ops::Range<usize>>, MultiscaleColumnarError> {
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
    for (buffer, expected_len) in buffers.iter().zip(expected_lengths) {
        let offset = nonnegative_i64(buffer.offset(), SpatialArrowFailure::InvalidBuffers)?;
        let length = nonnegative_i64(buffer.length(), SpatialArrowFailure::InvalidBuffers)?;
        let aligned = align(next)?;
        if offset != aligned || offset % ALIGNMENT != 0 || length != *expected_len {
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
    let aligned_end = align(next)?;
    if aligned_end != body.len() || body[next..aligned_end].iter().any(|byte| *byte != 0) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(ranges)
}

fn validate_assignment_rows(
    body: &[u8],
    ranges: &[std::ops::Range<usize>],
    link: &CellPatchLink,
    start: usize,
    rows: usize,
) -> Result<(), MultiscaleColumnarError> {
    let selected = &link.assignments()[checked_range(start, rows)?];
    for index in [0, 3, 6, 8, 10, 12] {
        validate_all_ones(&body[ranges[index].clone()])?;
    }
    validate_utf8_rows(
        &body[ranges[1].clone()],
        &body[ranges[2].clone()],
        selected.iter().map(|row| row.cell_id().as_str()),
    )?;
    validate_utf8_rows(
        &body[ranges[4].clone()],
        &body[ranges[5].clone()],
        selected.iter().map(|row| row.status().wire_name()),
    )?;
    validate_u64_rows(
        &body[ranges[7].clone()],
        selected.iter().map(|row| row.anchor_px()[0].to_bits()),
    )?;
    validate_u64_rows(
        &body[ranges[9].clone()],
        selected.iter().map(|row| row.anchor_px()[1].to_bits()),
    )?;
    validate_u64_rows(
        &body[ranges[11].clone()],
        selected.iter().map(|row| row.edge_start()),
    )?;
    validate_u64_rows(
        &body[ranges[13].clone()],
        selected.iter().map(|row| row.edge_count()),
    )
}

fn validate_edge_rows(
    body: &[u8],
    ranges: &[std::ops::Range<usize>],
    link: &CellPatchLink,
    start: usize,
    rows: usize,
) -> Result<(), MultiscaleColumnarError> {
    let selected = &link.edges()[checked_range(start, rows)?];
    validate_all_ones(&body[ranges[0].clone()])?;
    validate_u64_rows(
        &body[ranges[1].clone()],
        selected.iter().map(|row| row.assignment_row()),
    )?;
    validate_all_ones(&body[ranges[2].clone()])?;
    validate_utf8_rows(
        &body[ranges[3].clone()],
        &body[ranges[4].clone()],
        selected.iter().map(|row| row.patch_id().as_str()),
    )?;
    validate_weight_bitmap(&body[ranges[5].clone()], rows, link.mode())?;
    validate_weight_bitmap(&body[ranges[7].clone()], rows, link.mode())?;
    validate_u64_rows(
        &body[ranges[6].clone()],
        selected
            .iter()
            .map(|row| row.weight().map_or(0, |weight| weight.numerator())),
    )?;
    validate_u64_rows(
        &body[ranges[8].clone()],
        selected
            .iter()
            .map(|row| row.weight().map_or(0, |weight| weight.denominator())),
    )
}

fn validate_all_ones(bytes: &[u8]) -> Result<(), MultiscaleColumnarError> {
    if bytes.iter().any(|byte| *byte != u8::MAX) {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(())
}

fn validate_weight_bitmap(
    bytes: &[u8],
    rows: usize,
    mode: CellPatchAssignmentMode,
) -> Result<(), MultiscaleColumnarError> {
    let expected = vec![
        match mode {
            CellPatchAssignmentMode::ContainedShared => 0,
            CellPatchAssignmentMode::DeclaredWeightedInterpolation => u8::MAX,
        };
        validity_bytes(rows)?
    ];
    if bytes != expected {
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
        if read_i32(offsets, index)? != expected_offset {
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
    let offset_bytes = row_count
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if offsets.len() != offset_bytes
        || read_i32(offsets, row_count)? != expected_offset
        || expected_offset != values.len()
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
    }
    Ok(())
}

fn validate_u64_rows(
    values: &[u8],
    rows: impl Iterator<Item = u64>,
) -> Result<(), MultiscaleColumnarError> {
    let mut count = 0_usize;
    for (index, expected) in rows.enumerate() {
        let start = index
            .checked_mul(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let end = start
            .checked_add(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let observed = u64::from_le_bytes(
            values
                .get(start..end)
                .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidBuffers))?,
        );
        if observed != expected {
            return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
        }
        count = index
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    if values.len()
        != count
            .checked_mul(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidBuffers));
    }
    Ok(())
}

fn read_i32(bytes: &[u8], index: usize) -> Result<usize, MultiscaleColumnarError> {
    let start = index
        .checked_mul(size_of::<i32>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let end = start
        .checked_add(size_of::<i32>())
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

fn decoded_bytes(
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
) -> Result<u64, MultiscaleColumnarError> {
    let mut chunks: Box<dyn Iterator<Item = Result<usize, MultiscaleColumnarError>> + '_> =
        match profile {
            CellPatchArrowProfile::Assignment => Box::new(
                link.assignments()
                    .chunks(RECORD_BATCH_ROWS)
                    .map(assignment_decoded_bytes),
            ),
            CellPatchArrowProfile::Edge => Box::new(
                link.edges()
                    .chunks(RECORD_BATCH_ROWS)
                    .map(edge_decoded_bytes),
            ),
        };
    chunks.try_fold(0_u64, |total, bytes| {
        total
            .checked_add(u64::try_from(bytes?).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })
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

#[cfg(test)]
mod tests {
    use arrow_ipc::{
        root_as_message, BodyCompressionBuilder, CompressionType, MessageBuilder, MessageHeader,
        MetadataVersion, RecordBatchBuilder,
    };
    use flatbuffers::{FlatBufferBuilder, WIPOffset};

    use super::{validate_record_batch_features, MultiscaleColumnarError, SpatialArrowFailure};

    #[test]
    fn record_batch_feature_gate_rejects_compression_and_variadic_buffers() {
        for (with_compression, with_variadic) in [(true, false), (false, true)] {
            let mut builder = FlatBufferBuilder::new();
            let variadic = with_variadic.then(|| builder.create_vector(&[1_i64]));
            let compression = if with_compression {
                let mut compression = BodyCompressionBuilder::new(&mut builder);
                compression.add_codec(CompressionType::LZ4_FRAME);
                Some(compression.finish())
            } else {
                None
            };
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
