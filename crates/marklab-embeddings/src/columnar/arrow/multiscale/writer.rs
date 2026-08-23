use std::{io::Write, sync::Arc};

use arrow::{
    array::{ArrayRef, Int64Builder, StringBuilder},
    datatypes::Schema,
    record_batch::RecordBatch,
};
use arrow_ipc::{
    writer::{FileWriter, IpcWriteOptions},
    MetadataVersion,
};

use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

use super::{
    super::{
        profile::{ALIGNMENT, MAXIMUM_FOOTER_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS},
        writer::DigestingWriter,
    },
    profile::{footprint_schema, overlap_schema},
};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
        validate_footprint_domain, validate_overlap_domain,
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialColumnarWriteSummary,
};

/// Write the exact deterministic Arrow IPC profile for a patch-footprint set.
pub fn write_patch_footprint_set_arrow(
    output: &mut dyn Write,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_footprint_domain(expected, context, footprints)?;
    let row_count = footprints.row_count();
    enforce_footer_bound(row_count)?;
    let estimates = footprint_estimates(footprints)?;
    enforce_writer_estimates(row_count, estimates, budgets)?;
    let schema = footprint_schema(expected, context, footprints)?;
    write_arrow(output, &schema, row_count, budgets, |schema, start, end| {
        build_footprint_batch(schema, footprints, start, end)
    })
}

/// Write the exact deterministic Arrow IPC profile for a patch-overlap graph.
pub fn write_patch_overlap_graph_arrow(
    output: &mut dyn Write,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_overlap_domain(expected, context, footprints, overlap)?;
    let row_count = overlap.edge_count();
    enforce_footer_bound(row_count)?;
    let estimates = overlap_estimates(overlap)?;
    enforce_writer_estimates(row_count, estimates, budgets)?;
    let schema = overlap_schema(expected, context, footprints, overlap)?;
    write_arrow(output, &schema, row_count, budgets, |schema, start, end| {
        build_overlap_batch(schema, overlap, start, end)
    })
}

fn write_arrow<F>(
    output: &mut dyn Write,
    schema: &Schema,
    row_count: usize,
    budgets: EmbeddingColumnarBudgets,
    mut build_batch: F,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError>
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
    let (digest, encoded_len) = sink.finish();
    Ok(SpatialColumnarWriteSummary::new(
        digest,
        encoded_len,
        u64::try_from(row_count).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
    ))
}

fn build_footprint_batch(
    schema: &Schema,
    footprints: &PatchFootprintSet,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = end
        .checked_sub(start)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let selected = footprints
        .footprints()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let text_bytes = selected.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.patch_id().as_str().len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    let mut patch_ids = StringBuilder::with_capacity(rows, text_bytes);
    let mut origin_x = Int64Builder::with_capacity(rows);
    let mut origin_y = Int64Builder::with_capacity(rows);
    for row in selected {
        patch_ids.append_value(row.patch_id().as_str());
        origin_x.append_value(row.origin_px()[0]);
        origin_y.append_value(row.origin_px()[1]);
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(patch_ids.finish()),
        Arc::new(origin_x.finish()),
        Arc::new(origin_y.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ArrowWriter)
}

fn build_overlap_batch(
    schema: &Schema,
    overlap: &PatchOverlapGraph,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = end
        .checked_sub(start)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let selected = overlap
        .edges()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (left_bytes, right_bytes) =
        selected
            .iter()
            .try_fold((0_usize, 0_usize), |(left, right), edge| {
                Ok::<_, MultiscaleColumnarError>((
                    left.checked_add(edge.left_patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    right
                        .checked_add(edge.right_patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                ))
            })?;
    let mut left = StringBuilder::with_capacity(rows, left_bytes);
    let mut right = StringBuilder::with_capacity(rows, right_bytes);
    for edge in selected {
        left.append_value(edge.left_patch_id().as_str());
        right.append_value(edge.right_patch_id().as_str());
    }
    let columns: Vec<ArrayRef> = vec![Arc::new(left.finish()), Arc::new(right.finish())];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ArrowWriter)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WriterEstimates {
    decoded_bytes: u64,
    maximum_batch_bytes: usize,
}

fn footprint_estimates(
    footprints: &PatchFootprintSet,
) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_chunks(
        footprints
            .footprints()
            .chunks(RECORD_BATCH_ROWS)
            .map(|chunk| {
                let text = chunk.iter().try_fold(0_usize, |total, row| {
                    total
                        .checked_add(row.patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)
                })?;
                chunk_bytes(chunk.len(), 1, text, 2)
            }),
    )
}

fn overlap_estimates(
    overlap: &PatchOverlapGraph,
) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_chunks(overlap.edges().chunks(RECORD_BATCH_ROWS).map(|chunk| {
        let text = chunk.iter().try_fold(0_usize, |total, edge| {
            total
                .checked_add(edge.left_patch_id().as_str().len())
                .and_then(|value| value.checked_add(edge.right_patch_id().as_str().len()))
                .ok_or(MultiscaleColumnarError::SizeOverflow)
        })?;
        chunk_bytes(chunk.len(), 2, text, 0)
    }))
}

fn estimate_chunks(
    chunks: impl Iterator<Item = Result<usize, MultiscaleColumnarError>>,
) -> Result<WriterEstimates, MultiscaleColumnarError> {
    let mut decoded = 0_u64;
    let mut maximum = 0_usize;
    for bytes in chunks {
        let bytes = bytes?;
        decoded = decoded
            .checked_add(u64::try_from(bytes).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum = maximum.max(
            bytes
                .checked_add(7 * (ALIGNMENT - 1))
                .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
    }
    Ok(WriterEstimates {
        decoded_bytes: decoded,
        maximum_batch_bytes: maximum,
    })
}

fn chunk_bytes(
    rows: usize,
    utf8_columns: usize,
    text_bytes: usize,
    i64_columns: usize,
) -> Result<usize, MultiscaleColumnarError> {
    let validity = rows
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .and_then(|value| value.checked_mul(utf8_columns))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(size_of::<i64>())
        .and_then(|value| value.checked_mul(i64_columns))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    validity
        .checked_mul(utf8_columns + i64_columns)
        .and_then(|value| value.checked_add(offsets))
        .and_then(|value| value.checked_add(text_bytes))
        .and_then(|value| value.checked_add(values))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn enforce_writer_estimates(
    row_count: usize,
    estimates: WriterEstimates,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    enforce_decoded_budget(estimates.decoded_bytes, budgets)?;
    enforce_row_group_budget(estimates.maximum_batch_bytes, budgets)?;
    let record_blocks = row_count
        .div_ceil(RECORD_BATCH_ROWS)
        .checked_mul(size_of::<arrow_ipc::Block>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let block_capacity = record_blocks
        .checked_mul(2)
        .and_then(|value| value.checked_add(size_of::<Vec<arrow_ipc::Block>>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let batch_peak = estimates
        .maximum_batch_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(block_capacity))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let footer_peak = record_blocks
        .checked_add(MAXIMUM_MESSAGE_BYTES)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(block_capacity))
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(batch_peak.max(footer_peak), budgets)
}

fn estimated_footer_bytes(row_count: usize) -> Result<usize, MultiscaleColumnarError> {
    const FOOTER_BASE_BYTES: usize = 64 * 1024;
    FOOTER_BASE_BYTES
        .checked_add(
            row_count
                .div_ceil(RECORD_BATCH_ROWS)
                .checked_mul(size_of::<arrow_ipc::Block>())
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        )
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn enforce_footer_bound(row_count: usize) -> Result<(), MultiscaleColumnarError> {
    if estimated_footer_bytes(row_count)? > MAXIMUM_FOOTER_BYTES {
        return Err(MultiscaleColumnarError::ArrowWriter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_bound_accepts_the_last_group_and_rejects_the_next() {
        let group_bytes = size_of::<arrow_ipc::Block>();
        let maximum_groups = (MAXIMUM_FOOTER_BYTES - 64 * 1024) / group_bytes;
        let maximum_rows = maximum_groups * RECORD_BATCH_ROWS;
        assert!(
            estimated_footer_bytes(maximum_rows).expect("accepted footer") <= MAXIMUM_FOOTER_BYTES
        );
        assert!(
            estimated_footer_bytes(maximum_rows + 1).expect("rejected footer")
                > MAXIMUM_FOOTER_BYTES
        );
    }

    #[test]
    fn scalar_batch_oracles_drive_exact_and_one_short_budget_edges() {
        let footprint_decoded = 3 + 4 * 4 + 3 * 14 + 3 * 16;
        assert_eq!(chunk_bytes(3, 1, 3 * 14, 2), Ok(footprint_decoded));
        let footprint_batch = footprint_decoded + 7 * (ALIGNMENT - 1) + MAXIMUM_MESSAGE_BYTES;
        let footprint = WriterEstimates {
            decoded_bytes: footprint_decoded as u64,
            maximum_batch_bytes: footprint_batch,
        };
        assert_exact_and_one_short(3, footprint);

        let overlap_decoded = 2 + 2 * 3 * 4 + 4 * 14;
        assert_eq!(chunk_bytes(2, 2, 4 * 14, 0), Ok(overlap_decoded));
        let overlap_batch = overlap_decoded + 7 * (ALIGNMENT - 1) + MAXIMUM_MESSAGE_BYTES;
        let overlap = WriterEstimates {
            decoded_bytes: overlap_decoded as u64,
            maximum_batch_bytes: overlap_batch,
        };
        assert_exact_and_one_short(2, overlap);
    }

    fn assert_exact_and_one_short(row_count: usize, estimates: WriterEstimates) {
        let record_blocks = row_count.div_ceil(RECORD_BATCH_ROWS) * size_of::<arrow_ipc::Block>();
        let block_capacity = record_blocks * 2 + size_of::<Vec<arrow_ipc::Block>>();
        let batch_peak = estimates.maximum_batch_bytes * 2 + block_capacity;
        let footer_peak =
            (record_blocks + MAXIMUM_MESSAGE_BYTES) * 2 + block_capacity + MAXIMUM_MESSAGE_BYTES;
        let retained = batch_peak.max(footer_peak);
        let exact = EmbeddingColumnarBudgets::new(
            u64::MAX,
            retained,
            estimates.maximum_batch_bytes,
            estimates.decoded_bytes,
        );
        assert_eq!(
            enforce_writer_estimates(row_count, estimates, exact),
            Ok(())
        );
        let decoded_short = EmbeddingColumnarBudgets::new(
            u64::MAX,
            retained,
            estimates.maximum_batch_bytes,
            estimates.decoded_bytes - 1,
        );
        assert!(matches!(
            enforce_writer_estimates(row_count, estimates, decoded_short),
            Err(MultiscaleColumnarError::DecodedByteBudgetExceeded { .. })
        ));
        let group_short = EmbeddingColumnarBudgets::new(
            u64::MAX,
            retained,
            estimates.maximum_batch_bytes - 1,
            estimates.decoded_bytes,
        );
        assert!(matches!(
            enforce_writer_estimates(row_count, estimates, group_short),
            Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded { .. })
        ));
        let retained_short = EmbeddingColumnarBudgets::new(
            u64::MAX,
            retained - 1,
            estimates.maximum_batch_bytes,
            estimates.decoded_bytes,
        );
        assert!(matches!(
            enforce_writer_estimates(row_count, estimates, retained_short),
            Err(MultiscaleColumnarError::RetainedByteBudgetExceeded { .. })
        ));
    }
}
