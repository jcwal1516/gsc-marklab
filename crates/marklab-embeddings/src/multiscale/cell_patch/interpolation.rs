use super::{
    resources::{increment_edge_count, try_vec_capacity},
    types::{
        CellPatchAssignment, CellPatchAssignmentStatus, CellPatchEdge, CellPatchWeight,
        DeclaredCellPatchAssignment,
    },
};
use crate::multiscale::{error::MultiscaleEmbeddingError, expected::ExpectedPatchSet};

pub(super) struct InterpolationPass {
    pub(super) edge_count: usize,
    pub(super) edge_text_bytes: usize,
    pub(super) cell_text_bytes: usize,
    pub(super) assignments: Option<Box<[CellPatchAssignment]>>,
    pub(super) edges: Option<Box<[CellPatchEdge]>>,
}

pub(super) fn run(
    declarations: &[DeclaredCellPatchAssignment],
    expected_patches: &ExpectedPatchSet,
    materialize_edge_count: Option<usize>,
) -> Result<InterpolationPass, MultiscaleEmbeddingError> {
    let mut assignments = match materialize_edge_count {
        Some(_) => Some(try_vec_capacity::<CellPatchAssignment>(declarations.len())?),
        None => None,
    };
    let mut edges = match materialize_edge_count {
        Some(count) => Some(try_vec_capacity::<CellPatchEdge>(count)?),
        None => None,
    };
    let mut edge_count = 0_usize;
    let mut edge_text_bytes = 0_usize;
    let mut cell_text_bytes = 0_usize;

    for (row, declaration) in declarations.iter().enumerate() {
        cell_text_bytes = cell_text_bytes
            .checked_add(declaration.anchor.cell_id.as_str().len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        validate_group(row, declaration, expected_patches)?;
        let edge_start = edge_count;
        for contributor in &declaration.contributors {
            edge_count = increment_edge_count(edge_count)?;
            edge_text_bytes = edge_text_bytes
                .checked_add(contributor.patch_id.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
            if let Some(edges) = &mut edges {
                edges.push(CellPatchEdge {
                    assignment_row: u64::try_from(row)
                        .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
                    patch_id: contributor.patch_id.clone(),
                    weight: Some(CellPatchWeight::new(
                        contributor.numerator,
                        contributor.denominator,
                    )),
                });
            }
        }
        if let Some(assignments) = &mut assignments {
            assignments.push(CellPatchAssignment {
                cell_id: declaration.anchor.cell_id.clone(),
                status: if declaration.contributors.is_empty() {
                    CellPatchAssignmentStatus::InterpolationUnavailable
                } else {
                    CellPatchAssignmentStatus::Assigned
                },
                anchor_px: declaration.anchor.anchor_px,
                edge_start: u64::try_from(edge_start)
                    .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
                edge_count: u64::try_from(declaration.contributors.len())
                    .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
            });
        }
    }
    if materialize_edge_count.is_some_and(|expected| expected != edge_count) {
        return Err(MultiscaleEmbeddingError::SizeOverflow);
    }
    Ok(InterpolationPass {
        edge_count,
        edge_text_bytes,
        cell_text_bytes,
        assignments: assignments.map(Vec::into_boxed_slice),
        edges: edges.map(Vec::into_boxed_slice),
    })
}

fn validate_group(
    row: usize,
    declaration: &DeclaredCellPatchAssignment,
    expected_patches: &ExpectedPatchSet,
) -> Result<(), MultiscaleEmbeddingError> {
    let contributors = &declaration.contributors;
    if contributors.len() > 4
        || contributors
            .windows(2)
            .any(|pair| pair[0].patch_id >= pair[1].patch_id)
        || contributors.iter().any(|contributor| {
            expected_patches
                .ids()
                .binary_search(&contributor.patch_id)
                .is_err()
        })
    {
        return Err(MultiscaleEmbeddingError::InvalidCellPatchContributors { row });
    }
    let Some(first) = contributors.first() else {
        return Ok(());
    };
    let denominator = first.denominator;
    if denominator == 0 {
        return Err(MultiscaleEmbeddingError::InvalidCellPatchContributors { row });
    }
    let mut sum = 0_u64;
    let mut divisor = denominator;
    for contributor in contributors {
        if contributor.numerator == 0 || contributor.denominator != denominator {
            return Err(MultiscaleEmbeddingError::InvalidCellPatchContributors { row });
        }
        sum = sum
            .checked_add(contributor.numerator)
            .ok_or(MultiscaleEmbeddingError::InvalidCellPatchContributors { row })?;
        divisor = gcd(divisor, contributor.numerator);
    }
    if sum != denominator || divisor != 1 {
        return Err(MultiscaleEmbeddingError::InvalidCellPatchContributors { row });
    }
    Ok(())
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
