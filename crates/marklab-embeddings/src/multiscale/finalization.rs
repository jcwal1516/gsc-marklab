use std::mem::size_of;

use marklab_data::PatchId;

use super::{
    error::MultiscaleEmbeddingError, ExpectedRegionSet, PatchEmbeddingTable,
    PatchRegionDeclaration, PatchRegionLink, RegionEmbeddingRow, RegionEmbeddingTable,
    VerifiedDerivedRegionEmbeddingArtifactGraph,
};
use crate::{digest::canonical_positive_zero, EmbeddingStatus};

/// Explicit memory and deterministic-work limits for derived embedding finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingFinalizationBudgets {
    maximum_retained_bytes: usize,
    maximum_working_bytes: usize,
    maximum_contributor_rows: u64,
    maximum_component_accumulations: u64,
}

impl EmbeddingFinalizationBudgets {
    /// Declare retained, peak-working, contributor-row, and component-operation maxima.
    pub fn new(
        maximum_retained_bytes: usize,
        maximum_working_bytes: usize,
        maximum_contributor_rows: u64,
        maximum_component_accumulations: u64,
    ) -> Self {
        Self {
            maximum_retained_bytes,
            maximum_working_bytes,
            maximum_contributor_rows,
            maximum_component_accumulations,
        }
    }

    /// Maximum retained bytes for the completed candidate.
    pub fn maximum_retained_bytes(self) -> usize {
        self.maximum_retained_bytes
    }

    /// Maximum peak bytes including live borrowed inputs and operation-owned scratch.
    pub fn maximum_working_bytes(self) -> usize {
        self.maximum_working_bytes
    }

    /// Maximum sparse contributor rows visited by one deterministic pass.
    pub fn maximum_contributor_rows(self) -> u64 {
        self.maximum_contributor_rows
    }

    /// Maximum component multiply-and-sequential-add operations.
    pub fn maximum_component_accumulations(self) -> u64 {
        self.maximum_component_accumulations
    }
}

/// Unforgeable result of deterministic region recomputation from a verified derived graph.
///
/// The contained table may be borrowed for canonical publication. Only full physical validation
/// of this candidate can mint a region-table artifact receipt.
pub struct DerivedRegionEmbeddingTableCandidate {
    table: RegionEmbeddingTable,
    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) graph: VerifiedDerivedRegionEmbeddingArtifactGraph,
}

impl std::fmt::Debug for DerivedRegionEmbeddingTableCandidate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DerivedRegionEmbeddingTableCandidate")
            .field("qc_summary", &self.table.qc_summary())
            .field("dimension", &self.table.dimension())
            .finish_non_exhaustive()
    }
}

impl DerivedRegionEmbeddingTableCandidate {
    /// Borrow the exactly recomputed immutable region table for canonical publication.
    pub fn table(&self) -> &RegionEmbeddingTable {
        &self.table
    }

    /// Number of canonical derived region rows.
    pub fn row_count(&self) -> usize {
        self.table.row_count()
    }

    /// Fixed output dimension inherited from the source patch table.
    pub fn dimension(&self) -> u32 {
        self.table.dimension()
    }

    /// Format-independent logical identity of the recomputed region table.
    pub fn logical_digest(&self) -> marklab_project::ContentDigest {
        self.table.logical_digest()
    }

    /// Recomputed factual status, shape, zero-vector, and logical summary.
    pub fn qc_summary(&self) -> super::MultiscaleEmbeddingQcSummary {
        self.table.qc_summary()
    }
}

/// Deterministically recompute every region row from verified patch rows and declared fractions.
///
/// Fractions remain producer declarations; this operation proves no region geometry, tissue mask,
/// observation window, or opaque source-vector correspondence.
///
/// # Errors
///
/// Returns a typed binding, resource, allocation, shape, or table-construction error.
pub fn finalize_region_embedding_table_from_patches(
    expected_regions: &ExpectedRegionSet,
    source_patch_table: &PatchEmbeddingTable,
    link: &PatchRegionLink,
    graph: VerifiedDerivedRegionEmbeddingArtifactGraph,
    budgets: EmbeddingFinalizationBudgets,
) -> Result<DerivedRegionEmbeddingTableCandidate, MultiscaleEmbeddingError> {
    require_exact_bindings(expected_regions, source_patch_table, link, graph)?;

    let relation_count = u64::try_from(link.nonzero_relation_count())
        .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    if relation_count > budgets.maximum_contributor_rows() {
        return Err(
            MultiscaleEmbeddingError::DerivedEmbeddingContributorBudgetExceeded {
                required: relation_count,
                maximum: budgets.maximum_contributor_rows(),
            },
        );
    }
    let present_relations = count_present_relations(source_patch_table, link)?;
    let component_accumulations = present_relations
        .checked_mul(u64::from(source_patch_table.dimension()))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    if component_accumulations > budgets.maximum_component_accumulations() {
        return Err(
            MultiscaleEmbeddingError::DerivedEmbeddingComponentBudgetExceeded {
                required: component_accumulations,
                maximum: budgets.maximum_component_accumulations(),
            },
        );
    }

    let link_retained = link.retained_bytes()?;
    let input_retained = expected_regions
        .retained_bytes()?
        .checked_add(source_patch_table.retained_bytes()?)
        .and_then(|value| value.checked_add(link_retained))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let graph_bytes = size_of::<VerifiedDerivedRegionEmbeddingArtifactGraph>();
    let retained_graph_bytes = if cfg!(feature = "parquet") {
        graph_bytes
    } else {
        0
    };
    let base_working = input_retained
        .checked_add(graph_bytes)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let candidate_retained = RegionEmbeddingTable::predicted_final_retained_bytes(
        expected_regions,
        source_patch_table.dimension(),
    )?
    .checked_add(retained_graph_bytes)
    .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    if candidate_retained > budgets.maximum_retained_bytes() {
        return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required: candidate_retained,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    let region_count = expected_regions.ids().len();
    let dimension = usize::try_from(source_patch_table.dimension())
        .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    let accumulator_stride = dimension
        .checked_add(1)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let accumulator_count = region_count
        .checked_mul(accumulator_stride)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let accumulator_bytes = accumulator_count
        .checked_mul(size_of::<f64>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    require_working(
        base_working
            .checked_add(accumulator_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        budgets,
    )?;
    let mut accumulators = try_vec_with_zeros::<f64>(accumulator_count)?;
    accumulate_regions(
        source_patch_table,
        link,
        expected_regions,
        dimension,
        accumulator_stride,
        &mut accumulators,
    )?;

    let present_region_count = (0..region_count)
        .filter(|index| accumulators[index * accumulator_stride + dimension] > 0.0)
        .count();
    let row_slot_bytes = region_count
        .checked_mul(size_of::<RegionEmbeddingRow>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let output_vector_bytes = present_region_count
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(size_of::<f32>()))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    require_working(
        base_working
            .checked_add(accumulator_bytes)
            .and_then(|value| value.checked_add(row_slot_bytes))
            .and_then(|value| value.checked_add(output_vector_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        budgets,
    )?;
    let rows = materialize_rows(
        expected_regions,
        dimension,
        accumulator_stride,
        &accumulators,
    )?;
    let table_construction_bytes = RegionEmbeddingTable::construction_bytes(
        expected_regions,
        source_patch_table.dimension(),
        &rows,
    )?;
    require_working(
        base_working
            .checked_add(table_construction_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        budgets,
    )?;
    drop(accumulators);
    let table = RegionEmbeddingTable::from_rows(
        source_patch_table.dimension(),
        expected_regions,
        graph.expected_regions_artifact_id,
        graph.region_support_artifact_id,
        graph.region_support_logical_digest,
        graph.provenance_artifact_id,
        graph.provenance_logical_digest,
        rows,
        table_construction_bytes,
    )?;
    Ok(DerivedRegionEmbeddingTableCandidate {
        table,
        #[cfg(feature = "parquet")]
        graph,
    })
}

fn require_exact_bindings(
    expected_regions: &ExpectedRegionSet,
    source_patch_table: &PatchEmbeddingTable,
    link: &PatchRegionLink,
    graph: VerifiedDerivedRegionEmbeddingArtifactGraph,
) -> Result<(), MultiscaleEmbeddingError> {
    let row_count = u64::try_from(source_patch_table.row_count())
        .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    if source_patch_table.owning_slide_id() != expected_regions.owning_slide_id()
        || link.owning_slide_id() != expected_regions.owning_slide_id()
        || source_patch_table.logical_digest() != graph.source_patch_table_logical_digest
        || row_count != graph.source_patch_table_row_count
        || source_patch_table.dimension() != graph.output_dimension
        || link.logical_digest() != graph.patch_region_link_logical_digest
        || link.expected_regions_logical_digest() != expected_regions.logical_digest()
        || expected_regions.logical_digest() != graph.expected_regions_logical_digest
    {
        return Err(MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch);
    }
    Ok(())
}

fn count_present_relations(
    source_patch_table: &PatchEmbeddingTable,
    link: &PatchRegionLink,
) -> Result<u64, MultiscaleEmbeddingError> {
    let mut patch_index = 0_usize;
    let mut present = 0_u64;
    for relation in link.nonzero_relations() {
        let index = find_patch_row(source_patch_table, relation.patch_id(), &mut patch_index)?;
        if source_patch_table.row(index)?.status() == EmbeddingStatus::Present {
            present = present
                .checked_add(1)
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        }
    }
    Ok(present)
}

fn accumulate_regions(
    source_patch_table: &PatchEmbeddingTable,
    link: &PatchRegionLink,
    expected_regions: &ExpectedRegionSet,
    dimension: usize,
    stride: usize,
    accumulators: &mut [f64],
) -> Result<(), MultiscaleEmbeddingError> {
    let mut patch_index = 0_usize;
    for relation in link.nonzero_relations() {
        let source_index =
            find_patch_row(source_patch_table, relation.patch_id(), &mut patch_index)?;
        let source = source_patch_table.row(source_index)?;
        let Some(vector) = source.vector() else {
            continue;
        };
        let region_index = expected_regions
            .ids()
            .binary_search(relation.region_id())
            .map_err(|_| MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch)?;
        let base = region_index
            .checked_mul(stride)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let weight = declared_fraction_to_f64(relation);
        accumulators[base + dimension] += weight;
        for column in 0..dimension {
            let weighted = weight * f64::from(vector[column]);
            accumulators[base + column] += weighted;
        }
    }
    Ok(())
}

fn materialize_rows(
    expected_regions: &ExpectedRegionSet,
    dimension: usize,
    stride: usize,
    accumulators: &[f64],
) -> Result<Vec<RegionEmbeddingRow>, MultiscaleEmbeddingError> {
    let mut rows = try_vec_capacity::<RegionEmbeddingRow>(expected_regions.ids().len())?;
    for (region_index, region_id) in expected_regions.ids().iter().enumerate() {
        let base = region_index
            .checked_mul(stride)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let weight = accumulators[base + dimension];
        if weight > 0.0 {
            let mut vector = try_vec_capacity::<f32>(dimension)?;
            for column in 0..dimension {
                vector.push(canonical_positive_zero(
                    (accumulators[base + column] / weight) as f32,
                ));
            }
            rows.push(RegionEmbeddingRow::present(region_id.clone(), vector));
        } else {
            rows.push(RegionEmbeddingRow::non_present(
                region_id.clone(),
                EmbeddingStatus::MissingVector,
            )?);
        }
    }
    Ok(rows)
}

fn find_patch_row(
    table: &PatchEmbeddingTable,
    patch_id: &PatchId,
    patch_index: &mut usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    while *patch_index < table.row_count() {
        let observed = table.row(*patch_index)?.patch_id();
        match observed.cmp(patch_id) {
            std::cmp::Ordering::Less => {
                *patch_index = patch_index
                    .checked_add(1)
                    .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
            }
            std::cmp::Ordering::Equal => return Ok(*patch_index),
            std::cmp::Ordering::Greater => break,
        }
    }
    Err(MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch)
}

fn declared_fraction_to_f64(relation: &PatchRegionDeclaration) -> f64 {
    (relation.numerator() as f64) / (relation.denominator() as f64)
}

fn try_vec_capacity<T>(capacity: usize) -> Result<Vec<T>, MultiscaleEmbeddingError> {
    let requested = capacity
        .checked_mul(size_of::<T>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| MultiscaleEmbeddingError::AllocationFailed { requested })?;
    Ok(values)
}

fn try_vec_with_zeros<T: Clone + Default>(
    count: usize,
) -> Result<Vec<T>, MultiscaleEmbeddingError> {
    let mut values = try_vec_capacity(count)?;
    values.resize(count, T::default());
    Ok(values)
}

fn require_working(
    required: usize,
    budgets: EmbeddingFinalizationBudgets,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > budgets.maximum_working_bytes() {
        return Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded {
            required,
            maximum: budgets.maximum_working_bytes(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use marklab_data::{PatchId, RegionId};

    use super::{declared_fraction_to_f64, PatchRegionDeclaration};

    #[test]
    fn declared_fraction_conversion_casts_each_u64_before_division() {
        let relation = PatchRegionDeclaration::partial_overlap(
            PatchId::new("fraction-patch").expect("patch ID"),
            RegionId::new("fraction-region").expect("region ID"),
            9_007_199_254_740_993,
            9_007_199_254_740_995,
        )
        .expect("reduced fraction");
        assert_eq!(
            declared_fraction_to_f64(&relation).to_bits(),
            0x3feffffffffffffc
        );
    }
}
