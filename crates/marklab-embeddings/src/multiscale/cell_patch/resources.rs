use std::mem::size_of;

use super::{
    types::{
        CellPatchAnchor, CellPatchAssignment, CellPatchContributor, CellPatchEdge,
        DeclaredCellPatchAssignment,
    },
    CellPatchLink,
};
use crate::multiscale::error::MultiscaleEmbeddingError;

pub(super) const MAX_ASSIGNMENTS: usize = 100_000_000;
pub(super) const MAX_CELL_PATCH_EDGES: usize = 400_000_000;

pub(super) fn try_vec_capacity<T>(capacity: usize) -> Result<Vec<T>, MultiscaleEmbeddingError> {
    let requested = capacity
        .checked_mul(size_of::<T>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| MultiscaleEmbeddingError::AllocationFailed { requested })?;
    Ok(values)
}

pub(super) fn increment_edge_count(current: usize) -> Result<usize, MultiscaleEmbeddingError> {
    let observed = current
        .checked_add(1)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    if observed > MAX_CELL_PATCH_EDGES {
        return Err(MultiscaleEmbeddingError::CellPatchEdgeCountExceeded {
            observed,
            maximum: MAX_CELL_PATCH_EDGES,
        });
    }
    Ok(observed)
}

pub(super) fn anchor_input_bytes(
    anchors: &Vec<CellPatchAnchor>,
) -> Result<usize, MultiscaleEmbeddingError> {
    anchors.iter().try_fold(
        anchors
            .capacity()
            .checked_mul(size_of::<CellPatchAnchor>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        |total, anchor| {
            total
                .checked_add(anchor.cell_id.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        },
    )
}

pub(super) fn declaration_input_bytes(
    declarations: &Vec<DeclaredCellPatchAssignment>,
) -> Result<usize, MultiscaleEmbeddingError> {
    declarations.iter().try_fold(
        declarations
            .capacity()
            .checked_mul(size_of::<DeclaredCellPatchAssignment>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        |total, declaration| {
            let contributor_storage = declaration
                .contributors
                .capacity()
                .checked_mul(size_of::<CellPatchContributor>())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
            declaration.contributors.iter().try_fold(
                total
                    .checked_add(declaration.anchor.cell_id.as_str().len())
                    .and_then(|value| value.checked_add(contributor_storage))
                    .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
                |nested_total, contributor| {
                    nested_total
                        .checked_add(contributor.patch_id.as_str().len())
                        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
                },
            )
        },
    )
}

pub(super) fn retained_bytes(
    owning_slide_text_bytes: usize,
    assignments: &[CellPatchAssignment],
    edges: &[CellPatchEdge],
) -> Result<usize, MultiscaleEmbeddingError> {
    let assignment_storage = assignments
        .len()
        .checked_mul(size_of::<CellPatchAssignment>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let edge_storage = edges
        .len()
        .checked_mul(size_of::<CellPatchEdge>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let with_cells = assignments.iter().try_fold(
        size_of::<CellPatchLink>()
            .checked_add(assignment_storage)
            .and_then(|value| value.checked_add(edge_storage))
            .and_then(|value| value.checked_add(owning_slide_text_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        |total, assignment| {
            total
                .checked_add(assignment.cell_id.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        },
    )?;
    edges.iter().try_fold(with_cells, |total, edge| {
        total
            .checked_add(edge.patch_id.as_str().len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    })
}

pub(super) fn output_bytes_from_counts(
    owning_slide_text_bytes: usize,
    assignment_count: usize,
    cell_text_bytes: usize,
    edge_count: usize,
    edge_text_bytes: usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    let assignments = assignment_count
        .checked_mul(size_of::<CellPatchAssignment>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let edges = edge_count
        .checked_mul(size_of::<CellPatchEdge>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    size_of::<CellPatchLink>()
        .checked_add(owning_slide_text_bytes)
        .and_then(|value| value.checked_add(assignments))
        .and_then(|value| value.checked_add(cell_text_bytes))
        .and_then(|value| value.checked_add(edges))
        .and_then(|value| value.checked_add(edge_text_bytes))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

pub(super) fn enforce_retained(
    required: usize,
    maximum: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum });
    }
    Ok(())
}

pub(super) fn enforce_working(
    required: usize,
    maximum: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{increment_edge_count, MAX_CELL_PATCH_EDGES};
    use crate::multiscale::error::MultiscaleEmbeddingError;

    #[test]
    fn cell_patch_edge_counter_closes_at_the_version_one_limit() {
        assert_eq!(
            increment_edge_count(MAX_CELL_PATCH_EDGES - 1),
            Ok(MAX_CELL_PATCH_EDGES)
        );
        assert_eq!(
            increment_edge_count(MAX_CELL_PATCH_EDGES),
            Err(MultiscaleEmbeddingError::CellPatchEdgeCountExceeded {
                observed: MAX_CELL_PATCH_EDGES + 1,
                maximum: MAX_CELL_PATCH_EDGES,
            })
        );
    }
}
