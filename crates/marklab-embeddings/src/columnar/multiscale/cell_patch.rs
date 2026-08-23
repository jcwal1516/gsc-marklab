use marklab_project::ArtifactId;

use crate::{
    multiscale::physical::{
        encoding_version, schema_id, SpatialArtifactRole, SpatialPhysicalEncoding,
    },
    CellPatchAssignmentMode, CellPatchAssignmentStatus, CellPatchLink,
};

use super::MultiscaleColumnarError;

pub(crate) const CELL_PATCH_METADATA_KEYS: [&str; 11] = [
    "marklab.assignment_mode",
    "marklab.encoding_version",
    "marklab.expected_cells_artifact_id",
    "marklab.expected_patches_artifact_id",
    "marklab.footprint_artifact_id",
    "marklab.link_logical_digest",
    "marklab.owning_slide_id",
    "marklab.patch_context_artifact_id",
    "marklab.producer_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
];

pub(crate) fn cell_patch_dependencies(link: &CellPatchLink) -> [ArtifactId; 5] {
    let mut dependencies = [
        link.expected_cells_artifact_id(),
        link.expected_patches_artifact_id(),
        link.patch_context_artifact_id(),
        link.patch_footprints_artifact_id(),
        link.producer_artifact_id(),
    ];
    dependencies.sort_unstable();
    dependencies
}

pub(crate) fn cell_patch_metadata(
    role: SpatialArtifactRole,
    encoding: SpatialPhysicalEncoding,
    link: &CellPatchLink,
) -> [(String, String); 11] {
    let values = [
        link.mode().wire_name().to_owned(),
        encoding_version(role, encoding).to_owned(),
        link.expected_cells_artifact_id().to_string(),
        link.expected_patches_artifact_id().to_string(),
        link.patch_footprints_artifact_id().to_string(),
        link.logical_digest().to_string(),
        link.owning_slide_id().as_str().to_owned(),
        link.patch_context_artifact_id().to_string(),
        link.producer_artifact_id().to_string(),
        schema_id(role).to_owned(),
        "1".to_owned(),
    ];
    std::array::from_fn(|index| {
        (
            CELL_PATCH_METADATA_KEYS[index].to_owned(),
            values[index].clone(),
        )
    })
}

pub(crate) fn validate_cell_patch_domain(
    link: &CellPatchLink,
) -> Result<(), MultiscaleColumnarError> {
    let dependencies = cell_patch_dependencies(link);
    if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
        return binding_mismatch();
    }
    let mut edge_prefix = 0_u64;
    for (assignment_index, assignment) in link.assignments().iter().enumerate() {
        if !assignment
            .anchor_px()
            .iter()
            .all(|value| value.is_finite() && (*value != 0.0 || value.to_bits() == 0_f64.to_bits()))
            || assignment.edge_start() != edge_prefix
            || !valid_assignment_state(link.mode(), assignment.status(), assignment.edge_count())
        {
            return binding_mismatch();
        }
        let start =
            usize::try_from(edge_prefix).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
        edge_prefix = edge_prefix
            .checked_add(assignment.edge_count())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let end =
            usize::try_from(edge_prefix).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
        let edges = link
            .edges()
            .get(start..end)
            .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?;
        if edges
            .windows(2)
            .any(|pair| pair[0].patch_id() >= pair[1].patch_id())
            || edges
                .iter()
                .any(|edge| usize::try_from(edge.assignment_row()).ok() != Some(assignment_index))
        {
            return binding_mismatch();
        }
        match link.mode() {
            CellPatchAssignmentMode::ContainedShared => {
                if edges.iter().any(|edge| edge.weight().is_some()) {
                    return binding_mismatch();
                }
            }
            CellPatchAssignmentMode::DeclaredWeightedInterpolation => {
                validate_interpolation_group(edges)?;
            }
        }
    }
    if edge_prefix
        != u64::try_from(link.edge_count()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?
    {
        return binding_mismatch();
    }
    Ok(())
}

fn valid_assignment_state(
    mode: CellPatchAssignmentMode,
    status: CellPatchAssignmentStatus,
    edge_count: u64,
) -> bool {
    matches!(
        (mode, status, edge_count),
        (
            CellPatchAssignmentMode::ContainedShared,
            CellPatchAssignmentStatus::Assigned,
            1..
        ) | (
            CellPatchAssignmentMode::ContainedShared,
            CellPatchAssignmentStatus::OutsideSampledSupport,
            0,
        ) | (
            CellPatchAssignmentMode::DeclaredWeightedInterpolation,
            CellPatchAssignmentStatus::Assigned,
            1..=4,
        ) | (
            CellPatchAssignmentMode::DeclaredWeightedInterpolation,
            CellPatchAssignmentStatus::InterpolationUnavailable,
            0,
        )
    )
}

fn validate_interpolation_group(
    edges: &[crate::CellPatchEdge],
) -> Result<(), MultiscaleColumnarError> {
    if edges.is_empty() {
        return Ok(());
    }
    let first = edges[0]
        .weight()
        .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?;
    let denominator = first.denominator();
    let mut numerator_sum = 0_u64;
    let mut group_gcd = denominator;
    for edge in edges {
        let weight = edge
            .weight()
            .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?;
        if weight.numerator() == 0
            || weight.denominator() == 0
            || weight.denominator() != denominator
        {
            return binding_mismatch();
        }
        numerator_sum = numerator_sum
            .checked_add(weight.numerator())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        group_gcd = gcd(group_gcd, weight.numerator());
    }
    if numerator_sum != denominator || group_gcd != 1 {
        return binding_mismatch();
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

pub(crate) fn assignment_decoded_bytes(
    rows: &[crate::CellPatchAssignment],
) -> Result<usize, MultiscaleColumnarError> {
    let cell_text = rows.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.cell_id().as_str().len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    let status_text = rows.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.status().wire_name().len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    let text_bytes = cell_text
        .checked_add(status_text)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    decoded_bytes(rows.len(), 6, 2, text_bytes, 4)
}

pub(crate) fn edge_decoded_bytes(
    rows: &[crate::CellPatchEdge],
) -> Result<usize, MultiscaleColumnarError> {
    let patch_text = rows.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.patch_id().as_str().len())
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    decoded_bytes(rows.len(), 4, 1, patch_text, 3)
}

fn decoded_bytes(
    rows: usize,
    validity_columns: usize,
    utf8_columns: usize,
    text_bytes: usize,
    u64_columns: usize,
) -> Result<usize, MultiscaleColumnarError> {
    let validity = rows
        .checked_add(7)
        .map(|value| value / 8)
        .and_then(|value| value.checked_mul(validity_columns))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .and_then(|value| value.checked_mul(utf8_columns))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(size_of::<u64>())
        .and_then(|value| value.checked_mul(u64_columns))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    validity
        .checked_add(offsets)
        .and_then(|value| value.checked_add(text_bytes))
        .and_then(|value| value.checked_add(values))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn binding_mismatch<T>() -> Result<T, MultiscaleColumnarError> {
    Err(MultiscaleColumnarError::ArtifactBindingMismatch)
}
