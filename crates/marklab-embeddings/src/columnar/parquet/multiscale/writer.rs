use std::{io::Write, sync::Arc};

use arrow::{
    array::{ArrayRef, Int64Builder, StringBuilder},
    record_batch::RecordBatch,
};
use parquet::arrow::arrow_writer::{ArrowWriter, ArrowWriterOptions};

use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

use super::profile::{footprint_schema, overlap_schema, writer_properties, SpatialParquetProfile};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
        validate_footprint_domain, validate_overlap_domain,
    },
    parquet::{
        compact::canonical_compact_len,
        profile::{MAXIMUM_FOOTER_BYTES, PUBLIC_BATCH_ROWS, ROW_GROUP_ROWS},
        writer::{
            estimate_metadata_retained_bytes, ParquetDigestingWriter,
            MAXIMUM_ENCODED_FOOTER_BASE_BYTES, MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP,
            WRITER_FIXED_RETAINED_BYTES,
        },
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialColumnarWriteSummary,
};

/// Write the exact deterministic Parquet profile for a patch-footprint set.
pub fn write_patch_footprint_set_parquet(
    output: &mut (dyn Write + Send),
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_footprint_domain(expected, context, footprints)?;
    let profile = SpatialParquetProfile::Footprint;
    enforce_footer_bound(footprints.row_count())?;
    let estimates = footprint_estimates(footprints)?;
    enforce_writer_estimates(footprints.row_count(), estimates, budgets)?;
    let schema = footprint_schema();
    write_parquet(
        output,
        profile,
        Arc::new(schema),
        expected,
        footprints,
        None,
        footprints.row_count(),
        budgets,
        |schema, start, end| build_footprint_batch(schema, footprints, start, end),
    )
}

/// Write the exact deterministic Parquet profile for a patch-overlap graph.
pub fn write_patch_overlap_graph_parquet(
    output: &mut (dyn Write + Send),
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_overlap_domain(expected, context, footprints, overlap)?;
    let profile = SpatialParquetProfile::Overlap;
    enforce_footer_bound(overlap.edge_count())?;
    let estimates = overlap_estimates(overlap)?;
    enforce_writer_estimates(overlap.edge_count(), estimates, budgets)?;
    let schema = overlap_schema();
    write_parquet(
        output,
        profile,
        Arc::new(schema),
        expected,
        footprints,
        Some(overlap),
        overlap.edge_count(),
        budgets,
        |schema, start, end| build_overlap_batch(schema, overlap, start, end),
    )
}

#[allow(clippy::too_many_arguments)]
fn write_parquet<F>(
    output: &mut (dyn Write + Send),
    profile: SpatialParquetProfile,
    schema: Arc<arrow::datatypes::Schema>,
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    row_count: usize,
    budgets: EmbeddingColumnarBudgets,
    mut build_batch: F,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError>
where
    F: FnMut(
        &arrow::datatypes::Schema,
        usize,
        usize,
    ) -> Result<RecordBatch, MultiscaleColumnarError>,
{
    let options = ArrowWriterOptions::new()
        .with_properties(writer_properties(profile, expected, footprints, overlap))
        .with_skip_arrow_metadata(true)
        .with_schema_root(profile.root().to_owned());
    let mut sink = ParquetDigestingWriter::new(output, budgets.maximum_file_bytes());
    let write_result = (|| {
        let mut writer = ArrowWriter::try_new_with_options(&mut sink, Arc::clone(&schema), options)
            .map_err(|_| MultiscaleColumnarError::ParquetWriter)?;
        let mut start = 0_usize;
        while start < row_count {
            let end = start
                .checked_add(PUBLIC_BATCH_ROWS)
                .map(|value| value.min(row_count))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let batch = build_batch(&schema, start, end)?;
            writer
                .write(&batch)
                .map_err(|_| MultiscaleColumnarError::ParquetWriter)?;
            start = end;
        }
        let metadata = writer
            .close()
            .map_err(|_| MultiscaleColumnarError::ParquetWriter)?;
        canonical_compact_len(&metadata, MAXIMUM_FOOTER_BYTES)
            .map_err(|_| MultiscaleColumnarError::ParquetWriter)?;
        Ok(())
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
    schema: &arrow::datatypes::Schema,
    footprints: &PatchFootprintSet,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = footprints
        .footprints()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let text_bytes = rows.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.patch_id().as_str().len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    let mut ids = StringBuilder::with_capacity(rows.len(), text_bytes);
    let mut x = Int64Builder::with_capacity(rows.len());
    let mut y = Int64Builder::with_capacity(rows.len());
    for row in rows {
        ids.append_value(row.patch_id().as_str());
        x.append_value(row.origin_px()[0]);
        y.append_value(row.origin_px()[1]);
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(ids.finish()),
        Arc::new(x.finish()),
        Arc::new(y.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ParquetWriter)
}

fn build_overlap_batch(
    schema: &arrow::datatypes::Schema,
    overlap: &PatchOverlapGraph,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = overlap
        .edges()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (left_bytes, right_bytes) =
        rows.iter()
            .try_fold((0_usize, 0_usize), |(left, right), row| {
                Ok::<_, MultiscaleColumnarError>((
                    left.checked_add(row.left_patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    right
                        .checked_add(row.right_patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                ))
            })?;
    let mut left = StringBuilder::with_capacity(rows.len(), left_bytes);
    let mut right = StringBuilder::with_capacity(rows.len(), right_bytes);
    for row in rows {
        left.append_value(row.left_patch_id().as_str());
        right.append_value(row.right_patch_id().as_str());
    }
    let columns: Vec<ArrayRef> = vec![Arc::new(left.finish()), Arc::new(right.finish())];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ParquetWriter)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WriterEstimates {
    decoded_bytes: u64,
    row_group_bytes: usize,
    retained_bytes: usize,
}

#[derive(Clone, Copy)]
struct ChunkEstimate {
    decoded_bytes: usize,
    input_batch_bytes: usize,
    uncompressed_value_bytes: usize,
}

fn footprint_estimates(
    footprints: &PatchFootprintSet,
) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_writer(
        footprints.row_count(),
        3,
        footprints.footprints().chunks(ROW_GROUP_ROWS).map(|rows| {
            let text = rows.iter().try_fold(0_usize, |total, row| {
                total
                    .checked_add(row.patch_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)
            })?;
            let decoded_bytes = decoded_chunk(rows.len(), 1, text, 2)?;
            let uncompressed_value_bytes = rows
                .len()
                .checked_mul(20)
                .and_then(|value| value.checked_add(text))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            Ok(ChunkEstimate {
                decoded_bytes,
                input_batch_bytes: decoded_bytes,
                uncompressed_value_bytes,
            })
        }),
    )
}

fn overlap_estimates(
    overlap: &PatchOverlapGraph,
) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_writer(
        overlap.edge_count(),
        2,
        overlap.edges().chunks(ROW_GROUP_ROWS).map(|rows| {
            let text = rows.iter().try_fold(0_usize, |total, row| {
                total
                    .checked_add(row.left_patch_id().as_str().len())
                    .and_then(|value| value.checked_add(row.right_patch_id().as_str().len()))
                    .ok_or(MultiscaleColumnarError::SizeOverflow)
            })?;
            let decoded_bytes = decoded_chunk(rows.len(), 2, text, 0)?;
            let uncompressed_value_bytes = rows
                .len()
                .checked_mul(8)
                .and_then(|value| value.checked_add(text))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            Ok(ChunkEstimate {
                decoded_bytes,
                input_batch_bytes: decoded_bytes,
                uncompressed_value_bytes,
            })
        }),
    )
}

fn estimate_writer(
    row_count: usize,
    columns: usize,
    chunks: impl Iterator<Item = Result<ChunkEstimate, MultiscaleColumnarError>>,
) -> Result<WriterEstimates, MultiscaleColumnarError> {
    let mut decoded_bytes = 0_u64;
    let mut maximum_input_batch = 0_usize;
    let mut maximum_uncompressed_values = 0_usize;
    for chunk in chunks {
        let chunk = chunk?;
        decoded_bytes = decoded_bytes
            .checked_add(
                u64::try_from(chunk.decoded_bytes)
                    .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum_input_batch = maximum_input_batch.max(chunk.input_batch_bytes);
        maximum_uncompressed_values =
            maximum_uncompressed_values.max(chunk.uncompressed_value_bytes);
    }
    let group_rows = row_count.min(ROW_GROUP_ROWS);
    let page_bound = group_rows
        .checked_mul(columns)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let page_overhead = page_bound
        .checked_mul(1024 + 4 * size_of::<bytes::Bytes>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    // All leaves are required, so Data Page V2 level byte lengths are exactly zero. The encoder
    // workspace is still charged independently because the locked writer constructs level state.
    let encoder_workspace = group_rows
        .checked_mul(16)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let retained_row_group_chunks = maximum_uncompressed_values
        .checked_mul(2)
        .and_then(|value| value.checked_add(page_overhead))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let page_assembly_transient = maximum_uncompressed_values
        .checked_mul(4)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let row_group_bytes = maximum_input_batch
        .checked_add(encoder_workspace)
        .and_then(|value| value.checked_add(retained_row_group_chunks))
        .and_then(|value| value.checked_add(page_assembly_transient))
        .and_then(|value| value.checked_add(WRITER_FIXED_RETAINED_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let row_groups = row_count.div_ceil(ROW_GROUP_ROWS);
    let metadata = estimate_metadata_retained_bytes(row_groups)
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let row_group_peak = row_group_bytes
        .checked_add(metadata)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let footer_peak = metadata
        .checked_add(WRITER_FIXED_RETAINED_BYTES)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let retained_bytes = row_group_peak.max(footer_peak);
    Ok(WriterEstimates {
        decoded_bytes,
        row_group_bytes,
        retained_bytes,
    })
}

fn decoded_chunk(
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
    _row_count: usize,
    estimates: WriterEstimates,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    enforce_decoded_budget(estimates.decoded_bytes, budgets)?;
    enforce_row_group_budget(estimates.row_group_bytes, budgets)?;
    enforce_retained_budget(estimates.retained_bytes, budgets)
}

fn enforce_footer_bound(row_count: usize) -> Result<(), MultiscaleColumnarError> {
    let footer_bound = MAXIMUM_ENCODED_FOOTER_BASE_BYTES
        .checked_add(
            row_count
                .div_ceil(ROW_GROUP_ROWS)
                .checked_mul(MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        )
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if footer_bound > MAXIMUM_FOOTER_BYTES {
        return Err(MultiscaleColumnarError::ParquetWriter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_profile_memory_oracles_drive_exact_and_one_short_edges() {
        let footprint_decoded = 3 + 4 * 4 + 3 * 14 + 3 * 16;
        let footprint_uncompressed = 3 * 20 + 3 * 14;
        let footprint = estimate_writer(
            3,
            3,
            std::iter::once(Ok(ChunkEstimate {
                decoded_bytes: footprint_decoded,
                input_batch_bytes: footprint_decoded,
                uncompressed_value_bytes: footprint_uncompressed,
            })),
        )
        .expect("footprint estimate");
        assert_eq!(
            footprint,
            oracle(3, 3, footprint_decoded, footprint_uncompressed)
        );
        assert_exact_and_one_short(3, footprint);

        let overlap_decoded = 2 + 2 * 3 * 4 + 4 * 14;
        let overlap_uncompressed = 2 * 8 + 4 * 14;
        let overlap = estimate_writer(
            2,
            2,
            std::iter::once(Ok(ChunkEstimate {
                decoded_bytes: overlap_decoded,
                input_batch_bytes: overlap_decoded,
                uncompressed_value_bytes: overlap_uncompressed,
            })),
        )
        .expect("overlap estimate");
        assert_eq!(overlap, oracle(2, 2, overlap_decoded, overlap_uncompressed));
        assert_exact_and_one_short(2, overlap);
    }

    fn oracle(
        rows: usize,
        columns: usize,
        input_batch: usize,
        uncompressed: usize,
    ) -> WriterEstimates {
        let page_overhead = rows * columns * (1024 + 4 * size_of::<bytes::Bytes>());
        let encoder_workspace = rows * 16;
        let retained_chunks = uncompressed * 2 + page_overhead;
        let page_transient = uncompressed * 4;
        let row_group_bytes = input_batch
            + encoder_workspace
            + retained_chunks
            + page_transient
            + WRITER_FIXED_RETAINED_BYTES;
        let metadata = estimate_metadata_retained_bytes(1).expect("metadata estimate");
        WriterEstimates {
            decoded_bytes: input_batch as u64,
            row_group_bytes,
            retained_bytes: (row_group_bytes + metadata)
                .max(metadata + WRITER_FIXED_RETAINED_BYTES),
        }
    }

    fn assert_exact_and_one_short(row_count: usize, estimates: WriterEstimates) {
        let exact = EmbeddingColumnarBudgets::new(
            u64::MAX,
            estimates.retained_bytes,
            estimates.row_group_bytes,
            estimates.decoded_bytes,
        );
        assert_eq!(
            enforce_writer_estimates(row_count, estimates, exact),
            Ok(())
        );
        let decoded_short = EmbeddingColumnarBudgets::new(
            u64::MAX,
            estimates.retained_bytes,
            estimates.row_group_bytes,
            estimates.decoded_bytes - 1,
        );
        assert!(matches!(
            enforce_writer_estimates(row_count, estimates, decoded_short),
            Err(MultiscaleColumnarError::DecodedByteBudgetExceeded { .. })
        ));
        let group_short = EmbeddingColumnarBudgets::new(
            u64::MAX,
            estimates.retained_bytes,
            estimates.row_group_bytes - 1,
            estimates.decoded_bytes,
        );
        assert!(matches!(
            enforce_writer_estimates(row_count, estimates, group_short),
            Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded { .. })
        ));
        let retained_short = EmbeddingColumnarBudgets::new(
            u64::MAX,
            estimates.retained_bytes - 1,
            estimates.row_group_bytes,
            estimates.decoded_bytes,
        );
        assert!(matches!(
            enforce_writer_estimates(row_count, estimates, retained_short),
            Err(MultiscaleColumnarError::RetainedByteBudgetExceeded { .. })
        ));
    }
}
