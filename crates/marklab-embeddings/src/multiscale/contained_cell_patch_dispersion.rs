use std::mem::size_of;

use marklab_project::{ArtifactId, ContentDigest};
use thiserror::Error;

use crate::{CellEmbeddingArtifact, CellEmbeddingTable, EmbeddingQcSummary, EmbeddingStatus};

use super::{
    CellPatchAssignmentMode, CellPatchLink, VerifiedCellPatchInputArtifactGraph,
    VerifiedCellPatchLinkArtifact,
};

/// Availability of contained-patch cell-embedding local dispersion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainedCellPatchEmbeddingDispersionStatus {
    /// At least one represented patch has two present cell-embedding incidences.
    Available,
    /// No represented patch has two present cell-embedding incidences.
    InsufficientEligiblePatches,
}

/// Descriptive cell-embedding dispersion within exact contained-patch incidences.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContainedCellPatchEmbeddingDispersion {
    status: ContainedCellPatchEmbeddingDispersionStatus,
    assignment_count: u64,
    edge_count: u64,
    represented_patch_count: u64,
    eligible_patch_count: u64,
    eligible_incidence_count: u64,
    excluded_incidence_count: u64,
    dimension: u32,
    mean_squared_component_dispersion: Option<f64>,
    embedding_qc_summary: EmbeddingQcSummary,
    embedding_table_logical_digest: ContentDigest,
    expected_cells_artifact_id: ArtifactId,
    expected_cells_logical_digest: ContentDigest,
    embedding_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    embedding_provenance_artifact_id: ArtifactId,
    cell_patch_logical_digest: ContentDigest,
    cell_patch_assignment_artifact_id: ArtifactId,
    cell_patch_edge_artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    patch_context_artifact_id: ArtifactId,
    patch_footprints_artifact_id: ArtifactId,
    cell_patch_producer_artifact_id: ArtifactId,
}

impl ContainedCellPatchEmbeddingDispersion {
    /// Whether a numeric dispersion is available and, if not, why.
    pub fn status(self) -> ContainedCellPatchEmbeddingDispersionStatus {
        self.status
    }

    /// Exact expected-cell assignment-row count.
    pub fn assignment_count(self) -> u64 {
        self.assignment_count
    }

    /// Exact cell-patch incidence count.
    pub fn edge_count(self) -> u64 {
        self.edge_count
    }

    /// Number of distinct patches represented by at least one incidence.
    pub fn represented_patch_count(self) -> u64 {
        self.represented_patch_count
    }

    /// Number of represented patches with at least two present incidences.
    pub fn eligible_patch_count(self) -> u64 {
        self.eligible_patch_count
    }

    /// Present incidences belonging to eligible patches.
    pub fn eligible_incidence_count(self) -> u64 {
        self.eligible_incidence_count
    }

    /// Incidences excluded by non-present vectors or under-supported patches.
    pub fn excluded_incidence_count(self) -> u64 {
        self.excluded_incidence_count
    }

    /// Exact positive cell-embedding dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Incidence-weighted mean squared component deviation, when available.
    pub fn mean_squared_component_dispersion(self) -> Option<f64> {
        self.mean_squared_component_dispersion
    }

    /// Factual status, shape, zero-vector, and logical embedding summary.
    pub fn embedding_qc_summary(self) -> EmbeddingQcSummary {
        self.embedding_qc_summary
    }

    /// Format-independent identity of the exact cell-embedding table.
    pub fn embedding_table_logical_digest(self) -> ContentDigest {
        self.embedding_table_logical_digest
    }

    /// Expected-cell artifact identity shared by both joined inputs.
    pub fn expected_cells_artifact_id(self) -> ArtifactId {
        self.expected_cells_artifact_id
    }

    /// Format-independent expected-cell identity and order.
    pub fn expected_cells_logical_digest(self) -> ContentDigest {
        self.expected_cells_logical_digest
    }

    /// Exact verified physical cell-embedding table identity.
    pub fn embedding_artifact_id(self) -> ArtifactId {
        self.embedding_artifact_id
    }

    /// Exact verified physical cell-embedding row-link identity.
    pub fn row_link_artifact_id(self) -> ArtifactId {
        self.row_link_artifact_id
    }

    /// Exact cell-embedding model/provenance identity.
    pub fn embedding_provenance_artifact_id(self) -> ArtifactId {
        self.embedding_provenance_artifact_id
    }

    /// Format-independent identity of the exact contained cell-patch link.
    pub fn cell_patch_logical_digest(self) -> ContentDigest {
        self.cell_patch_logical_digest
    }

    /// Exact verified physical assignment-table identity.
    pub fn cell_patch_assignment_artifact_id(self) -> ArtifactId {
        self.cell_patch_assignment_artifact_id
    }

    /// Exact verified physical edge-table identity.
    pub fn cell_patch_edge_artifact_id(self) -> ArtifactId {
        self.cell_patch_edge_artifact_id
    }

    /// Exact expected-patch artifact identity bound by the link.
    pub fn expected_patches_artifact_id(self) -> ArtifactId {
        self.expected_patches_artifact_id
    }

    /// Exact calibrated patch-context artifact identity bound by the link.
    pub fn patch_context_artifact_id(self) -> ArtifactId {
        self.patch_context_artifact_id
    }

    /// Exact patch-footprint artifact identity bound by the link.
    pub fn patch_footprints_artifact_id(self) -> ArtifactId {
        self.patch_footprints_artifact_id
    }

    /// Exact contained-link producer artifact identity.
    pub fn cell_patch_producer_artifact_id(self) -> ArtifactId {
        self.cell_patch_producer_artifact_id
    }
}

/// Invalid joined input binding, unsupported mode, caller limit, or allocation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ContainedCellPatchEmbeddingDispersionError {
    /// Declared weighted interpolation is not an admitted local-dispersion weight.
    #[error("contained cell-patch embedding dispersion requires contained-shared assignments")]
    UnsupportedAssignmentMode,
    /// The materialized embedding table and its verified artifact disagree.
    #[error("contained cell-patch embedding table and verified artifact disagree")]
    EmbeddingArtifactBindingMismatch,
    /// The embedding artifact and cell-patch link name different expected cells.
    #[error("contained cell-patch embedding expected-cell bindings disagree")]
    ExpectedCellBindingMismatch,
    /// One ordered embedding row and cell-patch assignment name different cells.
    #[error("contained cell-patch embedding cell identity disagrees at row {row}")]
    CellIdBindingMismatch {
        /// First mismatched or absent assignment row.
        row: usize,
    },
    /// The verified managed input graph does not describe the live link.
    #[error("contained cell-patch verified input graph disagrees with the live link")]
    CellPatchGraphBindingMismatch,
    /// The paired physical assignment/edge receipt does not describe the live link.
    #[error("contained cell-patch physical receipt disagrees with the live link")]
    CellPatchReceiptBindingMismatch,
    /// Assignment traversal exceeds the caller maximum.
    #[error("contained cell-patch assignments {required} exceed caller maximum {maximum}")]
    AssignmentCountBudgetExceeded {
        /// Exact expected-cell assignment count.
        required: usize,
        /// Caller-provided assignment maximum.
        maximum: usize,
    },
    /// Edge traversal exceeds the caller maximum.
    #[error("contained cell-patch edges {required} exceed caller maximum {maximum}")]
    EdgeCountBudgetExceeded {
        /// Exact incidence count.
        required: usize,
        /// Caller-provided edge maximum.
        maximum: usize,
    },
    /// Conservative component work exceeds the caller maximum.
    #[error(
        "contained cell-patch component operations {required} exceed caller maximum {maximum}"
    )]
    ComponentOperationBudgetExceeded {
        /// Conservative three-pass edge/component count.
        required: u64,
        /// Caller-provided component-operation maximum.
        maximum: u64,
    },
    /// Exact edge-index and centroid storage exceeds the caller maximum.
    #[error("contained cell-patch working bytes {required} exceed caller maximum {maximum}")]
    WorkingByteBudgetExceeded {
        /// Exact incremental working bytes.
        required: usize,
        /// Caller-provided working-byte maximum.
        maximum: usize,
    },
    /// A checked count or byte computation overflowed.
    #[error("contained cell-patch dispersion size computation overflowed")]
    SizeOverflow,
    /// One exact bounded working allocation failed.
    #[error("contained cell-patch dispersion allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Exact admitted total working bytes.
        requested: usize,
    },
}

/// Measure cell-embedding dispersion within exact producer-declared contained patches.
///
/// The computation joins one verified C-04 cell table to a verified C-05 contained-shared link,
/// sorts incidences by patch and assignment, and reports the incidence-weighted mean squared
/// component deviation from patch-local present-cell centroids. Overlapping patches repeat exact
/// incidences and are not independent replicates. Runtime is `O(E log E + E*D)` with one
/// `E`-element edge-index vector and one reusable `f64[D]` centroid.
///
/// # Errors
///
/// Returns a typed category for unsupported interpolation, embedding/expected-cell/CellId/graph/
/// receipt drift, insufficient caller limits, checked size overflow, or bounded allocation failure.
#[allow(clippy::too_many_arguments)]
pub fn contained_cell_patch_embedding_dispersion(
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    receipt: VerifiedCellPatchLinkArtifact,
    maximum_assignments: usize,
    maximum_edges: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> Result<ContainedCellPatchEmbeddingDispersion, ContainedCellPatchEmbeddingDispersionError> {
    let embedding_qc_summary = table.qc_summary();
    let assignment_count = link.assignment_count();
    let edge_count = link.edge_count();
    if embedding_qc_summary != artifact.qc_summary()
        || embedding_qc_summary.logical_digest() != artifact.logical_digest()
        || artifact.row_count()
            != u64::try_from(table.row_count())
                .map_err(|_| ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?
    {
        return Err(ContainedCellPatchEmbeddingDispersionError::EmbeddingArtifactBindingMismatch);
    }
    if link.mode() != CellPatchAssignmentMode::ContainedShared {
        return Err(ContainedCellPatchEmbeddingDispersionError::UnsupportedAssignmentMode);
    }
    if artifact.expected_cells_artifact_id() != link.expected_cells_artifact_id()
        || artifact.expected_cells_logical_digest() != link.expected_cells_logical_digest()
    {
        return Err(ContainedCellPatchEmbeddingDispersionError::ExpectedCellBindingMismatch);
    }
    if table.row_count() != assignment_count {
        return Err(
            ContainedCellPatchEmbeddingDispersionError::CellIdBindingMismatch {
                row: table.row_count().min(assignment_count),
            },
        );
    }
    for row in 0..assignment_count {
        let embedding_row = table.row(row).map_err(|_| {
            ContainedCellPatchEmbeddingDispersionError::CellIdBindingMismatch { row }
        })?;
        if embedding_row.cell_id() != link.assignments()[row].cell_id() {
            return Err(ContainedCellPatchEmbeddingDispersionError::CellIdBindingMismatch { row });
        }
    }
    let assignment_count_u64 = u64::try_from(assignment_count)
        .map_err(|_| ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
    let edge_count_u64 = u64::try_from(edge_count)
        .map_err(|_| ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
    if graph.mode != link.mode()
        || graph.expected_cells_artifact_id != link.expected_cells_artifact_id()
        || graph.expected_patches_artifact_id != link.expected_patches_artifact_id()
        || graph.patch_context_artifact_id != link.patch_context_artifact_id()
        || graph.patch_footprints_artifact_id != link.patch_footprints_artifact_id()
        || graph.producer_artifact_id != link.producer_artifact_id()
        || graph.link_logical_digest != link.logical_digest()
        || graph.assignment_count != assignment_count_u64
        || graph.edge_count != edge_count_u64
    {
        return Err(ContainedCellPatchEmbeddingDispersionError::CellPatchGraphBindingMismatch);
    }
    if receipt.logical_digest() != link.logical_digest()
        || receipt.assignment_count() != assignment_count_u64
        || receipt.edge_count() != edge_count_u64
    {
        return Err(ContainedCellPatchEmbeddingDispersionError::CellPatchReceiptBindingMismatch);
    }
    if assignment_count > maximum_assignments {
        return Err(
            ContainedCellPatchEmbeddingDispersionError::AssignmentCountBudgetExceeded {
                required: assignment_count,
                maximum: maximum_assignments,
            },
        );
    }
    if edge_count > maximum_edges {
        return Err(
            ContainedCellPatchEmbeddingDispersionError::EdgeCountBudgetExceeded {
                required: edge_count,
                maximum: maximum_edges,
            },
        );
    }

    let dimension = embedding_qc_summary.dimension();
    let component_operations = required_component_operations(edge_count_u64, dimension)?;
    if component_operations > maximum_component_operations {
        return Err(
            ContainedCellPatchEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: component_operations,
                maximum: maximum_component_operations,
            },
        );
    }
    let dimension_usize = usize::try_from(dimension)
        .map_err(|_| ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
    let working_bytes = required_working_bytes(edge_count, dimension_usize)?;
    if working_bytes > maximum_working_bytes {
        return Err(
            ContainedCellPatchEmbeddingDispersionError::WorkingByteBudgetExceeded {
                required: working_bytes,
                maximum: maximum_working_bytes,
            },
        );
    }

    let mut edge_indices = Vec::new();
    edge_indices.try_reserve_exact(edge_count).map_err(|_| {
        ContainedCellPatchEmbeddingDispersionError::AllocationFailed {
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

    let mut represented_patch_count = 0_u64;
    let mut eligible_patch_count = 0_u64;
    let mut eligible_incidence_count = 0_u64;
    let mut group_start = 0_usize;
    while group_start < edge_indices.len() {
        let group_end = patch_group_end(link, &edge_indices, group_start);
        represented_patch_count = represented_patch_count
            .checked_add(1)
            .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
        let present_count =
            present_incidence_count(table, link, &edge_indices[group_start..group_end])?;
        if present_count >= 2 {
            eligible_patch_count = eligible_patch_count
                .checked_add(1)
                .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
            eligible_incidence_count = eligible_incidence_count
                .checked_add(present_count)
                .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
        }
        group_start = group_end;
    }
    let excluded_incidence_count = edge_count_u64
        .checked_sub(eligible_incidence_count)
        .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
    let status = if eligible_patch_count == 0 {
        ContainedCellPatchEmbeddingDispersionStatus::InsufficientEligiblePatches
    } else {
        ContainedCellPatchEmbeddingDispersionStatus::Available
    };

    let mean_squared_component_dispersion = if status
        == ContainedCellPatchEmbeddingDispersionStatus::Available
    {
        let mut centroid = zeroed_centroid(dimension_usize, working_bytes)?;
        let mut total_squared_deviation = 0.0_f64;
        let mut arithmetic_incidence_count = 0_u64;
        let mut group_start = 0_usize;
        while group_start < edge_indices.len() {
            let group_end = patch_group_end(link, &edge_indices, group_start);
            centroid.fill(0.0);
            let mut present_count = 0_u64;
            for &edge_index in &edge_indices[group_start..group_end] {
                let vector = incidence_vector(table, link, edge_index)?;
                let Some(vector) = vector else { continue };
                present_count = present_count
                    .checked_add(1)
                    .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
                for (sum, &component) in centroid.iter_mut().zip(vector) {
                    *sum += f64::from(component);
                }
            }
            if present_count >= 2 {
                let denominator = present_count as f64;
                for mean in &mut centroid {
                    *mean /= denominator;
                }
                for &edge_index in &edge_indices[group_start..group_end] {
                    let vector = incidence_vector(table, link, edge_index)?;
                    let Some(vector) = vector else { continue };
                    arithmetic_incidence_count = arithmetic_incidence_count
                        .checked_add(1)
                        .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
                    for (&component, &mean) in vector.iter().zip(&centroid) {
                        let difference = f64::from(component) - mean;
                        total_squared_deviation += difference * difference;
                    }
                }
            }
            group_start = group_end;
        }
        if arithmetic_incidence_count != eligible_incidence_count {
            return Err(ContainedCellPatchEmbeddingDispersionError::CellPatchGraphBindingMismatch);
        }
        let denominator = eligible_incidence_count as f64 * f64::from(dimension);
        let dispersion = total_squared_deviation / denominator;
        Some(if dispersion == 0.0 { 0.0 } else { dispersion })
    } else {
        None
    };

    Ok(ContainedCellPatchEmbeddingDispersion {
        status,
        assignment_count: assignment_count_u64,
        edge_count: edge_count_u64,
        represented_patch_count,
        eligible_patch_count,
        eligible_incidence_count,
        excluded_incidence_count,
        dimension,
        mean_squared_component_dispersion,
        embedding_qc_summary,
        embedding_table_logical_digest: artifact.logical_digest(),
        expected_cells_artifact_id: artifact.expected_cells_artifact_id(),
        expected_cells_logical_digest: artifact.expected_cells_logical_digest(),
        embedding_artifact_id: artifact.embedding_artifact_id(),
        row_link_artifact_id: artifact.row_link_artifact_id(),
        embedding_provenance_artifact_id: artifact.provenance_artifact_id(),
        cell_patch_logical_digest: link.logical_digest(),
        cell_patch_assignment_artifact_id: receipt.assignment_artifact_id(),
        cell_patch_edge_artifact_id: receipt.edge_artifact_id(),
        expected_patches_artifact_id: link.expected_patches_artifact_id(),
        patch_context_artifact_id: link.patch_context_artifact_id(),
        patch_footprints_artifact_id: link.patch_footprints_artifact_id(),
        cell_patch_producer_artifact_id: link.producer_artifact_id(),
    })
}

fn patch_group_end(link: &CellPatchLink, edge_indices: &[usize], start: usize) -> usize {
    let patch_id = link.edges()[edge_indices[start]].patch_id();
    edge_indices[start + 1..]
        .iter()
        .position(|&edge_index| link.edges()[edge_index].patch_id() != patch_id)
        .map_or(edge_indices.len(), |offset| start + 1 + offset)
}

fn present_incidence_count(
    table: &CellEmbeddingTable,
    link: &CellPatchLink,
    edge_indices: &[usize],
) -> Result<u64, ContainedCellPatchEmbeddingDispersionError> {
    edge_indices.iter().try_fold(0_u64, |count, &edge_index| {
        let present = incidence_vector(table, link, edge_index)?.is_some();
        count
            .checked_add(u64::from(present))
            .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)
    })
}

fn incidence_vector<'a>(
    table: &'a CellEmbeddingTable,
    link: &CellPatchLink,
    edge_index: usize,
) -> Result<Option<&'a [f32]>, ContainedCellPatchEmbeddingDispersionError> {
    let edge = link
        .edges()
        .get(edge_index)
        .ok_or(ContainedCellPatchEmbeddingDispersionError::CellPatchGraphBindingMismatch)?;
    let assignment_row = usize::try_from(edge.assignment_row())
        .map_err(|_| ContainedCellPatchEmbeddingDispersionError::SizeOverflow)?;
    let row = table.row(assignment_row).map_err(|_| {
        ContainedCellPatchEmbeddingDispersionError::CellIdBindingMismatch {
            row: assignment_row,
        }
    })?;
    if row.status() == EmbeddingStatus::Present {
        row.vector()
            .map(Some)
            .ok_or(ContainedCellPatchEmbeddingDispersionError::EmbeddingArtifactBindingMismatch)
    } else {
        Ok(None)
    }
}

fn required_component_operations(
    edge_count: u64,
    dimension: u32,
) -> Result<u64, ContainedCellPatchEmbeddingDispersionError> {
    edge_count
        .checked_mul(u64::from(dimension))
        .and_then(|value| value.checked_mul(3))
        .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)
}

fn required_working_bytes(
    edge_count: usize,
    dimension: usize,
) -> Result<usize, ContainedCellPatchEmbeddingDispersionError> {
    edge_count
        .checked_mul(size_of::<usize>())
        .and_then(|edge_bytes| {
            dimension
                .checked_mul(size_of::<f64>())
                .and_then(|centroid_bytes| edge_bytes.checked_add(centroid_bytes))
        })
        .ok_or(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)
}

fn zeroed_centroid(
    dimension: usize,
    requested: usize,
) -> Result<Vec<f64>, ContainedCellPatchEmbeddingDispersionError> {
    let mut centroid = Vec::new();
    centroid
        .try_reserve_exact(dimension)
        .map_err(|_| ContainedCellPatchEmbeddingDispersionError::AllocationFailed { requested })?;
    centroid.resize(dimension, 0.0);
    Ok(centroid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_resource_helpers_expose_overflow_and_allocation_failure() {
        assert_eq!(
            required_component_operations(u64::MAX, u32::MAX),
            Err(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)
        );
        assert_eq!(
            required_working_bytes(usize::MAX, usize::MAX),
            Err(ContainedCellPatchEmbeddingDispersionError::SizeOverflow)
        );
        assert_eq!(
            zeroed_centroid(usize::MAX, usize::MAX),
            Err(
                ContainedCellPatchEmbeddingDispersionError::AllocationFailed {
                    requested: usize::MAX,
                }
            )
        );
    }
}
