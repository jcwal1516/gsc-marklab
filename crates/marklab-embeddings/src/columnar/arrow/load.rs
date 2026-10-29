//! Bounded raw collection followed by the existing canonical preflight and stock comparer.

use std::io::{Read, Seek, SeekFrom};

use marklab_project::{ArtifactRecord, TableColumnType, TableScalarType};

use super::physical::{
    align, decode_footer, decode_record_batch_message, nonnegative_i64, read_footer,
    read_record_batch_block, schema_message_end, validate_footer_features,
    validate_record_batch_features,
};
use super::{
    multiscale,
    profile::{arrow_failure, EOS_BYTES, RECORD_BATCH_ROWS},
};
use crate::columnar::{
    driver::*, multiscale::enforce_retained_budget, read_context::*, ArrowIpcFailure,
    ColumnarError, EmbeddingColumnarBudgets,
};
use crate::*;

struct RawBatch<'a> {
    rows: usize,
    buffers: [&'a [u8]; 14],
}

impl RawBatch<'_> {
    fn text(&self, offsets: usize, data: usize, row: usize) -> Result<&str, ColumnarError> {
        let bytes = self.buffers[offsets];
        if bytes.len()
            != self
                .rows
                .checked_add(1)
                .and_then(|n| n.checked_mul(4))
                .ok_or(ColumnarError::SizeOverflow)?
        {
            return invalid();
        }
        let start = i32::from_le_bytes(
            bytes[row * 4..row * 4 + 4]
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        );
        let end = i32::from_le_bytes(
            bytes[(row + 1) * 4..(row + 2) * 4]
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        );
        let start =
            usize::try_from(start).map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
        let end =
            usize::try_from(end).map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
        let value = self.buffers[data]
            .get(start..end)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
        std::str::from_utf8(value).map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))
    }
    fn number(&self, data: usize, row: usize) -> Result<u64, ColumnarError> {
        if self.buffers[data].len()
            != self
                .rows
                .checked_mul(8)
                .ok_or(ColumnarError::SizeOverflow)?
        {
            return invalid();
        }
        Ok(u64::from_le_bytes(
            self.buffers[data][row * 8..row * 8 + 8]
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        ))
    }
    fn optional_number(
        &self,
        validity: usize,
        data: usize,
        row: usize,
    ) -> Result<Option<u64>, ColumnarError> {
        if self.buffers[validity].len() != self.rows.div_ceil(8) {
            return invalid();
        }
        let value = self.number(data, row)?;
        Ok((self.buffers[validity][row / 8] & (1 << (row % 8)) != 0).then_some(value))
    }
}

fn invalid<T>() -> Result<T, ColumnarError> {
    Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))
}

fn row_count(record: &ArtifactRecord) -> Result<usize, ColumnarError> {
    let count = usize::try_from(
        record
            .table()
            .ok_or(ColumnarError::ArtifactBindingMismatch)?
            .row_count(),
    )
    .map_err(|_| ColumnarError::SizeOverflow)?;
    if count > 400_000_000 {
        return invalid();
    }
    Ok(count)
}

fn same_count(record: &ArtifactRecord, count: usize) -> Result<(), ColumnarError> {
    if row_count(record)? != count {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn storage<T>(count: usize, text: usize) -> Result<usize, ColumnarError> {
    count
        .checked_mul(size_of::<T>())
        .and_then(|bytes| bytes.checked_add(text))
        .ok_or(ColumnarError::SizeOverflow)
}

fn text_bytes<'a>(mut ids: impl Iterator<Item = &'a str>) -> Result<usize, ColumnarError> {
    ids.try_fold(0_usize, |total, id| {
        total
            .checked_add(id.len())
            .ok_or(ColumnarError::SizeOverflow)
    })
}

fn collect<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    buffers: usize,
    held: usize,
    budgets: EmbeddingColumnarBudgets,
    mut consume: impl FnMut(usize, RawBatch<'_>) -> Result<(), ColumnarError>,
) -> Result<(), ColumnarError> {
    with_retained_bytes(held, budgets, |remaining| {
        let encoded = source
            .seek(SeekFrom::End(0))
            .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
        crate::columnar::multiscale::enforce_file_budget(encoded, remaining)?;
        if encoded != record.content().byte_len() {
            return Err(ColumnarError::ArtifactBindingMismatch);
        }
        let length = usize::try_from(encoded).map_err(|_| ColumnarError::SizeOverflow)?;
        let count = row_count(record)?;
        let (footer_start, footer_len, bytes) = read_footer(source, length, remaining)?;
        let footer = decode_footer(&bytes)?;
        validate_footer_features(footer)?;
        let blocks = footer
            .recordBatches()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
        if blocks.len() != count.div_ceil(RECORD_BATCH_ROWS) {
            return invalid();
        }
        let mut next = schema_message_end(source, length, footer_start)?;
        let mut decoded = 0_u64;
        for (index, block) in blocks.iter().enumerate() {
            let physical = read_record_batch_block(
                source,
                length,
                footer_start,
                footer_len,
                next,
                block.offset(),
                block.metaDataLength(),
                block.bodyLength(),
                remaining,
            )?;
            let (message, body) = physical.message_and_body();
            let batch = decode_record_batch_message(message, body, block.bodyLength())?;
            validate_record_batch_features(batch)?;
            let start = index
                .checked_mul(RECORD_BATCH_ROWS)
                .ok_or(ColumnarError::SizeOverflow)?;
            let rows = count.saturating_sub(start).min(RECORD_BATCH_ROWS);
            if usize::try_from(batch.length()).ok() != Some(rows) {
                return invalid();
            }
            let physical_buffers = batch
                .buffers()
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
            if physical_buffers.len() != buffers || buffers > 14 {
                return invalid();
            }
            let mut raw = RawBatch {
                rows,
                buffers: [&[]; 14],
            };
            let mut offset = 0;
            for (index, buffer) in physical_buffers.iter().enumerate() {
                let begin = nonnegative_i64(buffer.offset(), ArrowIpcFailure::InvalidBuffers)?;
                let size = nonnegative_i64(buffer.length(), ArrowIpcFailure::InvalidBuffers)?;
                let end = begin.checked_add(size).ok_or(ColumnarError::SizeOverflow)?;
                if begin != align(offset)?
                    || body
                        .get(offset..begin)
                        .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
                {
                    return invalid();
                }
                raw.buffers[index] = body
                    .get(begin..end)
                    .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
                offset = end;
                decoded = decoded
                    .checked_add(u64::try_from(size).map_err(|_| ColumnarError::SizeOverflow)?)
                    .ok_or(ColumnarError::SizeOverflow)?;
            }
            if align(offset)? != body.len() || body[offset..].iter().any(|byte| *byte != 0) {
                return invalid();
            }
            crate::columnar::multiscale::enforce_decoded_budget(decoded, remaining)?;
            consume(start, raw)?;
            next = physical.end;
        }
        if next != footer_start.saturating_sub(EOS_BYTES) {
            return invalid();
        }
        Ok(())
    })
}

pub(crate) fn read_cell_embedding_row_link_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellEmbeddingRowLinkReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLink, ColumnarError> {
    let count = context.expected.cells().len();
    same_count(record, count)?;
    if context.statuses.len() != count {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let held = storage::<CellEmbeddingRowLinkEntry>(
        count,
        text_bytes(context.expected.cells().iter().map(|id| id.as_str()))?,
    )?;
    enforce_retained_budget(held, budgets)?;
    let mut rows = try_capacity(count)?;
    collect(source, record, 7, held, budgets, |start, batch| {
        for local in 0..batch.rows {
            let id = &context.expected.cells()[start + local];
            if batch.text(1, 2, local)? != id.as_str() {
                return invalid();
            }
            rows.push(
                CellEmbeddingRowLinkEntry::new(
                    id.clone(),
                    context.statuses[start + local],
                    batch.number(4, local)?,
                    batch.optional_number(5, 6, local)?,
                )
                .map_err(map_embedding_error)?,
            );
        }
        Ok(())
    })?;
    let table = context.rebuild(rows, budgets)?;
    with_retained_bytes(held, budgets, |remaining| {
        super::row_link::reader::validate_cell_embedding_row_link_arrow_reader(
            source,
            record,
            context.expected,
            &table,
            remaining,
        )
    })?;
    Ok(table)
}

pub(crate) fn read_patch_footprint_set_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &PatchFootprintReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintSet, ColumnarError> {
    let count = context.expected.ids().len();
    same_count(record, count)?;
    let held = storage::<PatchFootprint>(
        count,
        text_bytes(context.expected.ids().iter().map(|id| id.as_str()))?,
    )?;
    enforce_retained_budget(held, budgets)?;
    let mut rows = try_capacity(count)?;
    collect(source, record, 7, held, budgets, |start, batch| {
        for local in 0..batch.rows {
            let id = &context.expected.ids()[start + local];
            if batch.text(1, 2, local)? != id.as_str() {
                return invalid();
            }
            rows.push(PatchFootprint::new(
                id.clone(),
                [
                    batch.number(4, local)? as i64,
                    batch.number(6, local)? as i64,
                ],
            ));
        }
        Ok(())
    })?;
    let table = context.rebuild(rows, budgets)?;
    with_retained_bytes(held, budgets, |remaining| {
        multiscale::reader::validate_patch_footprint_set_arrow_reader(
            source,
            record,
            context.expected,
            context.context,
            &table,
            remaining,
        )
    })?;
    Ok(table)
}

pub(crate) fn read_patch_overlap_graph_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &PatchOverlapReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapGraph, ColumnarError> {
    let table = context.rebuild(budgets)?;
    let held = overlap_retained_bytes(context.expected, &table)?;
    with_retained_bytes(held, budgets, |remaining| {
        multiscale::reader::validate_patch_overlap_graph_arrow_reader(
            source,
            record,
            context.expected,
            context.context,
            context.footprints,
            &table,
            remaining,
        )
    })?;
    Ok(table)
}

fn embedding_status(value: &str) -> Result<EmbeddingStatus, ColumnarError> {
    match value {
        "present" => Ok(EmbeddingStatus::Present),
        "missing_vector" => Ok(EmbeddingStatus::MissingVector),
        "extraction_failed" => Ok(EmbeddingStatus::ExtractionFailed),
        "qc_rejected" => Ok(EmbeddingStatus::QcRejected),
        _ => invalid(),
    }
}

macro_rules! matrix_read {
    ($name:ident, $context:ty, $table:ty, $profile:ident) => {
        pub(crate) fn $name<R: Read + Seek + ?Sized>(
            source: &mut R,
            record: &ArtifactRecord,
            context: &$context,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<$table, ColumnarError> {
            let count = context.expected.ids().len();
            same_count(record, count)?;
            let manifest = record
                .table()
                .ok_or(ColumnarError::ArtifactBindingMismatch)?;
            let dimension = match manifest.columns().get(1).map(|field| field.column_type()) {
                Some(TableColumnType::FixedSizeList {
                    element: TableScalarType::F32,
                    length,
                }) if *length > 0 && *length <= 65_536 => *length,
                _ => return Err(ColumnarError::ArtifactBindingMismatch),
            };
            let profile = crate::multiscale::physical::MatrixPhysicalProfile::$profile;
            if context.bindings.expected_entities().logical_digest()
                != context.expected.logical_digest()
                || !profile.record_matches_encoding(
                    record,
                    crate::multiscale::physical::SpatialPhysicalEncoding::Arrow,
                    manifest.row_count(),
                    dimension,
                )
            {
                return Err(ColumnarError::ArtifactBindingMismatch);
            }
            let held = <$table>::predicted_retained_bytes(dimension, context.expected)
                .map_err(map_multiscale_error)?;
            enforce_retained_budget(held, budgets)?;
            let components = count
                .checked_mul(dimension as usize)
                .ok_or(ColumnarError::SizeOverflow)?;
            let mut values = try_capacity(components)?;
            let mut statuses = try_capacity(count)?;
            collect(source, record, 9, held, budgets, |start, batch| {
                let expected_bytes = batch
                    .rows
                    .checked_mul(dimension as usize)
                    .and_then(|components| components.checked_mul(4))
                    .ok_or(ColumnarError::SizeOverflow)?;
                if batch.buffers[5].len() != expected_bytes {
                    return invalid();
                }
                for local in 0..batch.rows {
                    if batch.text(1, 2, local)? != context.expected.ids()[start + local].as_str() {
                        return invalid();
                    }
                    let status = embedding_status(batch.text(7, 8, local)?)?;
                    statuses.push(status);
                    let offset = local * dimension as usize * 4;
                    for chunk in
                        batch.buffers[5][offset..offset + dimension as usize * 4].chunks_exact(4)
                    {
                        let value = f32::from_le_bytes(
                            chunk
                                .try_into()
                                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
                        );
                        if !value.is_finite()
                            || (value == 0.0 && value.to_bits() != 0)
                            || (status != EmbeddingStatus::Present && value.to_bits() != 0)
                        {
                            return invalid();
                        }
                        values.push(value);
                    }
                }
                Ok(())
            })?;
            let bindings = context.bindings;
            let table = <$table>::from_physical_values(
                dimension,
                context.expected,
                bindings.expected_entities().artifact_id(),
                bindings.support().artifact_id(),
                bindings.support().logical_digest(),
                bindings.provenance().artifact_id(),
                bindings.provenance().logical_digest(),
                values,
                statuses,
                budgets.maximum_retained_bytes(),
            )
            .map_err(map_multiscale_error)?;
            with_retained_bytes(held, budgets, |remaining| {
                multiscale::matrix::reader::validate_reader(source, record, &table, remaining)
            })?;
            Ok(table)
        }
    };
}
matrix_read!(
    read_patch_embedding_table_arrow_reader,
    PatchEmbeddingTableReadContext<'_>,
    PatchEmbeddingTable,
    Patch
);
matrix_read!(
    read_region_embedding_table_arrow_reader,
    RegionEmbeddingTableReadContext<'_>,
    RegionEmbeddingTable,
    Region
);
matrix_read!(
    read_slide_embedding_table_arrow_reader,
    SlideEmbeddingTableReadContext<'_>,
    SlideEmbeddingTable,
    Slide
);

pub(crate) fn read_cell_patch_assignment_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellPatchAssignmentReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchLink, ColumnarError> {
    let count = context.common.expected_cells.cells().len();
    same_count(record, count)?;
    let held = storage::<CellPatchAssignmentReadRow>(
        count,
        text_bytes(
            context
                .common
                .expected_cells
                .cells()
                .iter()
                .map(|id| id.as_str()),
        )?,
    )?;
    enforce_retained_budget(held, budgets)?;
    let mut rows = try_capacity(count)?;
    collect(source, record, 14, held, budgets, |start, batch| {
        for local in 0..batch.rows {
            let id = &context.common.expected_cells.cells()[start + local];
            if batch.text(1, 2, local)? != id.as_str() {
                return invalid();
            }
            let status = match batch.text(4, 5, local)? {
                "assigned" => CellPatchAssignmentStatus::Assigned,
                "outside_sampled_support" => CellPatchAssignmentStatus::OutsideSampledSupport,
                "interpolation_unavailable" => CellPatchAssignmentStatus::InterpolationUnavailable,
                _ => return invalid(),
            };
            let anchor = [
                f64::from_bits(batch.number(7, local)?),
                f64::from_bits(batch.number(9, local)?),
            ];
            if anchor
                .iter()
                .any(|value| !value.is_finite() || (*value == 0.0 && value.to_bits() != 0))
            {
                return invalid();
            }
            rows.push(CellPatchAssignmentReadRow {
                anchor: CellPatchAnchor::new(id.clone(), anchor).map_err(map_multiscale_error)?,
                status,
                edge_start: batch.number(11, local)?,
                edge_count: batch.number(13, local)?,
            });
        }
        Ok(())
    })?;
    let link = context.rebuild(rows, budgets)?;
    let held = cell_patch_retained_bytes(&link)?;
    with_retained_bytes(held, budgets, |remaining| {
        multiscale::cell_patch::reader::validate_managed(
            source,
            record,
            multiscale::cell_patch::profile::CellPatchArrowProfile::Assignment,
            &link,
            remaining,
        )
    })?;
    Ok(link)
}

pub(crate) fn read_cell_patch_edge_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellPatchEdgeReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchLink, ColumnarError> {
    let count = row_count(record)?;
    let expected_count = context.assignments.iter().try_fold(0_u64, |total, row| {
        total
            .checked_add(row.edge_count())
            .ok_or(ColumnarError::SizeOverflow)
    })?;
    if u64::try_from(count).ok() != Some(expected_count) {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let max_text = context
        .common
        .expected_patches
        .ids()
        .iter()
        .map(|id| id.as_str().len())
        .max()
        .unwrap_or(0);
    let held = storage::<CellPatchEdgeReadRow>(
        count,
        count
            .checked_mul(max_text)
            .ok_or(ColumnarError::SizeOverflow)?,
    )?;
    enforce_retained_budget(held, budgets)?;
    let mut rows = try_capacity(count)?;
    collect(source, record, 9, held, budgets, |_, batch| {
        for local in 0..batch.rows {
            let name = batch.text(3, 4, local)?;
            let index = context
                .common
                .expected_patches
                .ids()
                .binary_search_by(|id| id.as_str().cmp(name))
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?;
            rows.push(CellPatchEdgeReadRow {
                assignment_row: batch.number(1, local)?,
                patch_id: context.common.expected_patches.ids()[index].clone(),
                numerator: batch.optional_number(5, 6, local)?,
                denominator: batch.optional_number(7, 8, local)?,
            });
        }
        Ok(())
    })?;
    let link = context.rebuild(rows, budgets)?;
    with_retained_bytes(cell_patch_retained_bytes(&link)?, budgets, |remaining| {
        multiscale::cell_patch::reader::validate_managed(
            source,
            record,
            multiscale::cell_patch::profile::CellPatchArrowProfile::Edge,
            &link,
            remaining,
        )
    })?;
    Ok(link)
}

pub(crate) fn read_patch_region_link_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &PatchRegionReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionLink, ColumnarError> {
    let count = row_count(record)?;
    let patches = context.expected_patches.ids();
    let regions = context.expected_regions.ids();
    let assessed = patches
        .len()
        .checked_mul(regions.len())
        .ok_or(ColumnarError::SizeOverflow)?;
    if count > assessed {
        return invalid();
    }
    let maximum_text = patches
        .iter()
        .map(|id| id.as_str().len())
        .max()
        .unwrap_or(0)
        .checked_add(
            regions
                .iter()
                .map(|id| id.as_str().len())
                .max()
                .unwrap_or(0),
        )
        .ok_or(ColumnarError::SizeOverflow)?;
    let held = storage::<PatchRegionDeclaration>(
        count,
        count
            .checked_mul(maximum_text)
            .ok_or(ColumnarError::SizeOverflow)?,
    )?;
    enforce_retained_budget(held, budgets)?;
    let mut rows = try_capacity(count)?;
    collect(source, record, 13, held, budgets, |_, batch| {
        for local in 0..batch.rows {
            let patch = batch.text(1, 2, local)?;
            let region = batch.text(4, 5, local)?;
            let patch = patches
                .binary_search_by(|id| id.as_str().cmp(patch))
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?;
            let region = regions
                .binary_search_by(|id| id.as_str().cmp(region))
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?;
            let numerator = batch.number(10, local)?;
            let denominator = batch.number(12, local)?;
            let declaration = match batch.text(7, 8, local)? {
                "fully_contained" if numerator == 1 && denominator == 1 => {
                    PatchRegionDeclaration::fully_contained(
                        patches[patch].clone(),
                        regions[region].clone(),
                    )
                }
                "partial_overlap" => PatchRegionDeclaration::partial_overlap(
                    patches[patch].clone(),
                    regions[region].clone(),
                    numerator,
                    denominator,
                )
                .map_err(map_multiscale_error)?,
                _ => return invalid(),
            };
            rows.push(declaration);
        }
        Ok(())
    })?;
    let link = context.rebuild(rows, budgets)?;
    let held = patch_region_retained_bytes(&link)?;
    with_retained_bytes(held, budgets, |remaining| {
        multiscale::patch_region::reader::validate_patch_region_link_arrow_reader(
            source, record, &link, remaining,
        )
    })?;
    Ok(link)
}

pub(crate) fn read_cell_embedding_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellEmbeddingTableReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, ColumnarError> {
    super::reader::read_cell_embedding_table_arrow_reader(
        source,
        record,
        context.expected,
        context.row_link,
        context.graph,
        budgets,
    )
}

pub(crate) fn verify_cell_embedding_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellEmbeddingTableReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingTableArtifact, ColumnarError> {
    let summary = super::reader::scan_cell_embedding_table_arrow_reader(
        source,
        record,
        context.expected,
        context.row_link,
        context.graph,
        budgets,
    )?;
    Ok(VerifiedCellEmbeddingTableArtifact::new(
        record.id(),
        context.graph,
        summary,
    ))
}

pub(crate) fn verify_cell_embedding_row_link_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellEmbeddingRowLinkVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingRowLinkArtifact, ColumnarError> {
    super::row_link::reader::validate_cell_embedding_row_link_arrow_reader(
        source,
        record,
        context.expected,
        context.row_link,
        budgets,
    )?;
    Ok(VerifiedCellEmbeddingRowLinkArtifact::new(
        record.id(),
        context.row_link,
    ))
}

pub(crate) fn verify_patch_footprint_set_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &PatchFootprintVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchFootprintArtifact, ColumnarError> {
    let values = &context.values;
    multiscale::reader::validate_patch_footprint_set_arrow_reader(
        source,
        record,
        values.expected,
        values.context,
        values.footprints,
        budgets,
    )?;
    VerifiedPatchFootprintArtifact::new(
        record.id(),
        values.context,
        values.footprints,
        context.graph,
    )
}

pub(crate) fn verify_patch_overlap_graph_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &PatchOverlapVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchOverlapArtifact, ColumnarError> {
    let values = &context.values;
    multiscale::reader::validate_patch_overlap_graph_arrow_reader(
        source,
        record,
        values.expected,
        values.context,
        values.footprints,
        values.overlap,
        budgets,
    )?;
    VerifiedPatchOverlapArtifact::new(
        record.id(),
        values.footprints,
        values.overlap,
        context.verified_footprints,
        context.graph,
    )
}

pub(crate) fn verify_patch_embedding_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &PatchEmbeddingTableVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchEmbeddingTableArtifact, ColumnarError> {
    multiscale::matrix::reader::validate_reader(source, record, context.table, budgets)?;
    VerifiedPatchEmbeddingTableArtifact::new(
        record.id(),
        context.table,
        context.source_row_link,
        context.support,
        context.graph,
    )
}

pub(crate) fn verify_region_embedding_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    candidate: &DerivedRegionEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedRegionEmbeddingTableArtifact, ColumnarError> {
    multiscale::matrix::reader::validate_reader(source, record, candidate.table(), budgets)?;
    VerifiedRegionEmbeddingTableArtifact::new(record.id(), candidate)
}

pub(crate) fn verify_slide_embedding_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    candidate: &DerivedSlideEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedSlideEmbeddingTableArtifact, ColumnarError> {
    multiscale::matrix::reader::validate_reader(source, record, candidate.table(), budgets)?;
    VerifiedSlideEmbeddingTableArtifact::new(record.id(), candidate)
}

pub(crate) fn verify_cell_patch_assignment_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellPatchVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, ColumnarError> {
    multiscale::cell_patch::reader::validate_managed(
        source,
        record,
        multiscale::cell_patch::profile::CellPatchArrowProfile::Assignment,
        context.link,
        budgets,
    )?;
    VerifiedCellPatchAssignmentArtifact::new(record.id(), context.link, context.graph)
}

pub(crate) fn verify_cell_patch_edge_table_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &CellPatchVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, ColumnarError> {
    multiscale::cell_patch::reader::validate_managed(
        source,
        record,
        multiscale::cell_patch::profile::CellPatchArrowProfile::Edge,
        context.link,
        budgets,
    )?;
    VerifiedCellPatchEdgeArtifact::new(record.id(), context.link, context.graph)
}

pub(crate) fn verify_patch_region_link_arrow_reader<R: Read + Seek + ?Sized>(
    source: &mut R,
    record: &ArtifactRecord,
    context: &PatchRegionVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchRegionLinkArtifact, ColumnarError> {
    multiscale::patch_region::reader::validate_patch_region_link_arrow_reader(
        source,
        record,
        context.link,
        budgets,
    )?;
    VerifiedPatchRegionLinkArtifact::new(record.id(), context.link, context.graph)
}
