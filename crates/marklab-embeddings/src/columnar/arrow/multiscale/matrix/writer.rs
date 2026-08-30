use std::{io::Write, mem::size_of, sync::Arc};

use crate::{PatchEmbeddingTable, RegionEmbeddingTable, SlideEmbeddingTable};
use arrow::{
    array::{ArrayRef, FixedSizeListBuilder, Float32Builder, StringBuilder},
    datatypes::{DataType, Schema},
    record_batch::RecordBatch,
};

use super::super::physical::write_record_batches;
use super::profile::{schema, BUFFER_COUNT};
use crate::columnar::arrow::profile::{
    ALIGNMENT, MAXIMUM_FOOTER_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
        matrix_decoded_bytes, validate_matrix_domain, MultiscaleMatrixTable,
    },
    ColumnarWriteSummary, EmbeddingColumnarBudgets, MultiscaleColumnarError,
};

macro_rules! typed_writer {
    ($name:ident, $table:ty) => {
        #[doc = "Write one exact deterministic C-05 Arrow matrix profile."]
        pub fn $name(
            output: &mut dyn Write,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<ColumnarWriteSummary, MultiscaleColumnarError> {
            write_matrix_arrow(output, table, budgets)
        }
    };
}

typed_writer!(write_patch_embedding_table_arrow, PatchEmbeddingTable);
typed_writer!(write_region_embedding_table_arrow, RegionEmbeddingTable);
typed_writer!(write_slide_embedding_table_arrow, SlideEmbeddingTable);

fn write_matrix_arrow(
    output: &mut dyn Write,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ColumnarWriteSummary, MultiscaleColumnarError> {
    validate_matrix_domain(table)?;
    let row_count = table.row_count();
    enforce_footer_bound(row_count)?;
    let estimates = estimates(table)?;
    enforce_estimates(row_count, estimates, budgets)?;
    let schema = schema(table)?;
    let encoded =
        write_record_batches(output, &schema, row_count, budgets, |schema, start, end| {
            build_batch(schema, table, start, end)
        })?;
    Ok(ColumnarWriteSummary::new(
        encoded.content_digest,
        encoded.encoded_byte_len,
        encoded.row_count,
        table.dimension(),
    ))
}

fn build_batch(
    schema: &Schema,
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = end
        .checked_sub(start)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let dimension =
        usize::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let component_count = rows
        .checked_mul(dimension)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (id_bytes, status_bytes) =
        (start..end).try_fold((0_usize, 0_usize), |(id_total, status_total), index| {
            let row = table
                .row(index)
                .map_err(|_| MultiscaleColumnarError::ArtifactBindingMismatch)?;
            Ok::<_, MultiscaleColumnarError>((
                id_total
                    .checked_add(row.id().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                status_total
                    .checked_add(row.status().wire_name().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
            ))
        })?;
    let mut ids = StringBuilder::with_capacity(rows, id_bytes);
    let values = Float32Builder::with_capacity(component_count);
    let list_field = match schema.field(1).data_type() {
        DataType::FixedSizeList(field, observed)
            if *observed
                == i32::try_from(table.dimension())
                    .map_err(|_| MultiscaleColumnarError::SizeOverflow)? =>
        {
            Arc::clone(field)
        }
        _ => return Err(MultiscaleColumnarError::ArrowWriter),
    };
    let mut embeddings = FixedSizeListBuilder::with_capacity(
        values,
        i32::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        rows,
    )
    .with_field(list_field);
    let mut statuses = StringBuilder::with_capacity(rows, status_bytes);
    for index in start..end {
        let row = table
            .row(index)
            .map_err(|_| MultiscaleColumnarError::ArtifactBindingMismatch)?;
        ids.append_value(row.id());
        if let Some(vector) = row.vector() {
            embeddings.values().append_slice(vector);
        } else {
            for _ in 0..dimension {
                embeddings.values().append_value(0.0);
            }
        }
        embeddings.append(true);
        statuses.append_value(row.status().wire_name());
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(ids.finish()),
        Arc::new(embeddings.finish()),
        Arc::new(statuses.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ArrowWriter)
}

#[derive(Clone, Copy)]
struct Estimates {
    decoded: u64,
    maximum_batch: usize,
}

fn estimates(table: &dyn MultiscaleMatrixTable) -> Result<Estimates, MultiscaleColumnarError> {
    let mut decoded = 0_u64;
    let mut maximum_batch = 0_usize;
    let mut start = 0_usize;
    while start < table.row_count() {
        let end = start
            .checked_add(RECORD_BATCH_ROWS)
            .map(|value| value.min(table.row_count()))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let bytes = matrix_decoded_bytes(table, start, end)?;
        decoded = decoded
            .checked_add(u64::try_from(bytes).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum_batch = maximum_batch.max(
            bytes
                .checked_add(BUFFER_COUNT * (ALIGNMENT - 1))
                .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
        start = end;
    }
    Ok(Estimates {
        decoded,
        maximum_batch,
    })
}

fn enforce_estimates(
    row_count: usize,
    estimates: Estimates,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    enforce_decoded_budget(estimates.decoded, budgets)?;
    enforce_row_group_budget(estimates.maximum_batch, budgets)?;
    let record_blocks = row_count
        .div_ceil(RECORD_BATCH_ROWS)
        .checked_mul(size_of::<arrow_ipc::Block>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let block_capacity = record_blocks
        .checked_mul(2)
        .and_then(|value| value.checked_add(size_of::<Vec<arrow_ipc::Block>>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let batch_peak = estimates
        .maximum_batch
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
    let record_blocks = row_count
        .div_ceil(RECORD_BATCH_ROWS)
        .checked_mul(size_of::<arrow_ipc::Block>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let footer_bound = record_blocks
        .checked_add(MAXIMUM_MESSAGE_BYTES)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if footer_bound > MAXIMUM_FOOTER_BYTES {
        return Err(MultiscaleColumnarError::ArrowWriter);
    }
    Ok(())
}
