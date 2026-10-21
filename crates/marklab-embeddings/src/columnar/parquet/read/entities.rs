use super::*;

use crate::columnar::multiscale::{cell_patch_metadata, matrix_metadata, patch_region_metadata};
use crate::columnar::read_context::{
    map_embedding_error, map_multiscale_error, overlap_retained_bytes, try_capacity,
    with_retained_bytes, CellPatchAssignmentReadRow, CellPatchEdgeReadRow,
};
use crate::multiscale::physical::SpatialArtifactRole;
use crate::{
    CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellEmbeddingRowLinkReadContext,
    CellPatchAnchor, CellPatchAssignmentReadContext, CellPatchEdgeReadContext, CellPatchLink,
    PatchEmbeddingTable, PatchEmbeddingTableReadContext, PatchFootprint, PatchFootprintReadContext,
    PatchFootprintSet, PatchOverlapGraph, PatchOverlapReadContext, PatchRegionDeclaration,
    PatchRegionLink, PatchRegionReadContext, PatchRegionRelation, RegionEmbeddingTable,
    RegionEmbeddingTableReadContext, SlideEmbeddingTable, SlideEmbeddingTableReadContext,
};

pub(crate) fn read_cell_embedding_row_link_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &CellEmbeddingRowLinkReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLink, ColumnarError> {
    let count = context.expected.cells().len();
    if context.statuses.len() != count {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let (prepared, rows) = collect_rows(
        reader,
        record,
        ReadProfile::RowLink(context.statuses),
        count,
        budgets,
        |batch, index, output| {
            row_link::reader::visit_batch(batch, index, |index, id, source, vector| {
                let expected = context
                    .expected
                    .cells()
                    .get(index)
                    .ok_or(ColumnarError::ArtifactBindingMismatch)?;
                if id != expected.as_str() {
                    return Err(failure(ParquetFailure::InvalidCellOrder));
                }
                output.push(
                    CellEmbeddingRowLinkEntry::new(
                        expected.clone(),
                        context.statuses[index],
                        source,
                        vector,
                    )
                    .map_err(map_embedding_error)?,
                );
                Ok(())
            })
        },
    )?;
    let link = with_retained_bytes(prepared.metadata.memory_size(), budgets, |remaining| {
        context.rebuild(rows, remaining)
    })?;
    row_link::reader::validate_record(record, record.content().byte_len(), &link)?;
    match_metadata(
        &prepared.metadata,
        row_link::profile::METADATA_KEYS
            .into_iter()
            .zip(row_link::profile::metadata_values(&link))
            .map(|(key, value)| (key.to_owned(), value)),
    )?;
    Ok(link)
}

pub(crate) fn read_patch_footprint_set_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &PatchFootprintReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintSet, ColumnarError> {
    let (prepared, rows) = collect_rows(
        reader,
        record,
        ReadProfile::Footprint,
        context.expected.ids().len(),
        budgets,
        |batch, index, output| {
            multiscale::reader::visit_footprint_batch(batch, index, |index, id, origin| {
                let expected = context
                    .expected
                    .ids()
                    .get(index)
                    .ok_or(ColumnarError::ArtifactBindingMismatch)?;
                if id != expected.as_str() {
                    return Err(failure(ParquetFailure::InvalidCanonicalRows));
                }
                output.push(PatchFootprint::new(expected.clone(), origin));
                Ok(())
            })
        },
    )?;
    let footprints = with_retained_bytes(prepared.metadata.memory_size(), budgets, |remaining| {
        context.rebuild(rows, remaining)
    })?;
    multiscale::reader::validate_record(
        record,
        record.content().byte_len(),
        SpatialArtifactRole::Footprint,
        footprints.row_count(),
        multiscale::reader::footprint_dependencies(&footprints),
    )?;
    match_metadata(
        &prepared.metadata,
        multiscale::profile::metadata_entries(
            SpatialParquetProfile::Footprint,
            context.expected,
            &footprints,
            None,
        ),
    )?;
    Ok(footprints)
}

pub(crate) fn read_patch_overlap_graph_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &PatchOverlapReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapGraph, ColumnarError> {
    let profile = ReadProfile::Overlap;
    let count = manifest_rows(record)?;
    let prepared = prepare(
        reader,
        record,
        profile,
        count,
        size_of::<crate::PatchOverlapEdge>(),
        budgets,
    )?;
    let overlap = with_retained_bytes(prepared.metadata.memory_size(), budgets, |remaining| {
        context.rebuild(remaining)
    })?;
    if overlap.edge_count() != count {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    multiscale::reader::validate_record(
        record,
        record.content().byte_len(),
        SpatialArtifactRole::Overlap,
        count,
        multiscale::reader::overlap_dependencies(context.footprints, &overlap),
    )?;
    match_metadata(
        &prepared.metadata,
        multiscale::profile::metadata_entries(
            SpatialParquetProfile::Overlap,
            context.expected,
            context.footprints,
            Some(&overlap),
        ),
    )?;
    visit_groups(
        reader,
        &prepared.metadata,
        profile.schema()?,
        count,
        prepared.retained_preflight_bytes,
        overlap_retained_bytes(context.expected, &overlap)?,
        budgets,
        |batch, index| {
            multiscale::reader::visit_overlap_batch(batch, index, |index, left, right| {
                let expected = overlap
                    .edges()
                    .get(index)
                    .ok_or(ColumnarError::ArtifactBindingMismatch)?;
                if left != expected.left_patch_id().as_str()
                    || right != expected.right_patch_id().as_str()
                {
                    return Err(failure(ParquetFailure::InvalidCanonicalRows));
                }
                Ok(())
            })
        },
    )?;
    Ok(overlap)
}

pub(crate) fn read_cell_patch_assignment_table_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &CellPatchAssignmentReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchLink, ColumnarError> {
    let count = context.common.expected_cells.cells().len();
    let (prepared, rows) = collect_rows(
        reader,
        record,
        ReadProfile::Assignment(context.common.mode),
        count,
        budgets,
        |batch, index, output| {
            cell_patch::reader::visit_assignment_batch(
                batch,
                index,
                |index, id, status, anchor, start, count| {
                    let expected = context
                        .common
                        .expected_cells
                        .cells()
                        .get(index)
                        .ok_or(ColumnarError::ArtifactBindingMismatch)?;
                    if id != expected.as_str() {
                        return Err(failure(ParquetFailure::InvalidCanonicalRows));
                    }
                    output.push(CellPatchAssignmentReadRow {
                        anchor: CellPatchAnchor::new(expected.clone(), anchor)
                            .map_err(map_multiscale_error)?,
                        status,
                        edge_start: start,
                        edge_count: count,
                    });
                    Ok(())
                },
            )
        },
    )?;
    let link = with_retained_bytes(prepared.metadata.memory_size(), budgets, |remaining| {
        context.rebuild(rows, remaining)
    })?;
    cell_patch::reader::validate_record(
        record,
        record.content().byte_len(),
        cell_patch::profile::CellPatchParquetProfile::Assignment,
        &link,
    )?;
    match_metadata(
        &prepared.metadata,
        cell_patch_metadata(
            SpatialArtifactRole::CellPatchAssignment,
            SpatialPhysicalEncoding::Parquet,
            &link,
        ),
    )?;
    Ok(link)
}

pub(crate) fn read_cell_patch_edge_table_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &CellPatchEdgeReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchLink, ColumnarError> {
    let count = manifest_rows(record)?;
    let (prepared, rows) = collect_rows(
        reader,
        record,
        ReadProfile::Edge(context.common.mode),
        count,
        budgets,
        |batch, index, output| {
            cell_patch::reader::visit_edge_batch(
                batch,
                context.common.mode,
                index,
                |_, assignment_row, id, numerator, denominator| {
                    let ids = context.common.expected_patches.ids();
                    let index = ids
                        .binary_search_by(|expected| expected.as_str().cmp(id))
                        .map_err(|_| ColumnarError::ArtifactBindingMismatch)?;
                    output.push(CellPatchEdgeReadRow {
                        assignment_row,
                        patch_id: ids[index].clone(),
                        numerator,
                        denominator,
                    });
                    Ok(())
                },
            )
        },
    )?;
    let link = with_retained_bytes(prepared.metadata.memory_size(), budgets, |remaining| {
        context.rebuild(rows, remaining)
    })?;
    cell_patch::reader::validate_record(
        record,
        record.content().byte_len(),
        cell_patch::profile::CellPatchParquetProfile::Edge,
        &link,
    )?;
    match_metadata(
        &prepared.metadata,
        cell_patch_metadata(
            SpatialArtifactRole::CellPatchEdge,
            SpatialPhysicalEncoding::Parquet,
            &link,
        ),
    )?;
    Ok(link)
}

pub(crate) fn read_patch_region_link_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &PatchRegionReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionLink, ColumnarError> {
    let count = manifest_rows(record)?;
    let (prepared, rows) = collect_rows(
        reader,
        record,
        ReadProfile::PatchRegion,
        count,
        budgets,
        |batch, index, output| {
            patch_region::reader::visit_batch(
                batch,
                index,
                |_, patch, region, relation, numerator, denominator| {
                    let patches = context.expected_patches.ids();
                    let regions = context.expected_regions.ids();
                    let patch_index = patches
                        .binary_search_by(|id| id.as_str().cmp(patch))
                        .map_err(|_| ColumnarError::ArtifactBindingMismatch)?;
                    let region_index = regions
                        .binary_search_by(|id| id.as_str().cmp(region))
                        .map_err(|_| ColumnarError::ArtifactBindingMismatch)?;
                    let patch = patches[patch_index].clone();
                    let region = regions[region_index].clone();
                    output.push(match relation {
                        PatchRegionRelation::FullyContained => {
                            PatchRegionDeclaration::fully_contained(patch, region)
                        }
                        PatchRegionRelation::PartialOverlap => {
                            PatchRegionDeclaration::partial_overlap(
                                patch,
                                region,
                                numerator,
                                denominator,
                            )
                            .map_err(map_multiscale_error)?
                        }
                    });
                    Ok(())
                },
            )
        },
    )?;
    let link = with_retained_bytes(prepared.metadata.memory_size(), budgets, |remaining| {
        context.rebuild(rows, remaining)
    })?;
    patch_region::reader::validate_record(record, record.content().byte_len(), &link)?;
    match_metadata(
        &prepared.metadata,
        patch_region_metadata(SpatialPhysicalEncoding::Parquet, &link),
    )?;
    Ok(link)
}

macro_rules! matrix_reader {
    ($read:ident, $context:ty, $table:ty, $profile:expr) => {
        pub(crate) fn $read<R: Read + Seek + ?Sized>(
            reader: &mut R,
            record: &ArtifactRecord,
            context: &$context,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<$table, ColumnarError> {
            let expected = context.expected;
            let bindings = context.bindings;
            if expected.logical_digest() != bindings.expected_entities().logical_digest() {
                return Err(ColumnarError::ArtifactBindingMismatch);
            }
            let dimension = matrix_dimension(record)?;
            let profile = ReadProfile::Matrix($profile, dimension);
            let count = expected.ids().len();
            let row_bytes = (dimension as usize)
                .checked_mul(size_of::<f32>())
                .and_then(|n| n.checked_add(size_of::<EmbeddingStatus>()))
                .ok_or(ColumnarError::SizeOverflow)?;
            let mut prepared = prepare(reader, record, profile, count, row_bytes, budgets)?;
            prepared.collection_bytes = <$table>::predicted_retained_bytes(dimension, expected)
                .map_err(map_multiscale_error)?;
            collection_budget(&prepared, budgets)?;
            let mut values = try_capacity(
                count
                    .checked_mul(dimension as usize)
                    .ok_or(ColumnarError::SizeOverflow)?,
            )?;
            let mut statuses = try_capacity(count)?;
            visit_groups(
                reader,
                &prepared.metadata,
                profile.schema()?,
                count,
                prepared.retained_preflight_bytes,
                prepared.collection_bytes,
                budgets,
                |batch, index| {
                    matrix::reader::visit_batch(
                        batch,
                        dimension,
                        index,
                        |index, id, status, vector| {
                            if expected
                                .ids()
                                .get(index)
                                .is_none_or(|expected| expected.as_str() != id)
                            {
                                return Err(failure(ParquetFailure::InvalidCanonicalRows));
                            }
                            statuses.push(status);
                            values.extend_from_slice(vector);
                            Ok(())
                        },
                    )
                },
            )?;
            let table =
                with_retained_bytes(prepared.metadata.memory_size(), budgets, |remaining| {
                    <$table>::from_physical_values(
                        dimension,
                        expected,
                        bindings.expected_entities().artifact_id(),
                        bindings.support().artifact_id(),
                        bindings.support().logical_digest(),
                        bindings.provenance().artifact_id(),
                        bindings.provenance().logical_digest(),
                        values,
                        statuses,
                        remaining.maximum_retained_bytes(),
                    )
                    .map_err(map_multiscale_error)
                })?;
            matrix::reader::validate_record(record, record.content().byte_len(), &table)?;
            match_metadata(
                &prepared.metadata,
                matrix_metadata(SpatialPhysicalEncoding::Parquet, &table)?,
            )?;
            Ok(table)
        }
    };
}

matrix_reader!(
    read_patch_embedding_table_parquet_reader,
    PatchEmbeddingTableReadContext<'_>,
    PatchEmbeddingTable,
    MatrixPhysicalProfile::Patch
);
matrix_reader!(
    read_region_embedding_table_parquet_reader,
    RegionEmbeddingTableReadContext<'_>,
    RegionEmbeddingTable,
    MatrixPhysicalProfile::Region
);
matrix_reader!(
    read_slide_embedding_table_parquet_reader,
    SlideEmbeddingTableReadContext<'_>,
    SlideEmbeddingTable,
    MatrixPhysicalProfile::Slide
);
