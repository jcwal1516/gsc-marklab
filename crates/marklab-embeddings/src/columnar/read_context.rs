//! Caller-supplied logical bindings for reconstructing physical columnar artifacts.

use marklab_data::{CohortHierarchy, PatchId};
use marklab_project::{ArtifactId, ContentDigest};

use crate::{
    CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellPatchAnchor, CellPatchAssignment,
    CellPatchAssignmentMode, CellPatchAssignmentStatus, CellPatchContributor, CellPatchEdge,
    CellPatchLink, CellPatchLinkBindings, DeclaredCellPatchAssignment, EmbeddingStatus,
    ExpectedCellSet, ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet,
    MultiscaleEmbeddingError, PatchEmbeddingContext, PatchFootprint, PatchFootprintSet,
    PatchOverlapGraph, PatchRegionAssessment, PatchRegionAssessmentBindings,
    PatchRegionDeclaration, PatchRegionLink,
};

use super::{ColumnarError, EmbeddingColumnarBudgets, MultiscaleEmbeddingTableReadBindings};

/// Logical dependencies and statuses omitted from the physical cell row-link columns.
pub struct CellEmbeddingRowLinkReadContext<'a> {
    /// Exact canonical cell set.
    pub expected: &'a ExpectedCellSet,
    /// Hierarchy against which cell membership is validated.
    pub hierarchy: &'a CohortHierarchy,
    /// One explicit extraction status per canonical cell.
    pub statuses: &'a [EmbeddingStatus],
    /// Source-cell artifact.
    pub source_cells_artifact_id: ArtifactId,
    /// Source-vector artifact.
    pub source_vectors_artifact_id: ArtifactId,
    /// Expected-cell artifact.
    pub expected_cells_artifact_id: ArtifactId,
    /// Identity-map artifact.
    pub identity_map_artifact_id: ArtifactId,
    /// Converter artifact.
    pub converter_artifact_id: ArtifactId,
}

impl CellEmbeddingRowLinkReadContext<'_> {
    pub(crate) fn rebuild(
        &self,
        entries: Vec<CellEmbeddingRowLinkEntry>,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<CellEmbeddingRowLink, ColumnarError> {
        if self.statuses.len() != self.expected.cells().len()
            || entries
                .iter()
                .zip(self.statuses)
                .any(|(row, status)| row.status() != *status)
        {
            return Err(ColumnarError::ArtifactBindingMismatch);
        }
        CellEmbeddingRowLink::new(
            self.source_cells_artifact_id,
            self.source_vectors_artifact_id,
            self.expected_cells_artifact_id,
            self.identity_map_artifact_id,
            self.converter_artifact_id,
            self.expected,
            self.hierarchy,
            entries,
            budgets.maximum_retained_bytes(),
        )
        .map_err(map_embedding_error)
    }
}

/// Exact support needed to reconstruct patch origins.
pub struct PatchFootprintReadContext<'a> {
    /// Hierarchy against which patch membership is validated.
    pub hierarchy: &'a CohortHierarchy,
    /// Exact canonical patch set.
    pub expected: &'a ExpectedPatchSet,
    /// Expected-patch artifact.
    pub expected_patches_artifact_id: ArtifactId,
    /// Image frame and patch geometry.
    pub context: &'a PatchEmbeddingContext,
    /// Patch-context artifact.
    pub patch_context_artifact_id: ArtifactId,
}

impl PatchFootprintReadContext<'_> {
    pub(crate) fn rebuild(
        &self,
        rows: Vec<PatchFootprint>,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<PatchFootprintSet, ColumnarError> {
        PatchFootprintSet::new(
            self.hierarchy,
            self.expected,
            self.expected_patches_artifact_id,
            self.context,
            self.patch_context_artifact_id,
            rows,
            budgets.maximum_retained_bytes(),
        )
        .map_err(map_multiscale_error)
    }
}

/// Exact geometry and work bound for reconstructing an overlap graph.
pub struct PatchOverlapReadContext<'a> {
    /// Exact canonical patch set.
    pub expected: &'a ExpectedPatchSet,
    /// Image frame and patch geometry.
    pub context: &'a PatchEmbeddingContext,
    /// Exact patch origins.
    pub footprints: &'a PatchFootprintSet,
    /// Footprint artifact.
    pub patch_footprints_artifact_id: ArtifactId,
    /// Maximum bytes during indexed graph derivation.
    pub maximum_working_bytes: usize,
}

impl PatchOverlapReadContext<'_> {
    pub(crate) fn rebuild(
        &self,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<PatchOverlapGraph, ColumnarError> {
        PatchOverlapGraph::derive(
            self.expected,
            self.context,
            self.footprints,
            self.patch_footprints_artifact_id,
            budgets.maximum_retained_bytes(),
            self.maximum_working_bytes
                .min(budgets.maximum_retained_bytes()),
        )
        .map_err(map_multiscale_error)
    }
}

/// Common logical support for either half of a physical cell-to-patch link.
pub struct CellPatchReadContext<'a> {
    /// Hierarchy against which cell and patch membership is validated.
    pub hierarchy: &'a CohortHierarchy,
    /// Exact canonical cell set.
    pub expected_cells: &'a ExpectedCellSet,
    /// Exact canonical patch set.
    pub expected_patches: &'a ExpectedPatchSet,
    /// Image frame and patch geometry.
    pub context: &'a PatchEmbeddingContext,
    /// Exact patch origins.
    pub footprints: &'a PatchFootprintSet,
    /// Exact producer, support, and coordinate-frame bindings.
    pub bindings: &'a CellPatchLinkBindings,
    /// Assignment mode declared by the caller.
    pub mode: CellPatchAssignmentMode,
    /// Maximum geometry candidate checks during contained assignment derivation.
    pub maximum_candidate_checks: usize,
    /// Maximum bytes during domain reconstruction.
    pub maximum_working_bytes: usize,
}

/// Assignment-table read context with the independently supplied edge half.
pub struct CellPatchAssignmentReadContext<'a> {
    /// Common logical support.
    pub common: CellPatchReadContext<'a>,
    /// Exact counterpart edges in canonical order.
    pub edges: &'a [CellPatchEdge],
}

/// Edge-table read context with the independently supplied assignment half.
pub struct CellPatchEdgeReadContext<'a> {
    /// Common logical support.
    pub common: CellPatchReadContext<'a>,
    /// Exact counterpart assignments in canonical order.
    pub assignments: &'a [CellPatchAssignment],
}

/// Exact logical support and assessment binding for a sparse patch-region table.
pub struct PatchRegionReadContext<'a> {
    /// Exact canonical patch set.
    pub expected_patches: &'a ExpectedPatchSet,
    /// Exact canonical region set.
    pub expected_regions: &'a ExpectedRegionSet,
    /// Image frame and patch geometry.
    pub context: &'a PatchEmbeddingContext,
    /// Exact patch origins.
    pub footprints: &'a PatchFootprintSet,
    /// Converter and support bindings for the exhaustive assessment.
    pub bindings: &'a PatchRegionAssessmentBindings,
    /// Assessment artifact identity; this read does not verify that separate artifact.
    pub assessment_artifact_id: ArtifactId,
    /// Assessment content identity supplied by the caller.
    pub assessment_content_digest: ContentDigest,
    /// Maximum bytes during domain reconstruction.
    pub maximum_working_bytes: usize,
}

impl PatchRegionReadContext<'_> {
    pub(crate) fn rebuild(
        &self,
        rows: Vec<PatchRegionDeclaration>,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<PatchRegionLink, ColumnarError> {
        let maximum = budgets.maximum_retained_bytes();
        let working = self.maximum_working_bytes.min(maximum);
        let assessment = PatchRegionAssessment::new(
            self.expected_patches,
            self.expected_regions,
            self.context,
            self.footprints,
            self.bindings,
            rows,
            maximum,
            working,
        )
        .map_err(map_multiscale_error)?;
        PatchRegionLink::from_exhaustive_assessment(
            &assessment,
            self.assessment_artifact_id,
            self.assessment_content_digest,
            maximum,
            working,
        )
        .map_err(map_multiscale_error)
    }
}

macro_rules! matrix_context {
    ($name:ident, $expected:ty, $doc:literal) => {
        #[doc = $doc]
        pub struct $name<'a> {
            /// Exact canonical entity set.
            pub expected: &'a $expected,
            /// Exact expected-set, support, and provenance logical bindings.
            pub bindings: MultiscaleEmbeddingTableReadBindings,
        }
    };
}

matrix_context!(
    PatchEmbeddingTableReadContext,
    ExpectedPatchSet,
    "Logical support for a plain patch matrix read."
);
matrix_context!(
    RegionEmbeddingTableReadContext,
    ExpectedRegionSet,
    "Logical support for a plain region matrix read; no finalization proof is produced."
);
matrix_context!(
    SlideEmbeddingTableReadContext,
    ExpectedSlideSet,
    "Logical support for a plain slide matrix read; no finalization proof is produced."
);

pub(crate) struct CellPatchAssignmentReadRow {
    pub(crate) anchor: CellPatchAnchor,
    pub(crate) status: CellPatchAssignmentStatus,
    pub(crate) edge_start: u64,
    pub(crate) edge_count: u64,
}

pub(crate) struct CellPatchEdgeReadRow {
    pub(crate) assignment_row: u64,
    pub(crate) patch_id: PatchId,
    pub(crate) numerator: Option<u64>,
    pub(crate) denominator: Option<u64>,
}

impl CellPatchAssignmentReadContext<'_> {
    pub(crate) fn rebuild(
        &self,
        rows: Vec<CellPatchAssignmentReadRow>,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<CellPatchLink, ColumnarError> {
        if rows.len() != self.common.expected_cells.cells().len() {
            return Err(ColumnarError::ArtifactBindingMismatch);
        }
        let held = rows.iter().try_fold(
            rows.capacity()
                .checked_mul(size_of::<CellPatchAssignmentReadRow>())
                .ok_or(ColumnarError::SizeOverflow)?,
            |total, row| {
                total
                    .checked_add(row.anchor.cell_id().as_str().len())
                    .ok_or(ColumnarError::SizeOverflow)
            },
        )?;
        let link = with_retained_bytes(
            held,
            reconstruction_budgets(budgets, self.common.maximum_working_bytes),
            |remaining| {
                self.common.rebuild_parts(
                    rows.len(),
                    |index| Ok(rows[index].anchor.clone()),
                    |index| {
                        let row = &rows[index];
                        let start = usize::try_from(row.edge_start)
                            .map_err(|_| ColumnarError::SizeOverflow)?;
                        let count = usize::try_from(row.edge_count)
                            .map_err(|_| ColumnarError::SizeOverflow)?;
                        let end = start
                            .checked_add(count)
                            .ok_or(ColumnarError::SizeOverflow)?;
                        let group = self
                            .edges
                            .get(start..end)
                            .ok_or(ColumnarError::ArtifactBindingMismatch)?;
                        if group
                            .iter()
                            .any(|edge| usize::try_from(edge.assignment_row()).ok() != Some(index))
                        {
                            return Err(ColumnarError::ArtifactBindingMismatch);
                        }
                        copy_contributors(
                            group.iter().map(|edge| {
                                let weight = edge
                                    .weight()
                                    .ok_or(ColumnarError::ArtifactBindingMismatch)?;
                                CellPatchContributor::new(
                                    edge.patch_id().clone(),
                                    weight.numerator(),
                                    weight.denominator(),
                                )
                                .map_err(map_multiscale_error)
                            }),
                            count,
                        )
                    },
                    remaining,
                )
            },
        )?;
        if link.edges() != self.edges
            || link.assignments().iter().zip(&rows).any(|(actual, row)| {
                actual.cell_id() != row.anchor.cell_id()
                    || actual.anchor_px() != row.anchor.anchor_px()
                    || actual.status() != row.status
                    || actual.edge_start() != row.edge_start
                    || actual.edge_count() != row.edge_count
            })
        {
            return Err(ColumnarError::ArtifactBindingMismatch);
        }
        Ok(link)
    }
}

impl CellPatchEdgeReadContext<'_> {
    pub(crate) fn rebuild(
        &self,
        rows: Vec<CellPatchEdgeReadRow>,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<CellPatchLink, ColumnarError> {
        if self.assignments.len() != self.common.expected_cells.cells().len() {
            return Err(ColumnarError::ArtifactBindingMismatch);
        }
        let held = rows.iter().try_fold(
            rows.capacity()
                .checked_mul(size_of::<CellPatchEdgeReadRow>())
                .ok_or(ColumnarError::SizeOverflow)?,
            |total, row| {
                total
                    .checked_add(row.patch_id.as_str().len())
                    .ok_or(ColumnarError::SizeOverflow)
            },
        )?;
        let link = with_retained_bytes(
            held,
            reconstruction_budgets(budgets, self.common.maximum_working_bytes),
            |remaining| {
                self.common.rebuild_parts(
                    self.assignments.len(),
                    |index| {
                        CellPatchAnchor::new(
                            self.assignments[index].cell_id().clone(),
                            self.assignments[index].anchor_px(),
                        )
                        .map_err(map_multiscale_error)
                    },
                    |index| {
                        let assignment = &self.assignments[index];
                        let start = usize::try_from(assignment.edge_start())
                            .map_err(|_| ColumnarError::SizeOverflow)?;
                        let count = usize::try_from(assignment.edge_count())
                            .map_err(|_| ColumnarError::SizeOverflow)?;
                        let end = start
                            .checked_add(count)
                            .ok_or(ColumnarError::SizeOverflow)?;
                        let group = rows
                            .get(start..end)
                            .ok_or(ColumnarError::ArtifactBindingMismatch)?;
                        copy_contributors(
                            group.iter().map(|row| {
                                if usize::try_from(row.assignment_row).ok() != Some(index) {
                                    return Err(ColumnarError::ArtifactBindingMismatch);
                                }
                                CellPatchContributor::new(
                                    row.patch_id.clone(),
                                    row.numerator
                                        .ok_or(ColumnarError::ArtifactBindingMismatch)?,
                                    row.denominator
                                        .ok_or(ColumnarError::ArtifactBindingMismatch)?,
                                )
                                .map_err(map_multiscale_error)
                            }),
                            count,
                        )
                    },
                    remaining,
                )
            },
        )?;
        if link.assignments() != self.assignments
            || link.edge_count() != rows.len()
            || link.edges().iter().zip(&rows).any(|(actual, row)| {
                actual.assignment_row() != row.assignment_row
                    || actual.patch_id() != &row.patch_id
                    || actual.weight().map(|weight| weight.numerator()) != row.numerator
                    || actual.weight().map(|weight| weight.denominator()) != row.denominator
            })
        {
            return Err(ColumnarError::ArtifactBindingMismatch);
        }
        Ok(link)
    }
}

impl CellPatchReadContext<'_> {
    fn rebuild_parts(
        &self,
        count: usize,
        mut anchor: impl FnMut(usize) -> Result<CellPatchAnchor, ColumnarError>,
        mut contributors: impl FnMut(usize) -> Result<Vec<CellPatchContributor>, ColumnarError>,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<CellPatchLink, ColumnarError> {
        let maximum = budgets.maximum_retained_bytes();
        let working = self.maximum_working_bytes.min(maximum);
        let cell_text = self
            .expected_cells
            .cells()
            .iter()
            .try_fold(0_usize, |total, id| {
                total
                    .checked_add(id.as_str().len())
                    .ok_or(ColumnarError::SizeOverflow)
            })?;
        match self.mode {
            CellPatchAssignmentMode::ContainedShared => {
                let required = count
                    .checked_mul(size_of::<CellPatchAnchor>())
                    .and_then(|bytes| bytes.checked_add(cell_text))
                    .ok_or(ColumnarError::SizeOverflow)?;
                super::multiscale::enforce_retained_budget(required, budgets)?;
                let mut anchors = try_capacity(count)?;
                for index in 0..count {
                    anchors.push(anchor(index)?);
                }
                CellPatchLink::derive_contained_shared(
                    self.hierarchy,
                    self.expected_cells,
                    self.expected_patches,
                    self.context,
                    self.footprints,
                    self.bindings,
                    anchors,
                    self.maximum_candidate_checks,
                    maximum,
                    working,
                )
                .map_err(map_multiscale_error)
            }
            CellPatchAssignmentMode::DeclaredWeightedInterpolation => {
                let maximum_patch_text = self
                    .expected_patches
                    .ids()
                    .iter()
                    .map(|id| id.as_str().len())
                    .max()
                    .unwrap_or(0);
                let required = count
                    .checked_mul(
                        size_of::<DeclaredCellPatchAssignment>()
                            + 4 * size_of::<CellPatchContributor>(),
                    )
                    .and_then(|bytes| {
                        count
                            .checked_mul(4)
                            .and_then(|edges| edges.checked_mul(maximum_patch_text))
                            .and_then(|text| bytes.checked_add(text))
                    })
                    .and_then(|bytes| bytes.checked_add(cell_text))
                    .ok_or(ColumnarError::SizeOverflow)?;
                super::multiscale::enforce_retained_budget(required, budgets)?;
                let mut declarations = try_capacity(count)?;
                for index in 0..count {
                    declarations.push(DeclaredCellPatchAssignment::new(
                        anchor(index)?,
                        contributors(index)?,
                    ));
                }
                CellPatchLink::from_declared_weighted_interpolation(
                    self.hierarchy,
                    self.expected_cells,
                    self.expected_patches,
                    self.context,
                    self.footprints,
                    self.bindings,
                    declarations,
                    maximum,
                    working,
                )
                .map_err(map_multiscale_error)
            }
        }
    }
}

fn copy_contributors(
    rows: impl Iterator<Item = Result<CellPatchContributor, ColumnarError>>,
    count: usize,
) -> Result<Vec<CellPatchContributor>, ColumnarError> {
    if count > 4 {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let mut output = try_capacity(count)?;
    for row in rows {
        output.push(row?);
    }
    Ok(output)
}

fn reconstruction_budgets(
    budgets: EmbeddingColumnarBudgets,
    maximum_working_bytes: usize,
) -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        budgets.maximum_file_bytes(),
        budgets.maximum_retained_bytes().min(maximum_working_bytes),
        budgets.maximum_row_group_bytes(),
        budgets.maximum_decoded_bytes(),
    )
}

pub(crate) fn try_capacity<T>(count: usize) -> Result<Vec<T>, ColumnarError> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(ColumnarError::SizeOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ColumnarError::AllocationFailed { requested })?;
    Ok(values)
}

pub(crate) fn cell_patch_retained_bytes(link: &CellPatchLink) -> Result<usize, ColumnarError> {
    let base = size_of::<CellPatchLink>()
        .checked_add(link.owning_slide_id().as_str().len())
        .and_then(|bytes| {
            bytes.checked_add(
                link.assignments()
                    .len()
                    .checked_mul(size_of::<CellPatchAssignment>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(link.edges().len().checked_mul(size_of::<CellPatchEdge>())?)
        })
        .ok_or(ColumnarError::SizeOverflow)?;
    link.assignments()
        .iter()
        .map(|row| row.cell_id().as_str())
        .chain(link.edges().iter().map(|row| row.patch_id().as_str()))
        .try_fold(base, |bytes, id| {
            bytes
                .checked_add(id.len())
                .ok_or(ColumnarError::SizeOverflow)
        })
}

pub(crate) fn patch_region_retained_bytes(link: &PatchRegionLink) -> Result<usize, ColumnarError> {
    let base = size_of::<PatchRegionLink>()
        .checked_add(link.owning_slide_id().as_str().len())
        .and_then(|bytes| {
            bytes.checked_add(
                link.nonzero_relations()
                    .len()
                    .checked_mul(size_of::<PatchRegionDeclaration>())?,
            )
        })
        .ok_or(ColumnarError::SizeOverflow)?;
    link.nonzero_relations()
        .iter()
        .try_fold(base, |bytes, row| {
            bytes
                .checked_add(row.patch_id().as_str().len())
                .and_then(|bytes| bytes.checked_add(row.region_id().as_str().len()))
                .ok_or(ColumnarError::SizeOverflow)
        })
}

pub(crate) fn overlap_retained_bytes(
    expected: &ExpectedPatchSet,
    graph: &PatchOverlapGraph,
) -> Result<usize, ColumnarError> {
    let base = size_of::<PatchOverlapGraph>()
        .checked_add(
            expected
                .ids()
                .len()
                .checked_mul(size_of::<PatchId>() + size_of::<usize>())
                .ok_or(ColumnarError::SizeOverflow)?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                graph
                    .edge_count()
                    .checked_mul(size_of::<crate::PatchOverlapEdge>())?,
            )
        })
        .ok_or(ColumnarError::SizeOverflow)?;
    expected
        .ids()
        .iter()
        .map(|id| id.as_str())
        .chain(graph.edges().iter().flat_map(|edge| {
            [
                edge.left_patch_id().as_str(),
                edge.right_patch_id().as_str(),
            ]
        }))
        .try_fold(base, |bytes, id| {
            bytes
                .checked_add(id.len())
                .ok_or(ColumnarError::SizeOverflow)
        })
}

/// Charge a live collector or reconstructed output to an existing bounded reader.
pub(crate) fn with_retained_bytes<T>(
    held: usize,
    budgets: EmbeddingColumnarBudgets,
    operation: impl FnOnce(EmbeddingColumnarBudgets) -> Result<T, ColumnarError>,
) -> Result<T, ColumnarError> {
    let maximum = budgets.maximum_retained_bytes();
    let remaining = maximum
        .checked_sub(held)
        .ok_or(ColumnarError::RetainedByteBudgetExceeded {
            required: held,
            maximum,
        })?;
    operation(EmbeddingColumnarBudgets::new(
        budgets.maximum_file_bytes(),
        remaining,
        budgets.maximum_row_group_bytes(),
        budgets.maximum_decoded_bytes(),
    ))
    .map_err(|error| match error {
        ColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: observed,
        } if observed == remaining => match required.checked_add(held) {
            Some(required) => ColumnarError::RetainedByteBudgetExceeded { required, maximum },
            None => ColumnarError::SizeOverflow,
        },
        error => error,
    })
}

pub(crate) fn map_multiscale_error(error: MultiscaleEmbeddingError) -> ColumnarError {
    match error {
        MultiscaleEmbeddingError::SizeOverflow => ColumnarError::SizeOverflow,
        MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum }
        | MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum } => {
            ColumnarError::RetainedByteBudgetExceeded { required, maximum }
        }
        MultiscaleEmbeddingError::AllocationFailed { requested } => {
            ColumnarError::AllocationFailed { requested }
        }
        _ => ColumnarError::ArtifactBindingMismatch,
    }
}

pub(crate) fn map_embedding_error(error: crate::EmbeddingError) -> ColumnarError {
    match error {
        crate::EmbeddingError::SizeOverflow => ColumnarError::SizeOverflow,
        crate::EmbeddingError::RetainedByteBudgetExceeded { required, maximum } => {
            ColumnarError::RetainedByteBudgetExceeded { required, maximum }
        }
        crate::EmbeddingError::AllocationFailed { requested } => {
            ColumnarError::AllocationFailed { requested }
        }
        _ => ColumnarError::ArtifactBindingMismatch,
    }
}
