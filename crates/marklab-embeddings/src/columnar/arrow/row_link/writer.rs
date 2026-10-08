use std::{
    io::{self, Write},
    sync::Arc,
};

use arrow::{
    array::{ArrayRef, StringBuilder, UInt64Array, UInt64Builder},
    buffer::{BooleanBuffer, NullBuffer},
    datatypes::Schema,
    record_batch::RecordBatch,
};
use arrow_ipc::{
    writer::{FileWriter, IpcWriteOptions},
    MetadataVersion,
};

use crate::CellEmbeddingRowLink;

use super::{
    super::{
        super::super::{
            EmbeddingColumnarBudgets, EmbeddingColumnarError, RowLinkColumnarWriteSummary,
        },
        profile::{
            align, enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
            validity_bytes, ALIGNMENT, CONTINUATION_MARKER, MAXIMUM_MESSAGE_BYTES,
            RECORD_BATCH_ROWS,
        },
        writer::DigestingWriter,
    },
    profile::row_link_schema,
};

/// Write the exact deterministic Arrow IPC profile for a canonical row link.
pub fn write_cell_embedding_row_link_arrow(
    output: &mut dyn Write,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<RowLinkColumnarWriteSummary, EmbeddingColumnarError> {
    let row_count = row_link.entries().len();
    let decoded_bytes = estimate_decoded_bytes(row_link)?;
    enforce_decoded_budget(decoded_bytes, budgets)?;
    let maximum_batch_bytes = estimate_batch_bytes(row_link)?;
    enforce_row_group_budget(maximum_batch_bytes, budgets)?;

    let record_batch_count = row_count.div_ceil(RECORD_BATCH_ROWS);
    let record_block_bytes = record_batch_count
        .checked_mul(size_of::<arrow_ipc::Block>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let record_block_capacity_bytes = record_block_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(size_of::<Vec<arrow_ipc::Block>>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let batch_peak_bytes = maximum_batch_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(record_block_capacity_bytes))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let footer_builder_bytes = record_block_bytes
        .checked_add(MAXIMUM_MESSAGE_BYTES)
        .and_then(|value| value.checked_mul(2))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let footer_peak_bytes = record_block_capacity_bytes
        .checked_add(footer_builder_bytes)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    enforce_retained_budget(batch_peak_bytes.max(footer_peak_bytes), budgets)?;

    let schema = row_link_schema(row_link)?;
    let options = IpcWriteOptions::try_new(ALIGNMENT, false, MetadataVersion::V5)
        .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
    let digesting_sink = DigestingWriter::new(output, budgets.maximum_file_bytes());
    let mut sink = CanonicalRowLinkWriter::new(digesting_sink);
    let write_result = (|| {
        let mut writer = FileWriter::try_new_with_options(&mut sink, &schema, options)
            .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
        let mut start = 0_usize;
        while start < row_count {
            let end = start
                .checked_add(RECORD_BATCH_ROWS)
                .map(|value| value.min(row_count))
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let batch = build_batch(row_link, &schema, start, end)?;
            if let Some((body_offset, replacement)) =
                nullable_validity_tail_patch(row_link, start, end)?
            {
                writer
                    .get_mut()
                    .arm_batch_patch(body_offset, replacement)
                    .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
            }
            writer
                .write(&batch)
                .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
            writer
                .get_mut()
                .finish_batch()
                .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
            start = end;
        }
        writer
            .finish()
            .map_err(|_| EmbeddingColumnarError::ArrowWriter)
    })();
    if let Some(observed) = sink.budget_exceeded() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    write_result?;
    let (content_digest, encoded_byte_len) = sink.finish();
    Ok(RowLinkColumnarWriteSummary::new(
        content_digest,
        encoded_byte_len,
        row_link.row_count(),
    ))
}

fn nullable_validity_tail_patch(
    row_link: &CellEmbeddingRowLink,
    start: usize,
    end: usize,
) -> Result<Option<(u64, u8)>, EmbeddingColumnarError> {
    let entries = row_link
        .entries()
        .get(start..end)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let rows = entries.len();
    let remainder = rows % 8;
    if remainder == 0
        || entries
            .iter()
            .any(|entry| entry.source_embedding_row().is_none())
    {
        return Ok(None);
    }
    let validity = validity_bytes(rows)?;
    let identifier_bytes = entries.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(size_of::<u64>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let lengths = [validity, offsets, identifier_bytes, validity, values];
    let nullable_validity_offset = lengths.into_iter().try_fold(0_usize, |next, length| {
        align(next)?
            .checked_add(length)
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    let tail_offset = align(nullable_validity_offset)?
        .checked_add(
            validity
                .checked_sub(1)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?,
        )
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let replacement = u8::MAX >> (8 - remainder);
    Ok(Some((
        u64::try_from(tail_offset).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        replacement,
    )))
}

struct PendingTailPatch {
    batch_start: u64,
    body_offset: u64,
    replacement: u8,
    prefix: [u8; 8],
    prefix_bytes: usize,
    patch_offset: Option<u64>,
    applied: bool,
}

struct CanonicalRowLinkWriter<'a> {
    inner: DigestingWriter<'a>,
    byte_len: u64,
    pending: Option<PendingTailPatch>,
}

impl<'a> CanonicalRowLinkWriter<'a> {
    fn new(inner: DigestingWriter<'a>) -> Self {
        Self {
            inner,
            byte_len: 0,
            pending: None,
        }
    }

    fn arm_batch_patch(&mut self, body_offset: u64, replacement: u8) -> io::Result<()> {
        if self.pending.is_some() {
            return Err(io::Error::other("canonical row-link patch already armed"));
        }
        self.pending = Some(PendingTailPatch {
            batch_start: self.byte_len,
            body_offset,
            replacement,
            prefix: [0; 8],
            prefix_bytes: 0,
            patch_offset: None,
            applied: false,
        });
        Ok(())
    }

    fn finish_batch(&mut self) -> io::Result<()> {
        if self.pending.as_ref().is_some_and(|patch| !patch.applied) {
            return Err(io::Error::other("canonical row-link patch was not applied"));
        }
        self.pending = None;
        Ok(())
    }

    fn budget_exceeded(&self) -> Option<u64> {
        self.inner.budget_exceeded()
    }

    fn finish(self) -> (marklab_project::ContentDigest, u64) {
        self.inner.finish()
    }

    fn write_inner(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.byte_len = self
            .byte_len
            .checked_add(
                u64::try_from(written).map_err(|_| io::Error::other("columnar size overflow"))?,
            )
            .ok_or_else(|| io::Error::other("columnar size overflow"))?;
        Ok(written)
    }
}

impl Write for CanonicalRowLinkWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let needs_prefix = self
            .pending
            .as_ref()
            .is_some_and(|patch| patch.patch_offset.is_none());
        if needs_prefix {
            let prefix_remaining = self
                .pending
                .as_ref()
                .map(|patch| {
                    8_usize
                        .checked_sub(patch.prefix_bytes)
                        .ok_or_else(|| io::Error::other("invalid canonical row-link IPC prefix"))
                })
                .transpose()?
                .unwrap_or_default();
            let length = buffer.len().min(prefix_remaining);
            let written = self.write_inner(&buffer[..length])?;
            if written == 0 {
                return Ok(0);
            }
            let patch = self
                .pending
                .as_mut()
                .ok_or_else(|| io::Error::other("canonical row-link patch disappeared"))?;
            let prefix_end = patch
                .prefix_bytes
                .checked_add(written)
                .ok_or_else(|| io::Error::other("columnar size overflow"))?;
            patch.prefix[patch.prefix_bytes..prefix_end].copy_from_slice(&buffer[..written]);
            patch.prefix_bytes = prefix_end;
            if patch.prefix_bytes == patch.prefix.len() {
                if &patch.prefix[..4] != CONTINUATION_MARKER {
                    return Err(io::Error::other("invalid canonical row-link IPC prefix"));
                }
                let declared = i32::from_le_bytes(
                    patch.prefix[4..]
                        .try_into()
                        .map_err(|_| io::Error::other("invalid canonical row-link IPC prefix"))?,
                );
                let declared = u64::try_from(declared)
                    .map_err(|_| io::Error::other("invalid canonical row-link IPC prefix"))?;
                patch.patch_offset = Some(
                    patch
                        .batch_start
                        .checked_add(8)
                        .and_then(|value| value.checked_add(declared))
                        .and_then(|value| value.checked_add(patch.body_offset))
                        .ok_or_else(|| io::Error::other("columnar size overflow"))?,
                );
            }
            return Ok(written);
        }

        let patch_offset = self.pending.as_ref().and_then(|patch| patch.patch_offset);
        if let Some(patch_offset) = patch_offset {
            if self.byte_len < patch_offset {
                let distance = usize::try_from(patch_offset - self.byte_len)
                    .map_err(|_| io::Error::other("columnar size overflow"))?;
                return self.write_inner(&buffer[..buffer.len().min(distance)]);
            }
            if self.byte_len == patch_offset {
                if buffer[0] != u8::MAX {
                    return Err(io::Error::other(
                        "unexpected canonical row-link validity tail",
                    ));
                }
                let replacement = self
                    .pending
                    .as_ref()
                    .map(|patch| patch.replacement)
                    .ok_or_else(|| io::Error::other("canonical row-link patch disappeared"))?;
                let written = self.write_inner(&[replacement])?;
                if written == 1 {
                    self.pending
                        .as_mut()
                        .ok_or_else(|| io::Error::other("canonical row-link patch disappeared"))?
                        .applied = true;
                }
                return Ok(written);
            }
            if self.pending.as_ref().is_some_and(|patch| !patch.applied) {
                return Err(io::Error::other("canonical row-link patch was skipped"));
            }
        }
        self.write_inner(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn build_batch(
    row_link: &CellEmbeddingRowLink,
    schema: &Schema,
    start: usize,
    end: usize,
) -> Result<RecordBatch, EmbeddingColumnarError> {
    let rows = end
        .checked_sub(start)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let identifier_bytes =
        row_link.entries()[start..end]
            .iter()
            .try_fold(0_usize, |total, entry| {
                total
                    .checked_add(entry.cell_id().as_str().len())
                    .ok_or(EmbeddingColumnarError::SizeOverflow)
            })?;
    let mut cells = StringBuilder::with_capacity(rows, identifier_bytes);
    let mut source_cells = UInt64Builder::with_capacity(rows);
    let mut source_embedding_values = Vec::with_capacity(rows);
    let entries = &row_link.entries()[start..end];
    for entry in entries {
        cells.append_value(entry.cell_id().as_str());
        source_cells.append_value(entry.source_cell_row());
        source_embedding_values.push(entry.source_embedding_row().unwrap_or_default());
    }
    // The frozen profile requires a physical validity bitmap even for all-valid batches.
    // Constructing the nullable column from a builder would elide that bitmap when its
    // null count is zero and make equivalent logical inputs encode differently.
    let source_embedding_validity = NullBuffer::new(BooleanBuffer::collect_bool(rows, |row| {
        entries[row].source_embedding_row().is_some()
    }));
    let source_embeddings = UInt64Array::new(
        source_embedding_values.into(),
        Some(source_embedding_validity),
    );
    let columns: Vec<ArrayRef> = vec![
        Arc::new(cells.finish()),
        Arc::new(source_cells.finish()),
        Arc::new(source_embeddings),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| EmbeddingColumnarError::ArrowWriter)
}

fn estimate_decoded_bytes(row_link: &CellEmbeddingRowLink) -> Result<u64, EmbeddingColumnarError> {
    let mut required = 0_u64;
    for batch in row_link.entries().chunks(RECORD_BATCH_ROWS) {
        let rows = batch.len();
        let identifier_bytes = batch.iter().try_fold(0_u64, |total, entry| {
            total
                .checked_add(
                    u64::try_from(entry.cell_id().as_str().len())
                        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                )
                .ok_or(EmbeddingColumnarError::SizeOverflow)
        })?;
        let offsets = rows
            .checked_add(1)
            .and_then(|value| value.checked_mul(size_of::<i32>()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let values = rows
            .checked_mul(2)
            .and_then(|value| value.checked_mul(size_of::<u64>()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let validity = validity_bytes(rows)?
            .checked_mul(3)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let batch_bytes = identifier_bytes
            .checked_add(u64::try_from(offsets).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .and_then(|value| value.checked_add(u64::try_from(values).ok()?))
            .and_then(|value| value.checked_add(u64::try_from(validity).ok()?))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        required = required
            .checked_add(batch_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok(required)
}

fn estimate_batch_bytes(row_link: &CellEmbeddingRowLink) -> Result<usize, EmbeddingColumnarError> {
    let rows = row_link.entries().len().min(RECORD_BATCH_ROWS);
    let identifier_bytes = row_link
        .entries()
        .chunks(RECORD_BATCH_ROWS)
        .map(|batch| {
            batch.iter().try_fold(0_usize, |total, entry| {
                total
                    .checked_add(entry.cell_id().as_str().len())
                    .ok_or(EmbeddingColumnarError::SizeOverflow)
            })
        })
        .try_fold(0_usize, |maximum, bytes| {
            bytes.map(|bytes| maximum.max(bytes))
        })?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(2)
        .and_then(|value| value.checked_mul(size_of::<u64>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let validity = validity_bytes(rows)?
        .checked_mul(3)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    identifier_bytes
        .checked_add(offsets)
        .and_then(|value| value.checked_add(values))
        .and_then(|value| value.checked_add(validity))
        .and_then(|value| value.checked_add(7 * (ALIGNMENT - 1)))
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}
