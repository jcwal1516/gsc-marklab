use std::io::{Read, Seek, SeekFrom, Write};

use arrow::{datatypes::Schema, record_batch::RecordBatch};
use arrow_ipc::{
    writer::{FileWriter, IpcWriteOptions},
    MetadataVersion,
};
use marklab_project::ContentDigest;

use crate::columnar::{
    arrow::{
        profile::{
            ALIGNMENT, ARROW_MAGIC, CONTINUATION_MARKER, EOS_BYTES, HEADER_BYTES,
            MAXIMUM_FOOTER_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS, TRAILER_BYTES,
        },
        writer::DigestingWriter,
    },
    multiscale::{enforce_retained_budget, SpatialArrowFailure},
    EmbeddingColumnarBudgets, MultiscaleColumnarError,
};

use super::profile::arrow_failure;

pub(super) struct EncodedArrow {
    pub(super) content_digest: ContentDigest,
    pub(super) encoded_byte_len: u64,
    pub(super) row_count: u64,
}

pub(super) fn write_record_batches<F>(
    output: &mut dyn Write,
    schema: &Schema,
    row_count: usize,
    budgets: EmbeddingColumnarBudgets,
    mut build_batch: F,
) -> Result<EncodedArrow, MultiscaleColumnarError>
where
    F: FnMut(&Schema, usize, usize) -> Result<RecordBatch, MultiscaleColumnarError>,
{
    let options = IpcWriteOptions::try_new(ALIGNMENT, false, MetadataVersion::V5)
        .map_err(|_| MultiscaleColumnarError::ArrowWriter)?;
    let mut sink = DigestingWriter::new(output, budgets.maximum_file_bytes());
    let write_result = (|| {
        let mut writer = FileWriter::try_new_with_options(&mut sink, schema, options)
            .map_err(|_| MultiscaleColumnarError::ArrowWriter)?;
        let mut start = 0_usize;
        while start < row_count {
            let end = start
                .checked_add(RECORD_BATCH_ROWS)
                .map(|value| value.min(row_count))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            writer
                .write(&build_batch(schema, start, end)?)
                .map_err(|_| MultiscaleColumnarError::ArrowWriter)?;
            start = end;
        }
        writer
            .finish()
            .map_err(|_| MultiscaleColumnarError::ArrowWriter)
    })();
    if let Some(observed) = sink.budget_exceeded() {
        return Err(MultiscaleColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    write_result?;
    let (content_digest, encoded_byte_len) = sink.finish();
    Ok(EncodedArrow {
        content_digest,
        encoded_byte_len,
        row_count: u64::try_from(row_count).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
    })
}

pub(super) fn read_footer<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(usize, usize, Vec<u8>), MultiscaleColumnarError> {
    if file_len < HEADER_BYTES + EOS_BYTES + TRAILER_BYTES {
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
    let trailer_start = file_len
        .checked_sub(TRAILER_BYTES)
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFooterLength))?;
    let mut trailer = [0_u8; TRAILER_BYTES];
    read_exact_at(
        reader,
        file_len,
        trailer_start,
        &mut trailer,
        SpatialArrowFailure::InvalidFooterLength,
    )?;
    if header.get(..ARROW_MAGIC.len()) != Some(ARROW_MAGIC)
        || header[ARROW_MAGIC.len()..].iter().any(|byte| *byte != 0)
        || trailer.get(4..) != Some(ARROW_MAGIC)
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidMagic));
    }
    let signed = i32::from_le_bytes(
        trailer[..4]
            .try_into()
            .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidFooterLength))?,
    );
    if signed <= 0 {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFooterLength));
    }
    let footer_len = usize::try_from(signed)
        .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidFooterLength))?;
    if footer_len > MAXIMUM_FOOTER_BYTES || footer_len > trailer_start {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFooterLength));
    }
    let footer_start = trailer_start
        .checked_sub(footer_len)
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidFooterLength))?;
    if footer_start < HEADER_BYTES + EOS_BYTES {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFooterLength));
    }
    let mut eos = [0_u8; EOS_BYTES];
    read_exact_at(
        reader,
        file_len,
        footer_start - EOS_BYTES,
        &mut eos,
        SpatialArrowFailure::InvalidFooterLength,
    )?;
    if &eos[..4] != CONTINUATION_MARKER || eos[4..] != [0; 4] {
        return Err(arrow_failure(SpatialArrowFailure::InvalidFooterLength));
    }
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

pub(super) fn read_exact_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    buffer: &mut [u8],
    failure: SpatialArrowFailure,
) -> Result<(), MultiscaleColumnarError> {
    if offset
        .checked_add(buffer.len())
        .is_none_or(|end| end > file_len)
    {
        return Err(arrow_failure(failure));
    }
    reader
        .seek(SeekFrom::Start(
            u64::try_from(offset).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        ))
        .and_then(|_| reader.read_exact(buffer))
        .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))
}

pub(super) fn read_vec_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    length: usize,
    failure: SpatialArrowFailure,
) -> Result<Vec<u8>, MultiscaleColumnarError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| MultiscaleColumnarError::AllocationFailed { requested: length })?;
    bytes.resize(length, 0);
    read_exact_at(reader, file_len, offset, &mut bytes, failure)?;
    Ok(bytes)
}

pub(super) fn checked_range(
    start: usize,
    rows: usize,
) -> Result<std::ops::Range<usize>, MultiscaleColumnarError> {
    Ok(start
        ..start
            .checked_add(rows)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?)
}

pub(super) fn validity_bytes(rows: usize) -> Result<usize, MultiscaleColumnarError> {
    rows.checked_add(7)
        .map(|value| value / 8)
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

pub(super) fn align(value: usize) -> Result<usize, MultiscaleColumnarError> {
    value
        .checked_add(ALIGNMENT - 1)
        .map(|value| value / ALIGNMENT * ALIGNMENT)
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

pub(super) fn nonnegative_i32(
    value: i32,
    failure: SpatialArrowFailure,
) -> Result<usize, MultiscaleColumnarError> {
    usize::try_from(value).map_err(|_| arrow_failure(failure))
}

pub(super) fn nonnegative_i64(
    value: i64,
    failure: SpatialArrowFailure,
) -> Result<usize, MultiscaleColumnarError> {
    usize::try_from(value).map_err(|_| arrow_failure(failure))
}
