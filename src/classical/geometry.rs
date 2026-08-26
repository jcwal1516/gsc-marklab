use std::{collections::BTreeMap, io::Write};

use marklab_workflow::ContentDigest;

use crate::{
    geom::{spatial_index::SpatialIndex2D, window::ObservationWindow2D},
    ClassicalSpatialError, ClassicalSpatialLimits,
};

pub(super) struct PairVisitBudget {
    used: usize,
    maximum: usize,
}

impl PairVisitBudget {
    pub(super) fn new(maximum: usize) -> Self {
        Self { used: 0, maximum }
    }

    pub(super) fn charge(&mut self) -> Result<(), ClassicalSpatialError> {
        self.used = self
            .used
            .checked_add(1)
            .ok_or(ClassicalSpatialError::SizeOverflow)?;
        if self.used > self.maximum {
            return Err(ClassicalSpatialError::PairVisitLimitExceeded {
                observed: self.used,
                maximum: self.maximum,
            });
        }
        Ok(())
    }

    pub(super) fn used(&self) -> usize {
        self.used
    }
}

pub(super) struct SpatialGeometryPlan2D {
    index: SpatialIndex2D,
    boundary_distances: Box<[f64]>,
    logical_digest: ContentDigest,
    estimated_storage_bytes: usize,
}

impl SpatialGeometryPlan2D {
    pub(super) fn new(
        x: &[f64],
        y: &[f64],
        window: &ObservationWindow2D,
        limits: ClassicalSpatialLimits,
    ) -> Result<Self, ClassicalSpatialError> {
        if x.len() != y.len() {
            return Err(ClassicalSpatialError::PatternShapeMismatch);
        }
        if x.len() > limits.maximum_points {
            return Err(ClassicalSpatialError::PointLimitExceeded {
                observed: x.len(),
                maximum: limits.maximum_points,
            });
        }
        let mut coordinate_rows = BTreeMap::new();
        let mut boundary_distances = Vec::new();
        boundary_distances
            .try_reserve_exact(x.len())
            .map_err(|_| ClassicalSpatialError::AllocationFailed)?;
        for (row, (&x_um, &y_um)) in x.iter().zip(y).enumerate() {
            if !x_um.is_finite() || !y_um.is_finite() {
                return Err(ClassicalSpatialError::NonFinitePoint { row });
            }
            let boundary_distance = window.signed_boundary_distance_um(x_um, y_um)?;
            if boundary_distance < 0.0 {
                return Err(ClassicalSpatialError::PointOutsideWindow { row });
            }
            let key = (canonical_bits(x_um), canonical_bits(y_um));
            if let Some(first_row) = coordinate_rows.insert(key, row) {
                return Err(ClassicalSpatialError::DuplicatePoint {
                    first_row,
                    second_row: row,
                });
            }
            boundary_distances.push(boundary_distance);
        }
        let estimated_storage_bytes = SpatialIndex2D::estimated_storage_bytes_for_len(x.len())
            .checked_add(
                x.len()
                    .checked_mul(std::mem::size_of::<f64>())
                    .ok_or(ClassicalSpatialError::SizeOverflow)?,
            )
            .ok_or(ClassicalSpatialError::SizeOverflow)?;
        let index = SpatialIndex2D::new(x, y).map_err(ClassicalSpatialError::geometry)?;
        let logical_digest = geometry_digest(coordinate_rows.keys().copied(), x.len(), window)?;
        Ok(Self {
            index,
            boundary_distances: boundary_distances.into_boxed_slice(),
            logical_digest,
            estimated_storage_bytes,
        })
    }

    pub(super) fn border_counts(
        &self,
        radii_um: &[f64],
        budget: &mut PairVisitBudget,
    ) -> Result<BorderCounts, ClassicalSpatialError> {
        let mut eligible_ends = vec![0_usize; radii_um.len() + 1];
        let mut pair_starts = vec![0_usize; radii_um.len() + 1];
        let mut pair_ends = vec![0_usize; radii_um.len() + 1];
        let maximum_radius =
            *radii_um
                .last()
                .ok_or_else(|| ClassicalSpatialError::InvalidConfig {
                    reason: "radii must not be empty".into(),
                })?;

        for (center, boundary_distance) in self.boundary_distances.iter().copied().enumerate() {
            let eligible_end = radii_um.partition_point(|radius| *radius <= boundary_distance);
            eligible_ends[eligible_end] = eligible_ends[eligible_end]
                .checked_add(1)
                .ok_or(ClassicalSpatialError::SizeOverflow)?;
            let query_radius = maximum_radius.min(boundary_distance);
            if query_radius < radii_um[0] {
                continue;
            }
            let mut visitor_error = None;
            self.index
                .visit_within_radius(center, query_radius, |neighbor| {
                    if visitor_error.is_some() {
                        return;
                    }
                    if let Err(error) = budget.charge() {
                        visitor_error = Some(error);
                        return;
                    }
                    let start = radii_um.partition_point(|radius| *radius < neighbor.distance_um);
                    if start < eligible_end {
                        pair_starts[start] = match pair_starts[start].checked_add(1) {
                            Some(value) => value,
                            None => {
                                visitor_error = Some(ClassicalSpatialError::SizeOverflow);
                                return;
                            }
                        };
                        pair_ends[eligible_end] = match pair_ends[eligible_end].checked_add(1) {
                            Some(value) => value,
                            None => {
                                visitor_error = Some(ClassicalSpatialError::SizeOverflow);
                                return;
                            }
                        };
                    }
                })
                .map_err(ClassicalSpatialError::geometry)?;
            if let Some(error) = visitor_error {
                return Err(error);
            }
        }

        let mut eligible = self.index.len();
        let mut active_pairs = 0_usize;
        let mut eligible_centers = Vec::with_capacity(radii_um.len());
        let mut ordered_pairs = Vec::with_capacity(radii_um.len());
        for index in 0..radii_um.len() {
            eligible = eligible
                .checked_sub(eligible_ends[index])
                .ok_or(ClassicalSpatialError::SizeOverflow)?;
            active_pairs = active_pairs
                .checked_sub(pair_ends[index])
                .and_then(|value| value.checked_add(pair_starts[index]))
                .ok_or(ClassicalSpatialError::SizeOverflow)?;
            eligible_centers.push(eligible);
            ordered_pairs.push(active_pairs);
        }
        Ok(BorderCounts {
            eligible_centers,
            ordered_pairs,
        })
    }

    pub(super) fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    pub(super) fn estimated_storage_bytes(&self) -> usize {
        self.estimated_storage_bytes
    }
}

pub(super) struct BorderCounts {
    pub(super) eligible_centers: Vec<usize>,
    pub(super) ordered_pairs: Vec<usize>,
}

fn geometry_digest(
    coordinates: impl IntoIterator<Item = (u64, u64)>,
    point_count: usize,
    window: &ObservationWindow2D,
) -> Result<ContentDigest, ClassicalSpatialError> {
    let mut writer = ContentDigest::builder();
    write_digest_part(&mut writer, b"marklab-spatial-geometry-plan-2d-v1")?;
    write_digest_part(&mut writer, window.descriptor().logical_digest.as_bytes())?;
    write_digest_part(&mut writer, &(point_count as u128).to_be_bytes())?;
    for (x_bits, y_bits) in coordinates {
        write_digest_part(&mut writer, &x_bits.to_be_bytes())?;
        write_digest_part(&mut writer, &y_bits.to_be_bytes())?;
    }
    Ok(writer.finish().0)
}

fn write_digest_part(
    writer: &mut marklab_workflow::ContentDigestWriter,
    part: &[u8],
) -> Result<(), ClassicalSpatialError> {
    writer
        .write_all(&(part.len() as u128).to_be_bytes())
        .and_then(|()| writer.write_all(part))
        .map_err(|_| ClassicalSpatialError::SizeOverflow)
}

fn canonical_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}
