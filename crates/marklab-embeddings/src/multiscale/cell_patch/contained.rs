use std::{cmp::Ordering, mem::size_of};

use super::{
    resources::{increment_edge_count, try_vec_capacity},
    types::{CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentStatus, CellPatchEdge},
};
use crate::multiscale::{
    context::PatchEmbeddingContext, error::MultiscaleEmbeddingError, footprint::PatchFootprintSet,
};

#[derive(Clone, Copy, Eq, PartialEq)]
struct BucketEntry {
    key: [i64; 2],
    patch_index: usize,
}

impl Ord for BucketEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key
            .cmp(&other.key)
            .then_with(|| self.patch_index.cmp(&other.patch_index))
    }
}

impl PartialOrd for BucketEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) struct ContainedPass {
    pub(super) candidate_checks: usize,
    pub(super) edge_count: usize,
    pub(super) edge_text_bytes: usize,
    pub(super) cell_text_bytes: usize,
    pub(super) assignments: Option<Box<[CellPatchAssignment]>>,
    pub(super) edges: Option<Box<[CellPatchEdge]>>,
}

pub(super) fn scratch_bytes(patch_count: usize) -> Result<usize, MultiscaleEmbeddingError> {
    patch_count
        .checked_mul(size_of::<BucketEntry>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

pub(super) fn run(
    anchors: &[CellPatchAnchor],
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    maximum_candidate_checks: usize,
    materialize_edge_count: Option<usize>,
) -> Result<ContainedPass, MultiscaleEmbeddingError> {
    let extent = context.patch_px().map(i64::from);
    let mut bucket_entries = try_vec_capacity::<BucketEntry>(footprints.footprints().len())?;
    for (patch_index, footprint) in footprints.footprints().iter().enumerate() {
        let origin = footprint.origin_px();
        bucket_entries.push(BucketEntry {
            key: [
                origin[1].div_euclid(extent[1]),
                origin[0].div_euclid(extent[0]),
            ],
            patch_index,
        });
    }
    bucket_entries.sort_unstable();

    let mut assignments = match materialize_edge_count {
        Some(_) => Some(try_vec_capacity::<CellPatchAssignment>(anchors.len())?),
        None => None,
    };
    let mut edges = match materialize_edge_count {
        Some(count) => Some(try_vec_capacity::<CellPatchEdge>(count)?),
        None => None,
    };
    let mut candidate_checks = 0_usize;
    let mut edge_count = 0_usize;
    let mut edge_text_bytes = 0_usize;
    let mut cell_text_bytes = 0_usize;

    for (row, anchor) in anchors.iter().enumerate() {
        cell_text_bytes = cell_text_bytes
            .checked_add(anchor.cell_id.as_str().len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let edge_start = edge_count;
        if let Some(point) = floor_anchor(anchor.anchor_px) {
            let bucket = [
                point[1].div_euclid(extent[1]),
                point[0].div_euclid(extent[0]),
            ];
            for dy in -1_i64..=0 {
                for dx in -1_i64..=0 {
                    let Some(y) = bucket[0].checked_add(dy) else {
                        continue;
                    };
                    let Some(x) = bucket[1].checked_add(dx) else {
                        continue;
                    };
                    let key = [y, x];
                    let start = bucket_entries.partition_point(|entry| entry.key < key);
                    let end = bucket_entries.partition_point(|entry| entry.key <= key);
                    for entry in &bucket_entries[start..end] {
                        candidate_checks = candidate_checks
                            .checked_add(1)
                            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
                        if candidate_checks > maximum_candidate_checks {
                            return Err(
                                MultiscaleEmbeddingError::CellPatchCandidateCheckBudgetExceeded {
                                    required: candidate_checks,
                                    maximum: maximum_candidate_checks,
                                },
                            );
                        }
                        let footprint = &footprints.footprints()[entry.patch_index];
                        if !contains_integer_point(footprint.origin_px(), extent, point)? {
                            continue;
                        }
                        edge_count = increment_edge_count(edge_count)?;
                        edge_text_bytes = edge_text_bytes
                            .checked_add(footprint.patch_id().as_str().len())
                            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
                        if let Some(edges) = &mut edges {
                            edges.push(CellPatchEdge {
                                assignment_row: u64::try_from(row)
                                    .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
                                patch_id: footprint.patch_id().clone(),
                                weight: None,
                            });
                        }
                    }
                }
            }
        }
        let group_count = edge_count
            .checked_sub(edge_start)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        if let Some(assignments) = &mut assignments {
            assignments.push(CellPatchAssignment {
                cell_id: anchor.cell_id.clone(),
                status: if group_count == 0 {
                    CellPatchAssignmentStatus::OutsideSampledSupport
                } else {
                    CellPatchAssignmentStatus::Assigned
                },
                anchor_px: anchor.anchor_px,
                edge_start: u64::try_from(edge_start)
                    .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
                edge_count: u64::try_from(group_count)
                    .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
            });
        }
    }

    if materialize_edge_count.is_some_and(|expected| expected != edge_count) {
        return Err(MultiscaleEmbeddingError::SizeOverflow);
    }
    if let Some(edges) = &mut edges {
        edges.sort_unstable_by(|left, right| {
            left.assignment_row
                .cmp(&right.assignment_row)
                .then_with(|| left.patch_id.cmp(&right.patch_id))
        });
        if edges.windows(2).any(|pair| {
            pair[0].assignment_row == pair[1].assignment_row && pair[0].patch_id >= pair[1].patch_id
        }) {
            return Err(MultiscaleEmbeddingError::SizeOverflow);
        }
    }
    Ok(ContainedPass {
        candidate_checks,
        edge_count,
        edge_text_bytes,
        cell_text_bytes,
        assignments: assignments.map(Vec::into_boxed_slice),
        edges: edges.map(Vec::into_boxed_slice),
    })
}

fn floor_anchor(anchor: [f64; 2]) -> Option<[i64; 2]> {
    Some([floor_to_i64(anchor[0])?, floor_to_i64(anchor[1])?])
}

fn floor_to_i64(value: f64) -> Option<i64> {
    const I64_MINIMUM: f64 = -9_223_372_036_854_775_808.0;
    const I64_UPPER_EXCLUSIVE: f64 = 9_223_372_036_854_775_808.0;
    if !(I64_MINIMUM..I64_UPPER_EXCLUSIVE).contains(&value) {
        return None;
    }
    let truncated = value as i64;
    if value < 0.0 && value != truncated as f64 {
        truncated.checked_sub(1)
    } else {
        Some(truncated)
    }
}

fn contains_integer_point(
    origin: [i64; 2],
    extent: [i64; 2],
    point_floor: [i64; 2],
) -> Result<bool, MultiscaleEmbeddingError> {
    for axis in 0..2 {
        let end = origin[axis]
            .checked_add(extent[axis])
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        if point_floor[axis] < origin[axis] || point_floor[axis] >= end {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{contains_integer_point, floor_to_i64};

    #[test]
    fn floor_conversion_preserves_negative_and_i64_boundaries() {
        assert_eq!(floor_to_i64(-0.25), Some(-1));
        assert_eq!(floor_to_i64(-1.0), Some(-1));
        assert_eq!(floor_to_i64(0.999), Some(0));
        assert_eq!(floor_to_i64(-9_223_372_036_854_775_808.0), Some(i64::MIN));
        assert_eq!(floor_to_i64(9_223_372_036_854_775_808.0), None);
    }

    #[test]
    fn containment_compares_large_integer_boundaries_without_lossy_endpoint_casts() {
        let origin = [9_007_199_254_740_993, 0];
        let extent = [224, 224];
        assert_eq!(
            contains_integer_point(origin, extent, [9_007_199_254_740_992, 0]),
            Ok(false)
        );
        assert_eq!(
            contains_integer_point(origin, extent, [9_007_199_254_740_994, 0]),
            Ok(true)
        );
        assert_eq!(
            contains_integer_point(origin, extent, [9_007_199_254_741_217, 0]),
            Ok(false)
        );
    }
}
