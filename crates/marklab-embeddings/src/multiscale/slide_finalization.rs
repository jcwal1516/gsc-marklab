use std::mem::size_of;

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

use super::{
    EmbeddingEntityKind, EmbeddingFinalizationBudgets, ExpectedSlideSet, MultiscaleEmbeddingError,
    MultiscaleEmbeddingQcSummary, PatchEmbeddingTable, RegionEmbeddingTable, SlideEmbeddingRow,
    SlideEmbeddingTable, VerifiedDerivedSlideEmbeddingArtifactGraph,
};
use crate::digest::canonical_positive_zero;

/// Unforgeable result of deterministic slide recomputation from one verified lower table.
///
/// The contained table may be borrowed for canonical publication. Only full physical validation
/// of this candidate can mint a slide-table artifact receipt.
pub struct DerivedSlideEmbeddingTableCandidate {
    table: SlideEmbeddingTable,
    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
}

impl std::fmt::Debug for DerivedSlideEmbeddingTableCandidate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DerivedSlideEmbeddingTableCandidate")
            .field("qc_summary", &self.table.qc_summary())
            .field("dimension", &self.table.dimension())
            .finish_non_exhaustive()
    }
}

impl DerivedSlideEmbeddingTableCandidate {
    /// Borrow the exactly recomputed immutable slide table for canonical publication.
    pub fn table(&self) -> &SlideEmbeddingTable {
        &self.table
    }

    /// Number of canonical slide rows, exactly one for a valid expected-slide set.
    pub fn row_count(&self) -> usize {
        self.table.row_count()
    }

    /// Fixed output dimension inherited from the selected lower table.
    pub fn dimension(&self) -> u32 {
        self.table.dimension()
    }

    /// Format-independent logical identity of the recomputed slide table.
    pub fn logical_digest(&self) -> ContentDigest {
        self.table.logical_digest()
    }

    /// Recomputed factual status, shape, zero-vector, and logical summary.
    pub fn qc_summary(&self) -> MultiscaleEmbeddingQcSummary {
        self.table.qc_summary()
    }
}

/// Deterministically recompute the singleton slide row from a verified patch-table graph.
///
/// # Errors
///
/// Returns a typed binding, resource, allocation, shape, or table-construction error.
pub fn finalize_slide_embedding_table_from_patches(
    expected_slides: &ExpectedSlideSet,
    source_patch_table: &PatchEmbeddingTable,
    graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    budgets: EmbeddingFinalizationBudgets,
) -> Result<DerivedSlideEmbeddingTableCandidate, MultiscaleEmbeddingError> {
    finalize_slide(
        expected_slides,
        SlideSource::Patches(source_patch_table),
        graph,
        budgets,
    )
}

/// Deterministically recompute the singleton slide row from a verified region-table graph.
///
/// # Errors
///
/// Returns a typed binding, resource, allocation, shape, or table-construction error.
pub fn finalize_slide_embedding_table_from_regions(
    expected_slides: &ExpectedSlideSet,
    source_region_table: &RegionEmbeddingTable,
    graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    budgets: EmbeddingFinalizationBudgets,
) -> Result<DerivedSlideEmbeddingTableCandidate, MultiscaleEmbeddingError> {
    finalize_slide(
        expected_slides,
        SlideSource::Regions(source_region_table),
        graph,
        budgets,
    )
}

#[derive(Clone, Copy)]
enum SlideSource<'a> {
    Patches(&'a PatchEmbeddingTable),
    Regions(&'a RegionEmbeddingTable),
}

impl<'a> SlideSource<'a> {
    fn entity_kind(self) -> EmbeddingEntityKind {
        match self {
            Self::Patches(_) => EmbeddingEntityKind::Patch,
            Self::Regions(_) => EmbeddingEntityKind::Region,
        }
    }

    fn owning_slide_id(self) -> &'a SlideId {
        match self {
            Self::Patches(table) => table.owning_slide_id(),
            Self::Regions(table) => table.owning_slide_id(),
        }
    }

    fn row_count(self) -> usize {
        match self {
            Self::Patches(table) => table.row_count(),
            Self::Regions(table) => table.row_count(),
        }
    }

    fn dimension(self) -> u32 {
        match self {
            Self::Patches(table) => table.dimension(),
            Self::Regions(table) => table.dimension(),
        }
    }

    fn logical_digest(self) -> ContentDigest {
        match self {
            Self::Patches(table) => table.logical_digest(),
            Self::Regions(table) => table.logical_digest(),
        }
    }

    fn support_artifact_id(self) -> ArtifactId {
        match self {
            Self::Patches(table) => table.support_artifact_id(),
            Self::Regions(table) => table.support_artifact_id(),
        }
    }

    fn support_logical_digest(self) -> ContentDigest {
        match self {
            Self::Patches(table) => table.support_logical_digest(),
            Self::Regions(table) => table.support_logical_digest(),
        }
    }

    fn present_count(self) -> u64 {
        match self {
            Self::Patches(table) => table.qc_summary().present_count(),
            Self::Regions(table) => table.qc_summary().present_count(),
        }
    }

    fn retained_bytes(self) -> Result<usize, MultiscaleEmbeddingError> {
        match self {
            Self::Patches(table) => table.retained_bytes(),
            Self::Regions(table) => table.retained_bytes(),
        }
    }

    fn vector(self, index: usize) -> Result<Option<&'a [f32]>, MultiscaleEmbeddingError> {
        match self {
            Self::Patches(table) => Ok(table.row(index)?.vector()),
            Self::Regions(table) => Ok(table.row(index)?.vector()),
        }
    }
}

fn finalize_slide(
    expected_slides: &ExpectedSlideSet,
    source: SlideSource<'_>,
    graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    budgets: EmbeddingFinalizationBudgets,
) -> Result<DerivedSlideEmbeddingTableCandidate, MultiscaleEmbeddingError> {
    require_exact_bindings(expected_slides, source, graph)?;

    let contributor_rows =
        u64::try_from(source.row_count()).map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    if contributor_rows > budgets.maximum_contributor_rows() {
        return Err(
            MultiscaleEmbeddingError::DerivedEmbeddingContributorBudgetExceeded {
                required: contributor_rows,
                maximum: budgets.maximum_contributor_rows(),
            },
        );
    }
    let component_accumulations = source
        .present_count()
        .checked_mul(u64::from(source.dimension()))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    if component_accumulations > budgets.maximum_component_accumulations() {
        return Err(
            MultiscaleEmbeddingError::DerivedEmbeddingComponentBudgetExceeded {
                required: component_accumulations,
                maximum: budgets.maximum_component_accumulations(),
            },
        );
    }

    let graph_bytes = size_of::<VerifiedDerivedSlideEmbeddingArtifactGraph>();
    let retained_graph_bytes = if cfg!(feature = "parquet") {
        graph_bytes
    } else {
        0
    };
    let base_working = expected_slides
        .retained_bytes()?
        .checked_add(source.retained_bytes()?)
        .and_then(|value| value.checked_add(graph_bytes))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let candidate_retained =
        SlideEmbeddingTable::predicted_final_retained_bytes(expected_slides, source.dimension())?
            .checked_add(retained_graph_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    if candidate_retained > budgets.maximum_retained_bytes() {
        return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required: candidate_retained,
            maximum: budgets.maximum_retained_bytes(),
        });
    }

    let dimension =
        usize::try_from(source.dimension()).map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    let accumulator_bytes = dimension
        .checked_mul(size_of::<f64>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    require_working(
        base_working
            .checked_add(accumulator_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        budgets,
    )?;
    let mut sums = try_vec_with_zeros::<f64>(dimension)?;
    for index in 0..source.row_count() {
        let Some(vector) = source.vector(index)? else {
            continue;
        };
        for column in 0..dimension {
            sums[column] += f64::from(vector[column]);
        }
    }

    let output_vector_bytes = if source.present_count() == 0 {
        0
    } else {
        dimension
            .checked_mul(size_of::<f32>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?
    };
    require_working(
        base_working
            .checked_add(accumulator_bytes)
            .and_then(|value| value.checked_add(size_of::<SlideEmbeddingRow>()))
            .and_then(|value| value.checked_add(output_vector_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        budgets,
    )?;
    let slide_id = expected_slides
        .ids()
        .first()
        .ok_or(MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch)?
        .clone();
    let row = if source.present_count() == 0 {
        SlideEmbeddingRow::non_present(slide_id, crate::EmbeddingStatus::MissingVector)?
    } else {
        let divisor = source.present_count() as f64;
        let mut vector = try_vec_capacity::<f32>(dimension)?;
        for sum in sums.iter().take(dimension) {
            vector.push(canonical_positive_zero((*sum / divisor) as f32));
        }
        SlideEmbeddingRow::present(slide_id, vector)
    };
    let mut rows = try_vec_capacity::<SlideEmbeddingRow>(1)?;
    rows.push(row);
    let construction_bytes =
        SlideEmbeddingTable::construction_bytes(expected_slides, source.dimension(), &rows)?;
    require_working(
        base_working
            .checked_add(construction_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        budgets,
    )?;
    drop(sums);
    let table = SlideEmbeddingTable::from_rows(
        source.dimension(),
        expected_slides,
        graph.expected_slides_artifact_id,
        graph.slide_support_artifact_id,
        graph.slide_support_logical_digest,
        graph.provenance_artifact_id,
        graph.provenance_logical_digest,
        rows,
        construction_bytes,
    )?;
    Ok(DerivedSlideEmbeddingTableCandidate {
        table,
        #[cfg(feature = "parquet")]
        graph,
    })
}

fn require_exact_bindings(
    expected_slides: &ExpectedSlideSet,
    source: SlideSource<'_>,
    graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
) -> Result<(), MultiscaleEmbeddingError> {
    let row_count =
        u64::try_from(source.row_count()).map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    if expected_slides.ids().len() != 1
        || expected_slides.owning_slide_id() != source.owning_slide_id()
        || crate::multiscale::matrix_artifact::slide_lineage_digest(
            expected_slides.owning_slide_id(),
        ) != graph.owning_slide_binding_digest
        || expected_slides.logical_digest() != graph.expected_slides_logical_digest
        || source.entity_kind() != graph.source_entity_kind
        || source.logical_digest() != graph.source_table_logical_digest
        || row_count != graph.source_table_row_count
        || source.support_artifact_id() != graph.source_support_artifact_id
        || source.support_logical_digest() != graph.source_support_logical_digest
        || source.dimension() != graph.output_dimension
    {
        return Err(MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch);
    }
    Ok(())
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
