use crate::columnar::arrow::multiscale::preflight::preflight_summary;
use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow_ipc::convert::fb_to_schema;
use marklab_project::ContentDigest;

use crate::{CellPatchAssignmentMode, CellPatchLink};

use super::super::super::physical::{
    align, checked_range, decode_footer, decode_record_batch_message, nonnegative_i32,
    nonnegative_i64, read_footer, traverse_record_batches, validate_footer_features,
    validate_record_batch_features, validity_bytes,
};
use super::profile::{
    arrow_failure, expected_schema, validate_flatbuffer_schema, CellPatchArrowProfile,
};
use crate::columnar::{
    arrow::profile::{ALIGNMENT, EOS_BYTES, RECORD_BATCH_ROWS},
    multiscale::{
        assignment_decoded_bytes, edge_decoded_bytes, enforce_decoded_budget, enforce_file_budget,
    },
    ArrowIpcFailure, ColumnarError, EmbeddingColumnarBudgets,
};

preflight_summary!(
    CellPatchAssignmentArrowPreflight,
    "Validated structural declaration for one canonical cell-patch Arrow table."
);
preflight_summary!(
    CellPatchEdgeArrowPreflight,
    "Validated structural declaration for one canonical cell-patch Arrow table."
);

#[derive(Clone, Copy)]
pub(crate) struct CommonArrowPreflight {
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
) -> Result<CellPatchAssignmentArrowPreflight, ColumnarError> {
    preflight_bytes(bytes, CellPatchArrowProfile::Assignment, link, budgets)
        .map(CellPatchAssignmentArrowPreflight::from_common)
}

/// Validate borrowed cell-patch edge Arrow bytes through raw bounded preflight only.
pub fn preflight_cell_patch_edge_table_arrow_bytes(
    bytes: &[u8],
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeArrowPreflight, ColumnarError> {
    preflight_bytes(bytes, CellPatchArrowProfile::Edge, link, budgets)
        .map(CellPatchEdgeArrowPreflight::from_common)
}

fn preflight_bytes(
    bytes: &[u8],
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CommonArrowPreflight, ColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
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
) -> Result<CommonArrowPreflight, ColumnarError> {
    let encoded_byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    let file_len = usize::try_from(encoded_byte_len).map_err(|_| ColumnarError::SizeOverflow)?;
    let (footer_start, footer_len, footer_bytes) = read_footer(reader, file_len, budgets)?;
    let footer = decode_footer(&footer_bytes)?;
    validate_footer_features(footer)?;
    let footer_schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    validate_flatbuffer_schema(footer_schema, profile, link)?;
    let schema = expected_schema(profile, link)?;
    if fb_to_schema(footer_schema) != schema {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    let row_count = profile.row_count(link);
    let expected_decoded_bytes = decoded_bytes(profile, link)?;
    enforce_decoded_budget(expected_decoded_bytes, budgets)?;
    let schema_end = super::super::super::physical::validate_schema_message(
        reader,
        file_len,
        footer_start,
        &schema,
        |schema| validate_flatbuffer_schema(schema, profile, link),
        |_bytes| Ok(()),
    )?;
    let blocks = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    if blocks.len() != row_count.div_ceil(RECORD_BATCH_ROWS) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let traversal = traverse_record_batches(
        reader,
        file_len,
        footer_start,
        footer_len,
        schema_end,
        blocks.iter(),
        row_count,
        budgets,
        |message, body, declared_body_len, start, rows| {
            validate_record_message(message, body, declared_body_len, profile, start, rows, link)
        },
    )?;
    if traversal.end_offset != footer_start.saturating_sub(EOS_BYTES)
        || traversal.row_count != row_count
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    if traversal.decoded_bytes != expected_decoded_bytes {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    Ok(CommonArrowPreflight {
        row_count: u64::try_from(row_count).map_err(|_| ColumnarError::SizeOverflow)?,
        record_batch_count: u32::try_from(blocks.len()).map_err(|_| ColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: traversal.retained_preflight_bytes,
        maximum_batch_decoded_bytes: usize::try_from(traversal.maximum_batch_decoded_bytes)
            .map_err(|_| ColumnarError::SizeOverflow)?,
    })
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
) -> Result<u64, ColumnarError> {
    let batch = decode_record_batch_message(message_bytes, body, declared_body_len)?;
    validate_record_batch_features(batch)?;
    if nonnegative_i64(batch.length(), ArrowIpcFailure::InvalidRowCount)? != rows
        || rows == 0
        || rows > RECORD_BATCH_ROWS
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFieldNodes))?;
    validate_nodes(nodes, profile, rows, link.mode())?;
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    if buffers.len() != profile.buffer_count() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
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
            .checked_add(u64::try_from(*length).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)
    })?;
    Ok(decoded)
}

fn validate_nodes(
    nodes: flatbuffers::Vector<'_, arrow_ipc::FieldNode>,
    profile: CellPatchArrowProfile,
    rows: usize,
    mode: CellPatchAssignmentMode,
) -> Result<(), ColumnarError> {
    if nodes.len() != profile.field_count() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFieldNodes));
    }
    for (index, node) in nodes.iter().enumerate() {
        let expected_nulls = match (profile, index, mode) {
            (CellPatchArrowProfile::Edge, 2 | 3, CellPatchAssignmentMode::ContainedShared) => rows,
            _ => 0,
        };
        if nonnegative_i64(node.length(), ArrowIpcFailure::InvalidFieldNodes)? != rows
            || nonnegative_i64(node.null_count(), ArrowIpcFailure::InvalidFieldNodes)?
                != expected_nulls
        {
            return Err(arrow_failure(ArrowIpcFailure::InvalidFieldNodes));
        }
    }
    Ok(())
}

fn expected_buffer_lengths(
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
    start: usize,
    rows: usize,
) -> Result<Vec<usize>, ColumnarError> {
    let validity = validity_bytes(rows)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(size_of::<u64>())
        .ok_or(ColumnarError::SizeOverflow)?;
    match profile {
        CellPatchArrowProfile::Assignment => {
            let selected = &link.assignments()[checked_range(start, rows)?];
            let (cells, statuses) = selected.iter().try_fold(
                (0_usize, 0_usize),
                |(cell_total, status_total), row| {
                    Ok::<_, ColumnarError>((
                        cell_total
                            .checked_add(row.cell_id().as_str().len())
                            .ok_or(ColumnarError::SizeOverflow)?,
                        status_total
                            .checked_add(row.status().wire_name().len())
                            .ok_or(ColumnarError::SizeOverflow)?,
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
                    .ok_or(ColumnarError::SizeOverflow)
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
) -> Result<Vec<std::ops::Range<usize>>, ColumnarError> {
    let mut ranges = Vec::new();
    let range_bytes = buffers
        .len()
        .checked_mul(size_of::<std::ops::Range<usize>>())
        .ok_or(ColumnarError::SizeOverflow)?;
    ranges
        .try_reserve_exact(buffers.len())
        .map_err(|_| ColumnarError::AllocationFailed {
            requested: range_bytes,
        })?;
    let mut next = 0_usize;
    for (buffer, expected_len) in buffers.iter().zip(expected_lengths) {
        let offset = nonnegative_i64(buffer.offset(), ArrowIpcFailure::InvalidBuffers)?;
        let length = nonnegative_i64(buffer.length(), ArrowIpcFailure::InvalidBuffers)?;
        let aligned = align(next)?;
        if offset != aligned || offset % ALIGNMENT != 0 || length != *expected_len {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        if offset > body.len() || body[next..offset].iter().any(|byte| *byte != 0) {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        let end = offset
            .checked_add(length)
            .ok_or(ColumnarError::SizeOverflow)?;
        if end > body.len() {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        ranges.push(offset..end);
        next = end;
    }
    let aligned_end = align(next)?;
    if aligned_end != body.len() || body[next..aligned_end].iter().any(|byte| *byte != 0) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    Ok(ranges)
}

fn validate_assignment_rows(
    body: &[u8],
    ranges: &[std::ops::Range<usize>],
    link: &CellPatchLink,
    start: usize,
    rows: usize,
) -> Result<(), ColumnarError> {
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
) -> Result<(), ColumnarError> {
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

fn validate_all_ones(bytes: &[u8]) -> Result<(), ColumnarError> {
    if bytes.iter().any(|byte| *byte != u8::MAX) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    Ok(())
}

fn validate_weight_bitmap(
    bytes: &[u8],
    rows: usize,
    mode: CellPatchAssignmentMode,
) -> Result<(), ColumnarError> {
    let expected = vec![
        match mode {
            CellPatchAssignmentMode::ContainedShared => 0,
            CellPatchAssignmentMode::DeclaredWeightedInterpolation => u8::MAX,
        };
        validity_bytes(rows)?
    ];
    if bytes != expected {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    Ok(())
}

fn validate_utf8_rows<'a>(
    offsets: &[u8],
    values: &[u8],
    rows: impl Iterator<Item = &'a str>,
) -> Result<(), ColumnarError> {
    let mut expected_offset = 0_usize;
    let mut row_count = 0_usize;
    for (index, expected) in rows.enumerate() {
        if read_i32(offsets, index)? != expected_offset {
            return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
        }
        let end = expected_offset
            .checked_add(expected.len())
            .ok_or(ColumnarError::SizeOverflow)?;
        if values.get(expected_offset..end) != Some(expected.as_bytes()) {
            return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
        }
        expected_offset = end;
        row_count = index.checked_add(1).ok_or(ColumnarError::SizeOverflow)?;
    }
    let offset_bytes = row_count
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    if offsets.len() != offset_bytes
        || read_i32(offsets, row_count)? != expected_offset
        || expected_offset != values.len()
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
    }
    Ok(())
}

fn validate_u64_rows(values: &[u8], rows: impl Iterator<Item = u64>) -> Result<(), ColumnarError> {
    let mut count = 0_usize;
    for (index, expected) in rows.enumerate() {
        let start = index
            .checked_mul(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let end = start
            .checked_add(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let observed = u64::from_le_bytes(
            values
                .get(start..end)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        );
        if observed != expected {
            return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
        }
        count = index.checked_add(1).ok_or(ColumnarError::SizeOverflow)?;
    }
    if values.len()
        != count
            .checked_mul(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    Ok(())
}

fn read_i32(bytes: &[u8], index: usize) -> Result<usize, ColumnarError> {
    let start = index
        .checked_mul(size_of::<i32>())
        .ok_or(ColumnarError::SizeOverflow)?;
    let end = start
        .checked_add(size_of::<i32>())
        .ok_or(ColumnarError::SizeOverflow)?;
    let value = i32::from_le_bytes(
        bytes
            .get(start..end)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?
            .try_into()
            .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
    );
    nonnegative_i32(value, ArrowIpcFailure::InvalidBuffers)
}

fn decoded_bytes(
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
) -> Result<u64, ColumnarError> {
    let mut chunks: Box<dyn Iterator<Item = Result<usize, ColumnarError>> + '_> = match profile {
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
            .checked_add(u64::try_from(bytes?).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)
    })
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
                Err(ColumnarError::Arrow {
                    reason: ArrowIpcFailure::ForbiddenFeature,
                })
            );
        }
    }
}
