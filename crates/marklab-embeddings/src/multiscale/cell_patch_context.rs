use marklab_data::{CellId, PatchId};
use marklab_project::ContentDigest;
use thiserror::Error;

use super::{
    cell_patch::{CellPatchAssignmentMode, CellPatchAssignmentStatus, CellPatchLink},
    overlap::PatchOverlapGraph,
    patch_dependency::{patch_dependency_weighting, PatchDependencyWeight},
    table::PatchEmbeddingTable,
};

/// One overlap-aware weighted patch context for a canonical cell.
#[derive(Clone, Debug, PartialEq)]
pub struct CellPatchContextResult {
    context_vector: Vec<f64>,
    linked_patch_count: u32,
    dependency_group_count: u32,
    effective_independent_patch_count: f64,
    component_operations: u64,
    table_logical_digest: ContentDigest,
    link_logical_digest: ContentDigest,
    overlap_logical_digest: ContentDigest,
    patch_context_logical_digest: ContentDigest,
}

impl CellPatchContextResult {
    /// Weighted patch-context vector in canonical component order.
    pub fn context_vector(&self) -> &[f64] {
        &self.context_vector
    }

    /// Number of exact patch links contributing vectors.
    pub fn linked_patch_count(&self) -> u32 {
        self.linked_patch_count
    }

    /// Number of positive-area overlap components represented by the linked patches.
    pub fn dependency_group_count(&self) -> u32 {
        self.dependency_group_count
    }

    /// Kish effective count after normalized link weights are aggregated by overlap component.
    pub fn effective_independent_patch_count(&self) -> f64 {
        self.effective_independent_patch_count
    }

    /// Checked linked-patch × component operation count.
    pub fn component_operations(&self) -> u64 {
        self.component_operations
    }

    /// Format-independent patch-table identity.
    pub fn table_logical_digest(&self) -> ContentDigest {
        self.table_logical_digest
    }

    /// Exact cell-patch link identity.
    pub fn link_logical_digest(&self) -> ContentDigest {
        self.link_logical_digest
    }

    /// Exact patch-overlap graph identity.
    pub fn overlap_logical_digest(&self) -> ContentDigest {
        self.overlap_logical_digest
    }

    /// Exact patch extraction context/scale identity retained by the link.
    pub fn patch_context_logical_digest(&self) -> ContentDigest {
        self.patch_context_logical_digest
    }
}

/// Invalid artifact binding, missing modality, or bounded-work request.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CellPatchContextError {
    /// Table, link, or overlap identities disagree.
    #[error("cell-patch context table, link, and overlap bindings disagree")]
    BindingMismatch,
    /// The requested cell is not part of the canonical link.
    #[error("cell-patch context requested an unknown canonical cell")]
    UnknownCell,
    /// The requested cell has no linked patch modality at this context/scale.
    #[error("cell-patch context is unavailable for the requested cell")]
    MissingModality,
    /// One linked patch row is not present with a usable vector.
    #[error("cell-patch context linked patch {patch_id} has no present vector")]
    MissingPatchVector {
        /// Exact linked patch lacking a usable vector.
        patch_id: PatchId,
    },
    /// The checked component work exceeds the caller's bound.
    #[error("cell-patch context component operations {required} exceed maximum {maximum}")]
    ComponentOperationBudgetExceeded {
        /// Checked linked-patch × dimension operations.
        required: u64,
        /// Caller-provided maximum.
        maximum: u64,
    },
    /// Count, index, or numeric accumulation overflowed.
    #[error("cell-patch context count or numeric accumulation overflowed")]
    NumericOverflow,
}

#[derive(Clone, Copy, Default)]
struct StableSum {
    sum: f64,
    correction: f64,
}

impl StableSum {
    fn add(&mut self, value: f64) -> Result<(), CellPatchContextError> {
        if !value.is_finite() {
            return Err(CellPatchContextError::NumericOverflow);
        }
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(CellPatchContextError::NumericOverflow);
        }
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        if !self.correction.is_finite() {
            return Err(CellPatchContextError::NumericOverflow);
        }
        self.sum = next;
        Ok(())
    }

    fn total(self) -> Result<f64, CellPatchContextError> {
        let total = self.sum + self.correction;
        if total.is_finite() {
            Ok(total)
        } else {
            Err(CellPatchContextError::NumericOverflow)
        }
    }
}

/// Compute one bounded weighted patch-context vector and overlap-aware effective patch count.
///
/// `CellPatchLink` is the canonical validation owner for cell/patch IDs, context/scale, and link
/// weights. This function additionally requires its patch identities to agree with a materialized
/// table and positive-area overlap graph before reading any vector. Runtime is `O(k log p + k*d)`
/// for `k` linked patches and dimension `d`, with `O(d + k)` result/scratch storage.
pub fn cell_patch_context(
    cell_id: &CellId,
    table: &PatchEmbeddingTable,
    link: &CellPatchLink,
    overlap: &PatchOverlapGraph,
    maximum_component_operations: u64,
) -> Result<CellPatchContextResult, CellPatchContextError> {
    validate_bindings(table, link, overlap)?;
    let assignment_row = link
        .assignments()
        .binary_search_by(|assignment| assignment.cell_id().cmp(cell_id))
        .map_err(|_| CellPatchContextError::UnknownCell)?;
    let assignment = &link.assignments()[assignment_row];
    if assignment.status() != CellPatchAssignmentStatus::Assigned {
        return Err(CellPatchContextError::MissingModality);
    }
    let edges = link
        .edges_for_assignment(assignment_row)
        .map_err(|_| CellPatchContextError::NumericOverflow)?;
    if edges.is_empty() {
        return Err(CellPatchContextError::MissingModality);
    }
    let component_operations = u64::try_from(edges.len())
        .ok()
        .and_then(|count| count.checked_mul(u64::from(table.dimension())))
        .ok_or(CellPatchContextError::NumericOverflow)?;
    if component_operations > maximum_component_operations {
        return Err(CellPatchContextError::ComponentOperationBudgetExceeded {
            required: component_operations,
            maximum: maximum_component_operations,
        });
    }
    let edge_count = edges.len() as f64;
    let dimension =
        usize::try_from(table.dimension()).map_err(|_| CellPatchContextError::NumericOverflow)?;
    let mut context = vec![StableSum::default(); dimension];
    let mut dependency_weights = Vec::with_capacity(edges.len());
    for edge in edges {
        let weight = match (link.mode(), edge.weight()) {
            (CellPatchAssignmentMode::ContainedShared, None) => 1.0 / edge_count,
            (CellPatchAssignmentMode::DeclaredWeightedInterpolation, Some(weight)) => {
                weight.numerator() as f64 / weight.denominator() as f64
            }
            _ => return Err(CellPatchContextError::BindingMismatch),
        };
        let row_index = table
            .row_index(edge.patch_id())
            .ok_or(CellPatchContextError::BindingMismatch)?;
        let vector = table
            .row(row_index)
            .map_err(|_| CellPatchContextError::NumericOverflow)?
            .vector()
            .ok_or_else(|| CellPatchContextError::MissingPatchVector {
                patch_id: edge.patch_id().clone(),
            })?;
        for (sum, value) in context.iter_mut().zip(vector) {
            sum.add(weight * f64::from(*value))?;
        }
        dependency_weights.push(
            PatchDependencyWeight::new(edge.patch_id().clone(), weight)
                .map_err(|_| CellPatchContextError::NumericOverflow)?,
        );
    }
    let context_vector = context
        .into_iter()
        .map(StableSum::total)
        .collect::<Result<Vec<_>, _>>()?;
    let dependency = patch_dependency_weighting(overlap, dependency_weights)
        .map_err(|_| CellPatchContextError::BindingMismatch)?;
    Ok(CellPatchContextResult {
        context_vector,
        linked_patch_count: u32::try_from(edges.len())
            .map_err(|_| CellPatchContextError::NumericOverflow)?,
        dependency_group_count: dependency.dependency_group_count(),
        effective_independent_patch_count: dependency.effective_independent_patch_count(),
        component_operations,
        table_logical_digest: table.logical_digest(),
        link_logical_digest: link.logical_digest(),
        overlap_logical_digest: overlap.logical_digest(),
        patch_context_logical_digest: link.patch_context_logical_digest(),
    })
}

fn validate_bindings(
    table: &PatchEmbeddingTable,
    link: &CellPatchLink,
    overlap: &PatchOverlapGraph,
) -> Result<(), CellPatchContextError> {
    if table.owning_slide_id() != link.owning_slide_id()
        || table.expected_entities_artifact_id() != link.expected_patches_artifact_id()
        || table.expected_entities_logical_digest() != link.expected_patches_logical_digest()
        || overlap.expected_patches_artifact_id() != link.expected_patches_artifact_id()
        || overlap.expected_patches_logical_digest() != link.expected_patches_logical_digest()
        || overlap.patch_footprints_artifact_id() != link.patch_footprints_artifact_id()
        || overlap.patch_footprints_logical_digest() != link.patch_footprints_logical_digest()
    {
        return Err(CellPatchContextError::BindingMismatch);
    }
    Ok(())
}
