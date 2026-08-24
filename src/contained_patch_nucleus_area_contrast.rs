use std::mem::size_of;

use marklab_embeddings::{
    CellPatchAssignmentMode, CellPatchLink, VerifiedCellPatchInputArtifactGraph,
    VerifiedCellPatchLinkArtifact,
};
use marklab_workflow::{ArtifactId, ContentDigest, MarklabProject};
use thiserror::Error;

use crate::{
    binary_nucleus_area_contrast::{
        declared_binary_group_nucleus_area_contrast, DeclaredBinaryGroupNucleusAreaContrast,
        DeclaredBinaryGroupNucleusAreaContrastError,
    },
    scalar_mark::{DeclaredScalarPatternInput, NucleusAreaUm2MarkDeclaration},
};

/// Availability of the equal-patch contained nucleus-area contrast.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainedPatchBinaryNucleusAreaContrastStatus {
    /// At least one represented patch contains both exact binary groups.
    Available,
    /// No represented patch contains both exact binary groups.
    InsufficientEligiblePatches,
}

/// Descriptive equal-patch mean of contained binary-group nucleus-area contrasts.
#[derive(Clone, Debug, PartialEq)]
pub struct ContainedPatchBinaryNucleusAreaContrast {
    status: ContainedPatchBinaryNucleusAreaContrastStatus,
    whole_input_contrast: DeclaredBinaryGroupNucleusAreaContrast,
    assignment_count: u64,
    edge_count: u64,
    represented_patch_count: u64,
    eligible_patch_count: u64,
    eligible_marked_incidence_count: u64,
    eligible_unmarked_incidence_count: u64,
    eligible_incidence_count: u64,
    excluded_incidence_count: u64,
    equal_patch_mean_marked_minus_unmarked_nucleus_area_um2: Option<f64>,
    cell_patch_logical_digest: ContentDigest,
    expected_cells_artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    patch_context_artifact_id: ArtifactId,
    patch_footprints_artifact_id: ArtifactId,
    cell_patch_producer_artifact_id: ArtifactId,
    cell_patch_assignment_artifact_id: ArtifactId,
    cell_patch_edge_artifact_id: ArtifactId,
}

impl ContainedPatchBinaryNucleusAreaContrast {
    /// Whether at least one patch contains marked and unmarked incidences.
    pub fn status(&self) -> ContainedPatchBinaryNucleusAreaContrastStatus {
        self.status
    }

    /// Exact S12 whole-input validation, declarations, value identity, and contrast.
    pub fn whole_input_contrast(&self) -> &DeclaredBinaryGroupNucleusAreaContrast {
        &self.whole_input_contrast
    }

    /// Exact expected-cell assignment-row count.
    pub fn assignment_count(&self) -> u64 {
        self.assignment_count
    }

    /// Exact contained cell-patch incidence count.
    pub fn edge_count(&self) -> u64 {
        self.edge_count
    }

    /// Number of distinct patches represented by at least one incidence.
    pub fn represented_patch_count(&self) -> u64 {
        self.represented_patch_count
    }

    /// Number of represented patches containing both exact binary groups.
    pub fn eligible_patch_count(&self) -> u64 {
        self.eligible_patch_count
    }

    /// Marked incidences belonging to eligible patches.
    pub fn eligible_marked_incidence_count(&self) -> u64 {
        self.eligible_marked_incidence_count
    }

    /// Unmarked incidences belonging to eligible patches.
    pub fn eligible_unmarked_incidence_count(&self) -> u64 {
        self.eligible_unmarked_incidence_count
    }

    /// All marked and unmarked incidences belonging to eligible patches.
    pub fn eligible_incidence_count(&self) -> u64 {
        self.eligible_incidence_count
    }

    /// Incidences belonging to patches without both binary groups.
    pub fn excluded_incidence_count(&self) -> u64 {
        self.excluded_incidence_count
    }

    /// Equal-patch mean of marked-minus-unmarked nucleus-area contrasts.
    pub fn equal_patch_mean_marked_minus_unmarked_nucleus_area_um2(&self) -> Option<f64> {
        self.equal_patch_mean_marked_minus_unmarked_nucleus_area_um2
    }

    /// Format-independent identity of the exact contained cell-patch link.
    pub fn cell_patch_logical_digest(&self) -> ContentDigest {
        self.cell_patch_logical_digest
    }

    /// Expected-cell artifact identity bound by the link.
    pub fn expected_cells_artifact_id(&self) -> ArtifactId {
        self.expected_cells_artifact_id
    }

    /// Expected-patch artifact identity bound by the link.
    pub fn expected_patches_artifact_id(&self) -> ArtifactId {
        self.expected_patches_artifact_id
    }

    /// Exact patch-context artifact identity bound by the link.
    pub fn patch_context_artifact_id(&self) -> ArtifactId {
        self.patch_context_artifact_id
    }

    /// Exact patch-footprint artifact identity bound by the link.
    pub fn patch_footprints_artifact_id(&self) -> ArtifactId {
        self.patch_footprints_artifact_id
    }

    /// Exact contained-link producer artifact identity.
    pub fn cell_patch_producer_artifact_id(&self) -> ArtifactId {
        self.cell_patch_producer_artifact_id
    }

    /// Exact verified physical assignment-table identity.
    pub fn cell_patch_assignment_artifact_id(&self) -> ArtifactId {
        self.cell_patch_assignment_artifact_id
    }

    /// Exact verified physical edge-table identity.
    pub fn cell_patch_edge_artifact_id(&self) -> ArtifactId {
        self.cell_patch_edge_artifact_id
    }
}

/// Invalid scalar/link binding, unsupported mode, caller limit, or allocation.
#[derive(Debug, Error)]
pub enum ContainedPatchBinaryNucleusAreaContrastError {
    /// Exact S12 project, provenance, value, identity, or row admission failed.
    #[error(transparent)]
    WholeInputContrast(#[from] DeclaredBinaryGroupNucleusAreaContrastError),
    /// Declared interpolation is not an admitted patch-local comparison weight.
    #[error("contained-patch binary nucleus-area contrast requires contained-shared assignments")]
    UnsupportedAssignmentMode,
    /// The declared scalar input and cell-patch link name different owning slides.
    #[error("contained-patch binary nucleus-area owning slides disagree")]
    OwningSlideBindingMismatch,
    /// One declared row and cell-patch assignment name different cells.
    #[error("contained-patch binary nucleus-area cell identity disagrees at row {row}")]
    CellIdBindingMismatch {
        /// First mismatched or absent assignment row.
        row: usize,
    },
    /// The verified managed input graph does not describe the live link.
    #[error("contained-patch binary nucleus-area graph disagrees with the live link")]
    CellPatchGraphBindingMismatch,
    /// The paired physical assignment/edge receipt does not describe the live link.
    #[error("contained-patch binary nucleus-area receipt disagrees with the live link")]
    CellPatchReceiptBindingMismatch,
    /// Assignment traversal exceeds the caller maximum.
    #[error("contained-patch binary nucleus-area assignments {required} exceed caller maximum {maximum}")]
    AssignmentCountBudgetExceeded {
        /// Exact assignment-row count.
        required: usize,
        /// Caller-provided assignment maximum.
        maximum: usize,
    },
    /// Edge traversal exceeds the caller maximum.
    #[error(
        "contained-patch binary nucleus-area edges {required} exceed caller maximum {maximum}"
    )]
    EdgeCountBudgetExceeded {
        /// Exact incidence count.
        required: usize,
        /// Caller-provided edge maximum.
        maximum: usize,
    },
    /// Exact edge-index storage exceeds the caller maximum.
    #[error("contained-patch binary nucleus-area working bytes {required} exceed caller maximum {maximum}")]
    WorkingByteBudgetExceeded {
        /// Exact incremental heap byte count.
        required: usize,
        /// Caller-provided working-byte maximum.
        maximum: usize,
    },
    /// A checked count or byte computation overflowed.
    #[error("contained-patch binary nucleus-area size computation overflowed")]
    SizeOverflow,
    /// Exact bounded edge-index allocation failed.
    #[error("contained-patch binary nucleus-area allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Exact requested working bytes.
        requested: usize,
    },
}

/// Compare declared nucleus area by binary group within exact contained patches.
///
/// S12 first validates and binds the whole declared scalar input. This caller then joins those rows
/// to the existing contained-shared cell-patch graph and physical receipt, sorts incidences in
/// canonical patch order, and gives every eligible patch equal descriptive weight. Overlapping
/// patches repeat exact incidences and are not independent replicates. Runtime is
/// `O(N + E log E)` with one `E`-element edge-index allocation. The result proves no observation
/// window, spatial association, segmentation accuracy, patient effect, inference, real-source
/// result, or biological meaning.
///
/// # Errors
///
/// Returns a typed category when S12 validation fails, assignment mode, slide, ordered CellIds,
/// graph, or receipt disagree, a caller limit is insufficient, a checked size overflows, or the
/// bounded edge-index allocation fails.
#[allow(clippy::too_many_arguments)]
pub fn contained_patch_binary_nucleus_area_contrast(
    project: &MarklabProject,
    input: &DeclaredScalarPatternInput<'_>,
    nucleus_area_mark: NucleusAreaUm2MarkDeclaration,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    receipt: VerifiedCellPatchLinkArtifact,
    maximum_rows: usize,
    maximum_assignments: usize,
    maximum_edges: usize,
    maximum_working_bytes: usize,
) -> Result<ContainedPatchBinaryNucleusAreaContrast, ContainedPatchBinaryNucleusAreaContrastError> {
    let whole_input_contrast = declared_binary_group_nucleus_area_contrast(
        project,
        input,
        nucleus_area_mark,
        maximum_rows,
    )?;
    if link.mode() != CellPatchAssignmentMode::ContainedShared {
        return Err(ContainedPatchBinaryNucleusAreaContrastError::UnsupportedAssignmentMode);
    }
    if input.owning_slide_id() != link.owning_slide_id() {
        return Err(ContainedPatchBinaryNucleusAreaContrastError::OwningSlideBindingMismatch);
    }

    let assignment_count = link.assignment_count();
    let edge_count = link.edge_count();
    if whole_input_contrast.row_count() != assignment_count {
        return Err(
            ContainedPatchBinaryNucleusAreaContrastError::CellIdBindingMismatch {
                row: whole_input_contrast.row_count().min(assignment_count),
            },
        );
    }
    for (row, (declared, assignment)) in input.cell_ids().iter().zip(link.assignments()).enumerate()
    {
        if declared != assignment.cell_id() {
            return Err(
                ContainedPatchBinaryNucleusAreaContrastError::CellIdBindingMismatch { row },
            );
        }
    }

    let assignment_count_u64 = u64::try_from(assignment_count)
        .map_err(|_| ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
    let edge_count_u64 = u64::try_from(edge_count)
        .map_err(|_| ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
    if graph.logical_digest() != link.logical_digest()
        || graph.assignment_count() != assignment_count_u64
        || graph.edge_count() != edge_count_u64
        || graph.producer_artifact_id() != link.producer_artifact_id()
    {
        return Err(ContainedPatchBinaryNucleusAreaContrastError::CellPatchGraphBindingMismatch);
    }
    if receipt.logical_digest() != link.logical_digest()
        || receipt.assignment_count() != assignment_count_u64
        || receipt.edge_count() != edge_count_u64
    {
        return Err(ContainedPatchBinaryNucleusAreaContrastError::CellPatchReceiptBindingMismatch);
    }
    if assignment_count > maximum_assignments {
        return Err(
            ContainedPatchBinaryNucleusAreaContrastError::AssignmentCountBudgetExceeded {
                required: assignment_count,
                maximum: maximum_assignments,
            },
        );
    }
    if edge_count > maximum_edges {
        return Err(
            ContainedPatchBinaryNucleusAreaContrastError::EdgeCountBudgetExceeded {
                required: edge_count,
                maximum: maximum_edges,
            },
        );
    }
    let working_bytes = edge_count
        .checked_mul(size_of::<usize>())
        .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
    if working_bytes > maximum_working_bytes {
        return Err(
            ContainedPatchBinaryNucleusAreaContrastError::WorkingByteBudgetExceeded {
                required: working_bytes,
                maximum: maximum_working_bytes,
            },
        );
    }

    let mut edge_indices = Vec::new();
    edge_indices.try_reserve_exact(edge_count).map_err(|_| {
        ContainedPatchBinaryNucleusAreaContrastError::AllocationFailed {
            requested: working_bytes,
        }
    })?;
    edge_indices.extend(0..edge_count);
    edge_indices.sort_unstable_by(|left, right| {
        let left_edge = &link.edges()[*left];
        let right_edge = &link.edges()[*right];
        left_edge
            .patch_id()
            .cmp(right_edge.patch_id())
            .then_with(|| left_edge.assignment_row().cmp(&right_edge.assignment_row()))
            .then_with(|| left.cmp(right))
    });

    let nucleus_areas = input
        .pattern()
        .nucleus_area_um2
        .as_deref()
        .expect("S12 proved the dense nucleus-area column");
    let mut represented_patch_count = 0_u64;
    let mut eligible_patch_count = 0_u64;
    let mut eligible_marked_incidence_count = 0_u64;
    let mut eligible_unmarked_incidence_count = 0_u64;
    let mut total_patch_contrast = 0.0_f64;
    let mut group_start = 0_usize;
    while group_start < edge_indices.len() {
        let group_end = patch_group_end(link, &edge_indices, group_start);
        represented_patch_count = represented_patch_count
            .checked_add(1)
            .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
        let mut marked_count = 0_u64;
        let mut unmarked_count = 0_u64;
        let mut marked_sum = 0.0_f64;
        let mut unmarked_sum = 0.0_f64;
        for &edge_index in &edge_indices[group_start..group_end] {
            let assignment_row = usize::try_from(link.edges()[edge_index].assignment_row())
                .map_err(|_| ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
            let area = *nucleus_areas.get(assignment_row).ok_or(
                ContainedPatchBinaryNucleusAreaContrastError::CellIdBindingMismatch {
                    row: assignment_row,
                },
            )?;
            let binary = *input.pattern().mark.get(assignment_row).ok_or(
                ContainedPatchBinaryNucleusAreaContrastError::CellIdBindingMismatch {
                    row: assignment_row,
                },
            )?;
            if binary == 1 {
                marked_count = marked_count
                    .checked_add(1)
                    .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
                marked_sum += f64::from(area);
            } else {
                unmarked_count = unmarked_count
                    .checked_add(1)
                    .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
                unmarked_sum += f64::from(area);
            }
        }
        if marked_count > 0 && unmarked_count > 0 {
            eligible_patch_count = eligible_patch_count
                .checked_add(1)
                .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
            eligible_marked_incidence_count = eligible_marked_incidence_count
                .checked_add(marked_count)
                .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
            eligible_unmarked_incidence_count = eligible_unmarked_incidence_count
                .checked_add(unmarked_count)
                .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
            total_patch_contrast +=
                marked_sum / marked_count as f64 - unmarked_sum / unmarked_count as f64;
        }
        group_start = group_end;
    }
    let eligible_incidence_count = eligible_marked_incidence_count
        .checked_add(eligible_unmarked_incidence_count)
        .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
    let excluded_incidence_count = edge_count_u64
        .checked_sub(eligible_incidence_count)
        .ok_or(ContainedPatchBinaryNucleusAreaContrastError::SizeOverflow)?;
    let status = if eligible_patch_count == 0 {
        ContainedPatchBinaryNucleusAreaContrastStatus::InsufficientEligiblePatches
    } else {
        ContainedPatchBinaryNucleusAreaContrastStatus::Available
    };
    let equal_patch_mean_marked_minus_unmarked_nucleus_area_um2 =
        (eligible_patch_count > 0).then(|| {
            let value = total_patch_contrast / eligible_patch_count as f64;
            if value == 0.0 {
                0.0
            } else {
                value
            }
        });

    Ok(ContainedPatchBinaryNucleusAreaContrast {
        status,
        whole_input_contrast,
        assignment_count: assignment_count_u64,
        edge_count: edge_count_u64,
        represented_patch_count,
        eligible_patch_count,
        eligible_marked_incidence_count,
        eligible_unmarked_incidence_count,
        eligible_incidence_count,
        excluded_incidence_count,
        equal_patch_mean_marked_minus_unmarked_nucleus_area_um2,
        cell_patch_logical_digest: link.logical_digest(),
        expected_cells_artifact_id: link.expected_cells_artifact_id(),
        expected_patches_artifact_id: link.expected_patches_artifact_id(),
        patch_context_artifact_id: link.patch_context_artifact_id(),
        patch_footprints_artifact_id: link.patch_footprints_artifact_id(),
        cell_patch_producer_artifact_id: link.producer_artifact_id(),
        cell_patch_assignment_artifact_id: receipt.assignment_artifact_id(),
        cell_patch_edge_artifact_id: receipt.edge_artifact_id(),
    })
}

fn patch_group_end(link: &CellPatchLink, edge_indices: &[usize], start: usize) -> usize {
    let patch = link.edges()[edge_indices[start]].patch_id();
    let mut end = start + 1;
    while end < edge_indices.len() && link.edges()[edge_indices[end]].patch_id() == patch {
        end += 1;
    }
    end
}
