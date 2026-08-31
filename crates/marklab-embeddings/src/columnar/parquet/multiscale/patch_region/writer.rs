use std::{io::Write, mem::size_of, sync::Arc};

use arrow::{
    array::{ArrayRef, StringBuilder, UInt64Builder},
    record_batch::RecordBatch,
};
use parquet::arrow::arrow_writer::{ArrowWriter, ArrowWriterOptions};

use crate::{PatchRegionDeclaration, PatchRegionLink};

use super::super::writer_resources::enforce_footer_bound;
use super::profile::{schema, writer_properties, COLUMN_COUNT, ROOT};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
        patch_region_decoded_bytes, validate_patch_region_domain,
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

/// Write the exact deterministic Parquet patch-region-link profile.
pub fn write_patch_region_link_parquet(
    output: &mut (dyn Write + Send),
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_patch_region_domain(link)?;
    let row_count = link.nonzero_relation_count();
    enforce_footer_bound(row_count)?;
    let estimates = writer_estimates(link)?;
    enforce_decoded_budget(estimates.decoded_bytes, budgets)?;
    enforce_row_group_budget(estimates.row_group_bytes, budgets)?;
    enforce_retained_budget(estimates.retained_bytes, budgets)?;

    let schema = Arc::new(schema());
    let options = ArrowWriterOptions::new()
        .with_properties(writer_properties(link)?)
        .with_skip_arrow_metadata(true)
        .with_schema_root(ROOT.to_owned());
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
                .write(&build_batch(&schema, link, start, end)?)
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

fn build_batch(
    schema: &arrow::datatypes::Schema,
    link: &PatchRegionLink,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = link
        .nonzero_relations()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (patch_text, region_text, relation_text) = text_bytes(rows)?;
    let mut patches = StringBuilder::with_capacity(rows.len(), patch_text);
    let mut regions = StringBuilder::with_capacity(rows.len(), region_text);
    let mut relations = StringBuilder::with_capacity(rows.len(), relation_text);
    let mut numerators = UInt64Builder::with_capacity(rows.len());
    let mut denominators = UInt64Builder::with_capacity(rows.len());
    for row in rows {
        patches.append_value(row.patch_id().as_str());
        regions.append_value(row.region_id().as_str());
        relations.append_value(row.relation().wire_name());
        numerators.append_value(row.numerator());
        denominators.append_value(row.denominator());
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(patches.finish()),
        Arc::new(regions.finish()),
        Arc::new(relations.finish()),
        Arc::new(numerators.finish()),
        Arc::new(denominators.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| MultiscaleColumnarError::ParquetWriter)
}

#[derive(Clone, Copy)]
struct WriterEstimates {
    decoded_bytes: u64,
    row_group_bytes: usize,
    retained_bytes: usize,
}

fn writer_estimates(link: &PatchRegionLink) -> Result<WriterEstimates, MultiscaleColumnarError> {
    let rows = link.nonzero_relations();
    let mut decoded_bytes = 0_u64;
    let mut maximum_input = 0_usize;
    let mut maximum_plain = 0_usize;
    for chunk in rows.chunks(ROW_GROUP_ROWS) {
        let decoded = patch_region_decoded_bytes(chunk)?;
        decoded_bytes = decoded_bytes
            .checked_add(u64::try_from(decoded).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum_input = maximum_input.max(decoded);
        let (patch_text, region_text, relation_text) = text_bytes(chunk)?;
        let plain = chunk
            .len()
            .checked_mul(28)
            .and_then(|value| value.checked_add(patch_text))
            .and_then(|value| value.checked_add(region_text))
            .and_then(|value| value.checked_add(relation_text))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum_plain = maximum_plain.max(plain);
    }
    let group_rows = rows.len().min(ROW_GROUP_ROWS);
    let page_overhead = group_rows
        .checked_mul(COLUMN_COUNT)
        .and_then(|value| value.checked_mul(1024 + 4 * size_of::<bytes::Bytes>()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let encoder_workspace = group_rows
        .checked_mul(16)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let retained_chunks = maximum_plain
        .checked_mul(2)
        .and_then(|value| value.checked_add(page_overhead))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let page_transient = maximum_plain
        .checked_mul(4)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let row_group_bytes = maximum_input
        .checked_add(encoder_workspace)
        .and_then(|value| value.checked_add(retained_chunks))
        .and_then(|value| value.checked_add(page_transient))
        .and_then(|value| value.checked_add(WRITER_FIXED_RETAINED_BYTES))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let metadata = estimate_metadata_retained_bytes_for_columns(
        rows.len().div_ceil(ROW_GROUP_ROWS),
        COLUMN_COUNT,
    )
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

fn text_bytes(
    rows: &[PatchRegionDeclaration],
) -> Result<(usize, usize, usize), MultiscaleColumnarError> {
    rows.iter().try_fold(
        (0_usize, 0_usize, 0_usize),
        |(patches, regions, relations), row| {
            Ok((
                patches
                    .checked_add(row.patch_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                regions
                    .checked_add(row.region_id().as_str().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                relations
                    .checked_add(row.relation().wire_name().len())
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?,
            ))
        },
    )
}
