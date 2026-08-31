use std::{io::Write, sync::Arc};

use arrow::{
    array::{ArrayRef, StringBuilder, UInt64Builder},
    record_batch::RecordBatch,
};
use parquet::arrow::arrow_writer::{ArrowWriter, ArrowWriterOptions};

use crate::{CellPatchAssignmentMode, CellPatchLink};

use super::super::writer_resources::enforce_footer_bound;
use super::profile::{assignment_schema, edge_schema, writer_properties, CellPatchParquetProfile};
use crate::columnar::{
    multiscale::{
        assignment_decoded_bytes, edge_decoded_bytes, enforce_decoded_budget,
        enforce_retained_budget, enforce_row_group_budget, validate_cell_patch_domain,
    },
    parquet::{
        compact::canonical_compact_len,
        profile::{MAXIMUM_FOOTER_BYTES, PUBLIC_BATCH_ROWS, ROW_GROUP_ROWS},
        writer::{
            estimate_metadata_retained_bytes_for_columns, ParquetDigestingWriter,
            WRITER_FIXED_RETAINED_BYTES,
        },
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialColumnarWriteSummary,
};

/// Write the exact deterministic Parquet cell-patch assignment-table profile.
pub fn write_cell_patch_assignment_table_parquet(
    output: &mut (dyn Write + Send),
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_cell_patch_domain(link)?;
    enforce_footer_bound(link.assignment_count())?;
    let estimates = assignment_estimates(link)?;
    enforce_writer_estimates(estimates, budgets)?;
    let schema = Arc::new(assignment_schema());
    write_parquet(
        output,
        CellPatchParquetProfile::Assignment,
        schema,
        link,
        link.assignment_count(),
        budgets,
        |schema, start, end| build_assignment_batch(schema, link, start, end),
    )
}

/// Write the exact deterministic Parquet cell-patch edge-table profile.
pub fn write_cell_patch_edge_table_parquet(
    output: &mut (dyn Write + Send),
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_cell_patch_domain(link)?;
    enforce_footer_bound(link.edge_count())?;
    let estimates = edge_estimates(link)?;
    enforce_writer_estimates(estimates, budgets)?;
    let schema = Arc::new(edge_schema());
    write_parquet(
        output,
        CellPatchParquetProfile::Edge,
        schema,
        link,
        link.edge_count(),
        budgets,
        |schema, start, end| build_edge_batch(schema, link, start, end),
    )
}

#[allow(clippy::too_many_arguments)]
fn write_parquet<F>(
    output: &mut (dyn Write + Send),
    profile: CellPatchParquetProfile,
    schema: Arc<arrow::datatypes::Schema>,
    link: &CellPatchLink,
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
        .with_properties(writer_properties(profile, link)?)
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
            writer
                .write(&build_batch(&schema, start, end)?)
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

fn build_assignment_batch(
    schema: &arrow::datatypes::Schema,
    link: &CellPatchLink,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = link
        .assignments()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (cell_text, status_text) =
        rows.iter()
            .try_fold((0_usize, 0_usize), |(cells, statuses), row| {
                Ok::<_, MultiscaleColumnarError>((
                    cells
                        .checked_add(row.cell_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    statuses
                        .checked_add(row.status().wire_name().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                ))
            })?;
    let mut cells = StringBuilder::with_capacity(rows.len(), cell_text);
    let mut statuses = StringBuilder::with_capacity(rows.len(), status_text);
    let mut x = UInt64Builder::with_capacity(rows.len());
    let mut y = UInt64Builder::with_capacity(rows.len());
    let mut starts = UInt64Builder::with_capacity(rows.len());
    let mut counts = UInt64Builder::with_capacity(rows.len());
    for row in rows {
        cells.append_value(row.cell_id().as_str());
        statuses.append_value(row.status().wire_name());
        x.append_value(row.anchor_px()[0].to_bits());
        y.append_value(row.anchor_px()[1].to_bits());
        starts.append_value(row.edge_start());
        counts.append_value(row.edge_count());
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(cells.finish()),
        Arc::new(statuses.finish()),
        Arc::new(x.finish()),
        Arc::new(y.finish()),
        Arc::new(starts.finish()),
        Arc::new(counts.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ParquetWriter)
}

fn build_edge_batch(
    schema: &arrow::datatypes::Schema,
    link: &CellPatchLink,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = link
        .edges()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let patch_text = rows.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.patch_id().as_str().len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    let mut assignments = UInt64Builder::with_capacity(rows.len());
    let mut patches = StringBuilder::with_capacity(rows.len(), patch_text);
    let mut numerators = UInt64Builder::with_capacity(rows.len());
    let mut denominators = UInt64Builder::with_capacity(rows.len());
    for row in rows {
        assignments.append_value(row.assignment_row());
        patches.append_value(row.patch_id().as_str());
        match row.weight() {
            Some(weight) => {
                numerators.append_value(weight.numerator());
                denominators.append_value(weight.denominator());
            }
            None => {
                numerators.append_null();
                denominators.append_null();
            }
        }
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(assignments.finish()),
        Arc::new(patches.finish()),
        Arc::new(numerators.finish()),
        Arc::new(denominators.finish()),
    ];
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
    plain_value_bytes: usize,
    level_bytes: usize,
}

fn assignment_estimates(link: &CellPatchLink) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_writer(
        link.assignment_count(),
        6,
        link.assignments().chunks(ROW_GROUP_ROWS).map(|rows| {
            let cell_text = rows.iter().try_fold(0_usize, |total, row| {
                total
                    .checked_add(row.cell_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)
            })?;
            let status_text = rows.iter().try_fold(0_usize, |total, row| {
                total
                    .checked_add(row.status().wire_name().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)
            })?;
            let plain = rows
                .len()
                .checked_mul(40)
                .and_then(|value| value.checked_add(cell_text))
                .and_then(|value| value.checked_add(status_text))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            Ok(ChunkEstimate {
                decoded_bytes: assignment_decoded_bytes(rows)?,
                plain_value_bytes: plain,
                level_bytes: 0,
            })
        }),
    )
}

fn edge_estimates(link: &CellPatchLink) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_writer(
        link.edge_count(),
        4,
        link.edges().chunks(ROW_GROUP_ROWS).map(|rows| {
            let patch_text = rows.iter().try_fold(0_usize, |total, row| {
                total
                    .checked_add(row.patch_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)
            })?;
            let fixed = match link.mode() {
                CellPatchAssignmentMode::ContainedShared => 12,
                CellPatchAssignmentMode::DeclaredWeightedInterpolation => 28,
            };
            let plain = rows
                .len()
                .checked_mul(fixed)
                .and_then(|value| value.checked_add(patch_text))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let bitmap = rows
                .len()
                .checked_add(7)
                .map(|value| value / 8)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            Ok(ChunkEstimate {
                decoded_bytes: edge_decoded_bytes(rows)?,
                plain_value_bytes: plain,
                level_bytes: bitmap
                    .checked_mul(4)
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
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
    let mut maximum_input = 0_usize;
    let mut maximum_encoded_values = 0_usize;
    for chunk in chunks {
        let chunk = chunk?;
        decoded_bytes = decoded_bytes
            .checked_add(
                u64::try_from(chunk.decoded_bytes)
                    .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum_input = maximum_input.max(chunk.decoded_bytes);
        maximum_encoded_values = maximum_encoded_values.max(
            chunk
                .plain_value_bytes
                .checked_add(chunk.level_bytes)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
    }
    let group_rows = row_count.min(ROW_GROUP_ROWS);
    let page_overhead = group_rows
        .checked_mul(columns)
        .and_then(|value| value.checked_mul(1024 + 4 * size_of::<bytes::Bytes>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let encoder_workspace = group_rows
        .checked_mul(16)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let retained_chunks = maximum_encoded_values
        .checked_mul(2)
        .and_then(|value| value.checked_add(page_overhead))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let page_transient = maximum_encoded_values
        .checked_mul(4)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let row_group_bytes = maximum_input
        .checked_add(encoder_workspace)
        .and_then(|value| value.checked_add(retained_chunks))
        .and_then(|value| value.checked_add(page_transient))
        .and_then(|value| value.checked_add(WRITER_FIXED_RETAINED_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let metadata =
        estimate_metadata_retained_bytes_for_columns(row_count.div_ceil(ROW_GROUP_ROWS), columns)
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let retained_bytes = row_group_bytes
        .checked_add(metadata)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?
        .max(
            metadata
                .checked_add(WRITER_FIXED_RETAINED_BYTES)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
    Ok(WriterEstimates {
        decoded_bytes,
        row_group_bytes,
        retained_bytes,
    })
}

fn enforce_writer_estimates(
    estimates: WriterEstimates,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    enforce_decoded_budget(estimates.decoded_bytes, budgets)?;
    enforce_row_group_budget(estimates.row_group_bytes, budgets)?;
    enforce_retained_budget(estimates.retained_bytes, budgets)
}
