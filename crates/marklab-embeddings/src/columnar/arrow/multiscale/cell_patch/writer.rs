use std::{io::Write, sync::Arc};

use crate::{CellPatchAssignmentMode, CellPatchLink};
use arrow::{
    array::{ArrayRef, StringBuilder, UInt64Array, UInt64Builder},
    buffer::{BooleanBuffer, NullBuffer},
    datatypes::Schema,
    record_batch::RecordBatch,
};

use super::super::physical::write_record_batches;
use super::profile::{assignment_schema, edge_schema};
use crate::columnar::arrow::profile::{
    ALIGNMENT, MAXIMUM_FOOTER_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
};
use crate::columnar::{
    multiscale::{
        assignment_decoded_bytes, edge_decoded_bytes, enforce_decoded_budget,
        enforce_retained_budget, enforce_row_group_budget, validate_cell_patch_domain,
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialColumnarWriteSummary,
};

/// Write the exact deterministic Arrow IPC cell-patch assignment-table profile.
pub fn write_cell_patch_assignment_table_arrow(
    output: &mut dyn Write,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_cell_patch_domain(link)?;
    enforce_footer_bound(link.assignment_count())?;
    let estimates = assignment_estimates(link)?;
    enforce_writer_estimates(link.assignment_count(), estimates, budgets)?;
    let schema = assignment_schema(link)?;
    write_arrow(
        output,
        &schema,
        link.assignment_count(),
        budgets,
        |schema, start, end| build_assignment_batch(schema, link, start, end),
    )
}

/// Write the exact deterministic Arrow IPC cell-patch edge-table profile.
pub fn write_cell_patch_edge_table_arrow(
    output: &mut dyn Write,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_cell_patch_domain(link)?;
    enforce_footer_bound(link.edge_count())?;
    let estimates = edge_estimates(link)?;
    enforce_writer_estimates(link.edge_count(), estimates, budgets)?;
    let schema = edge_schema(link)?;
    write_arrow(
        output,
        &schema,
        link.edge_count(),
        budgets,
        |schema, start, end| build_edge_batch(schema, link, start, end),
    )
}

fn write_arrow<F>(
    output: &mut dyn Write,
    schema: &Schema,
    row_count: usize,
    budgets: EmbeddingColumnarBudgets,
    build_batch: F,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError>
where
    F: FnMut(&Schema, usize, usize) -> Result<RecordBatch, MultiscaleColumnarError>,
{
    let encoded = write_record_batches(output, schema, row_count, budgets, build_batch)?;
    Ok(SpatialColumnarWriteSummary::new(
        encoded.content_digest,
        encoded.encoded_byte_len,
        encoded.row_count,
    ))
}

fn build_assignment_batch(
    schema: &Schema,
    link: &CellPatchLink,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let selected = link
        .assignments()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (cell_text, status_text) =
        selected
            .iter()
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
    let mut cells = StringBuilder::with_capacity(selected.len(), cell_text);
    let mut statuses = StringBuilder::with_capacity(selected.len(), status_text);
    let mut x = UInt64Builder::with_capacity(selected.len());
    let mut y = UInt64Builder::with_capacity(selected.len());
    let mut edge_start = UInt64Builder::with_capacity(selected.len());
    let mut edge_count = UInt64Builder::with_capacity(selected.len());
    for row in selected {
        cells.append_value(row.cell_id().as_str());
        statuses.append_value(row.status().wire_name());
        x.append_value(row.anchor_px()[0].to_bits());
        y.append_value(row.anchor_px()[1].to_bits());
        edge_start.append_value(row.edge_start());
        edge_count.append_value(row.edge_count());
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(cells.finish()),
        Arc::new(statuses.finish()),
        Arc::new(x.finish()),
        Arc::new(y.finish()),
        Arc::new(edge_start.finish()),
        Arc::new(edge_count.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ArrowWriter)
}

fn build_edge_batch(
    schema: &Schema,
    link: &CellPatchLink,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let selected = link
        .edges()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let patch_text = selected.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.patch_id().as_str().len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    let mut assignment_rows = UInt64Builder::with_capacity(selected.len());
    let mut patch_ids = StringBuilder::with_capacity(selected.len(), patch_text);
    let mut numerators = Vec::new();
    numerators.try_reserve_exact(selected.len()).map_err(|_| {
        MultiscaleColumnarError::AllocationFailed {
            requested: selected.len().saturating_mul(size_of::<u64>()),
        }
    })?;
    let mut denominators = Vec::new();
    denominators
        .try_reserve_exact(selected.len())
        .map_err(|_| MultiscaleColumnarError::AllocationFailed {
            requested: selected.len().saturating_mul(size_of::<u64>()),
        })?;
    for row in selected {
        assignment_rows.append_value(row.assignment_row());
        patch_ids.append_value(row.patch_id().as_str());
        numerators.push(row.weight().map_or(0, |weight| weight.numerator()));
        denominators.push(row.weight().map_or(0, |weight| weight.denominator()));
    }
    let validity = explicit_weight_validity(link.mode(), selected.len())?;
    let numerator_array = UInt64Array::new(numerators.into(), Some(validity.clone()));
    let denominator_array = UInt64Array::new(denominators.into(), Some(validity));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(assignment_rows.finish()),
        Arc::new(patch_ids.finish()),
        Arc::new(numerator_array),
        Arc::new(denominator_array),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ArrowWriter)
}

fn explicit_weight_validity(
    mode: CellPatchAssignmentMode,
    rows: usize,
) -> Result<NullBuffer, MultiscaleColumnarError> {
    let byte_count = rows
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let bytes = vec![
        match mode {
            CellPatchAssignmentMode::ContainedShared => 0,
            CellPatchAssignmentMode::DeclaredWeightedInterpolation => u8::MAX,
        };
        byte_count
    ];
    Ok(NullBuffer::new(BooleanBuffer::new(bytes.into(), 0, rows)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WriterEstimates {
    decoded_bytes: u64,
    maximum_batch_bytes: usize,
}

fn assignment_estimates(link: &CellPatchLink) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_chunks(
        link.assignments()
            .chunks(RECORD_BATCH_ROWS)
            .map(|rows| assignment_decoded_bytes(rows).map(|bytes| (bytes, 14))),
    )
}

fn edge_estimates(link: &CellPatchLink) -> Result<WriterEstimates, MultiscaleColumnarError> {
    estimate_chunks(
        link.edges()
            .chunks(RECORD_BATCH_ROWS)
            .map(|rows| edge_decoded_bytes(rows).map(|bytes| (bytes, 9))),
    )
}

fn estimate_chunks(
    chunks: impl Iterator<Item = Result<(usize, usize), MultiscaleColumnarError>>,
) -> Result<WriterEstimates, MultiscaleColumnarError> {
    let mut decoded = 0_u64;
    let mut maximum = 0_usize;
    for chunk in chunks {
        let (bytes, buffer_count) = chunk?;
        decoded = decoded
            .checked_add(u64::try_from(bytes).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum = maximum.max(
            bytes
                .checked_add(buffer_count * (ALIGNMENT - 1))
                .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
    }
    Ok(WriterEstimates {
        decoded_bytes: decoded,
        maximum_batch_bytes: maximum,
    })
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

fn enforce_footer_bound(row_count: usize) -> Result<(), MultiscaleColumnarError> {
    const FOOTER_BASE_BYTES: usize = 64 * 1024;
    let footer_bytes = FOOTER_BASE_BYTES
        .checked_add(
            row_count
                .div_ceil(RECORD_BATCH_ROWS)
                .checked_mul(size_of::<arrow_ipc::Block>())
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        )
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if footer_bytes > MAXIMUM_FOOTER_BYTES {
        return Err(MultiscaleColumnarError::ArrowWriter);
    }
    Ok(())
}
