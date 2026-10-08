use std::{io::Write, sync::Arc};

use crate::PatchRegionLink;
use arrow::{
    array::{ArrayRef, StringBuilder, UInt64Builder},
    datatypes::Schema,
    record_batch::RecordBatch,
};

use super::super::physical::{enforce_writer_budgets, write_record_batches};
use super::profile::schema;
use crate::columnar::arrow::profile::{
    ALIGNMENT, MAXIMUM_FOOTER_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
};
use crate::columnar::{
    multiscale::{patch_region_decoded_bytes, validate_patch_region_domain},
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialColumnarWriteSummary,
};

/// Write the exact deterministic Arrow IPC patch-region-link profile.
pub fn write_patch_region_link_arrow(
    output: &mut dyn Write,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError> {
    validate_patch_region_domain(link)?;
    let row_count = link.nonzero_relation_count();
    enforce_footer_bound(row_count)?;
    let estimates = estimates(link)?;
    enforce_writer_budgets(
        row_count,
        estimates.decoded,
        estimates.maximum_batch,
        budgets,
    )?;
    let schema = schema(link)?;
    let encoded =
        write_record_batches(output, &schema, row_count, budgets, |schema, start, end| {
            build_batch(schema, link, start, end)
        })?;
    Ok(SpatialColumnarWriteSummary::new(
        encoded.content_digest,
        encoded.encoded_byte_len,
        encoded.row_count,
    ))
}

fn build_batch(
    schema: &Schema,
    link: &PatchRegionLink,
    start: usize,
    end: usize,
) -> Result<RecordBatch, MultiscaleColumnarError> {
    let rows = link
        .nonzero_relations()
        .get(start..end)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let (patch_bytes, region_bytes, relation_bytes) =
        rows.iter()
            .try_fold((0_usize, 0_usize, 0_usize), |totals, row| {
                Ok::<_, MultiscaleColumnarError>((
                    totals
                        .0
                        .checked_add(row.patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    totals
                        .1
                        .checked_add(row.region_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                    totals
                        .2
                        .checked_add(row.relation().wire_name().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                ))
            })?;
    let mut patches = StringBuilder::with_capacity(rows.len(), patch_bytes);
    let mut regions = StringBuilder::with_capacity(rows.len(), region_bytes);
    let mut relations = StringBuilder::with_capacity(rows.len(), relation_bytes);
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
        .map_err(|_| MultiscaleColumnarError::ArrowWriter)
}

#[derive(Clone, Copy)]
struct Estimates {
    decoded: u64,
    maximum_batch: usize,
}

fn estimates(link: &PatchRegionLink) -> Result<Estimates, MultiscaleColumnarError> {
    let mut decoded = 0_u64;
    let mut maximum_batch = 0_usize;
    for rows in link.nonzero_relations().chunks(RECORD_BATCH_ROWS) {
        let bytes = patch_region_decoded_bytes(rows)?;
        decoded = decoded
            .checked_add(u64::try_from(bytes).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        maximum_batch = maximum_batch.max(
            bytes
                .checked_add(13 * (ALIGNMENT - 1))
                .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        );
    }
    Ok(Estimates {
        decoded,
        maximum_batch,
    })
}

fn enforce_footer_bound(row_count: usize) -> Result<(), MultiscaleColumnarError> {
    const FOOTER_BASE_BYTES: usize = 64 * 1024;
    let footer = FOOTER_BASE_BYTES
        .checked_add(
            row_count
                .div_ceil(RECORD_BATCH_ROWS)
                .checked_mul(size_of::<arrow_ipc::Block>())
                .ok_or(MultiscaleColumnarError::SizeOverflow)?,
        )
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    if footer > MAXIMUM_FOOTER_BYTES {
        return Err(MultiscaleColumnarError::ArrowWriter);
    }
    Ok(())
}
