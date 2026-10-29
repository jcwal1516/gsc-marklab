use crate::columnar::parquet::multiscale::writer_resources::enforce_footer_bound;
use std::{io::Write, mem::size_of, sync::Arc};

use arrow::{
    array::{ArrayRef, FixedSizeListBuilder, Float32Builder, StringBuilder},
    datatypes::{DataType, Schema},
    record_batch::RecordBatch,
};
use parquet::arrow::arrow_writer::{ArrowWriter, ArrowWriterOptions};

use crate::{PatchEmbeddingTable, RegionEmbeddingTable, SlideEmbeddingTable};

use super::profile::{schema, writer_properties, COLUMN_COUNT};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
        matrix_decoded_bytes, validate_matrix_domain, MultiscaleMatrixTable,
    },
    parquet::{
        profile::{MAXIMUM_FOOTER_BYTES, PUBLIC_BATCH_ROWS, ROW_GROUP_ROWS},
        writer::{
            estimate_metadata_retained_bytes_for_columns, ParquetDigestingWriter,
            WRITER_FIXED_RETAINED_BYTES,
        },
    },
    ColumnarError, ColumnarWriteSummary, EmbeddingColumnarBudgets,
};

macro_rules! typed_writer {
    ($name:ident, $table:ty) => {
        #[doc = "Write one exact deterministic multiscale Parquet matrix profile."]
        pub fn $name(
            output: &mut (dyn Write + Send),
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<ColumnarWriteSummary, ColumnarError> {
            write_matrix_parquet(output, table, budgets)
        }
    };
}

typed_writer!(write_patch_embedding_table_parquet, PatchEmbeddingTable);
typed_writer!(write_region_embedding_table_parquet, RegionEmbeddingTable);
typed_writer!(write_slide_embedding_table_parquet, SlideEmbeddingTable);

fn write_matrix_parquet(
    output: &mut (dyn Write + Send),
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ColumnarWriteSummary, ColumnarError> {
    validate_matrix_domain(table)?;
    enforce_footer_bound(table.row_count())?;
    let estimates = writer_estimates(table)?;
    enforce_decoded_budget(estimates.decoded_bytes, budgets)?;
    enforce_row_group_budget(estimates.row_group_bytes, budgets)?;
    enforce_retained_budget(estimates.retained_bytes, budgets)?;

    let schema = Arc::new(schema(table)?);
    let options = ArrowWriterOptions::new()
        .with_properties(writer_properties(table)?)
        .with_skip_arrow_metadata(true)
        .with_schema_root(table.profile().parquet_root().to_owned());
    let mut sink = ParquetDigestingWriter::new(output, budgets.maximum_file_bytes());
    let result = (|| {
        let mut writer = ArrowWriter::try_new_with_options(&mut sink, Arc::clone(&schema), options)
            .map_err(|_| ColumnarError::ParquetWriter)?;
        let mut start = 0_usize;
        while start < table.row_count() {
            let end = start
                .checked_add(PUBLIC_BATCH_ROWS)
                .map(|value| value.min(table.row_count()))
                .ok_or(ColumnarError::SizeOverflow)?;
            writer
                .write(&build_batch(&schema, table, start, end)?)
                .map_err(|_| ColumnarError::ParquetWriter)?;
            start = end;
        }
        writer.close().map_err(|_| ColumnarError::ParquetWriter)?;
        sink.require_footer_within(MAXIMUM_FOOTER_BYTES)
    })();
    if let Some(observed) = sink.budget_exceeded() {
        return Err(ColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    result?;
    let (digest, encoded_len) = sink.finish();
    Ok(ColumnarWriteSummary::new(
        digest,
        encoded_len,
        u64::try_from(table.row_count()).map_err(|_| ColumnarError::SizeOverflow)?,
        table.dimension(),
    ))
}

fn build_batch(
    schema: &Schema,
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    end: usize,
) -> Result<RecordBatch, ColumnarError> {
    let rows = end.checked_sub(start).ok_or(ColumnarError::SizeOverflow)?;
    let dimension = usize::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?;
    let components = rows
        .checked_mul(dimension)
        .ok_or(ColumnarError::SizeOverflow)?;
    let (id_bytes, status_bytes) = text_bytes(table, start, end)?;
    let mut ids = StringBuilder::with_capacity(rows, id_bytes);
    let values = Float32Builder::with_capacity(components);
    let list_field = match schema.field(1).data_type() {
        DataType::FixedSizeList(field, length)
            if *length
                == i32::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)? =>
        {
            Arc::clone(field)
        }
        _ => return Err(ColumnarError::ParquetWriter),
    };
    let mut embeddings = FixedSizeListBuilder::with_capacity(
        values,
        i32::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?,
        rows,
    )
    .with_field(list_field);
    let mut statuses = StringBuilder::with_capacity(rows, status_bytes);
    for index in start..end {
        let row = table
            .row(index)
            .map_err(|_| ColumnarError::ArtifactBindingMismatch)?;
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
        .map_err(|_| ColumnarError::ParquetWriter)
}

#[derive(Clone, Copy)]
struct WriterEstimates {
    decoded_bytes: u64,
    row_group_bytes: usize,
    retained_bytes: usize,
}

fn writer_estimates(table: &dyn MultiscaleMatrixTable) -> Result<WriterEstimates, ColumnarError> {
    let dimension = usize::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?;
    let mut decoded_bytes = 0_u64;
    let mut maximum_input = 0_usize;
    let mut maximum_plain = 0_usize;
    let mut start = 0_usize;
    while start < table.row_count() {
        let end = start
            .checked_add(ROW_GROUP_ROWS)
            .map(|value| value.min(table.row_count()))
            .ok_or(ColumnarError::SizeOverflow)?;
        let decoded = matrix_decoded_bytes(table, start, end)?;
        decoded_bytes = decoded_bytes
            .checked_add(u64::try_from(decoded).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)?;
        maximum_input = maximum_input.max(decoded);
        let rows = end - start;
        let components = rows
            .checked_mul(dimension)
            .ok_or(ColumnarError::SizeOverflow)?;
        let (ids, statuses) = text_bytes(table, start, end)?;
        let plain = components
            .checked_mul(size_of::<f32>())
            .and_then(|value| value.checked_add(rows.checked_mul(2 * size_of::<u32>())?))
            .and_then(|value| value.checked_add(ids))
            .and_then(|value| value.checked_add(statuses))
            .ok_or(ColumnarError::SizeOverflow)?;
        maximum_plain = maximum_plain.max(plain);
        start = end;
    }
    let group_rows = table.row_count().min(ROW_GROUP_ROWS);
    let group_components = group_rows
        .checked_mul(dimension)
        .ok_or(ColumnarError::SizeOverflow)?;
    let level_bytes = bitmap_bytes(group_components)?
        .checked_mul(4)
        .ok_or(ColumnarError::SizeOverflow)?;
    let page_overhead = group_rows
        .checked_mul(COLUMN_COUNT)
        .and_then(|value| value.checked_mul(1024 + 4 * size_of::<bytes::Bytes>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let level_workspace = group_components
        .checked_mul(16)
        .ok_or(ColumnarError::SizeOverflow)?;
    let plain_and_levels = maximum_plain
        .checked_add(level_bytes)
        .ok_or(ColumnarError::SizeOverflow)?;
    let row_group_bytes = maximum_input
        .checked_add(level_workspace)
        .and_then(|value| value.checked_add(plain_and_levels.checked_mul(2)?))
        .and_then(|value| value.checked_add(page_overhead))
        .and_then(|value| value.checked_add(plain_and_levels.checked_mul(4)?))
        .and_then(|value| value.checked_add(WRITER_FIXED_RETAINED_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    let metadata = estimate_metadata_retained_bytes_for_columns(
        table.row_count().div_ceil(ROW_GROUP_ROWS),
        COLUMN_COUNT,
    )
    .map_err(|_| ColumnarError::SizeOverflow)?;
    let retained_bytes = row_group_bytes
        .checked_add(metadata)
        .ok_or(ColumnarError::SizeOverflow)?
        .max(
            metadata
                .checked_add(WRITER_FIXED_RETAINED_BYTES)
                .ok_or(ColumnarError::SizeOverflow)?,
        );
    Ok(WriterEstimates {
        decoded_bytes,
        row_group_bytes,
        retained_bytes,
    })
}

fn text_bytes(
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    end: usize,
) -> Result<(usize, usize), ColumnarError> {
    (start..end).try_fold((0_usize, 0_usize), |(ids, statuses), index| {
        let row = table
            .row(index)
            .map_err(|_| ColumnarError::ArtifactBindingMismatch)?;
        Ok((
            ids.checked_add(row.id().len())
                .ok_or(ColumnarError::SizeOverflow)?,
            statuses
                .checked_add(row.status().wire_name().len())
                .ok_or(ColumnarError::SizeOverflow)?,
        ))
    })
}

fn bitmap_bytes(values: usize) -> Result<usize, ColumnarError> {
    values
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(ColumnarError::SizeOverflow)
}
