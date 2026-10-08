use std::collections::{BTreeMap, HashSet};

use marklab_data::PatchId;
use marklab_project::ContentDigest;
use thiserror::Error;

use super::overlap::PatchOverlapGraph;

/// One positive within-specimen patch aggregation weight.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchDependencyWeight {
    patch_id: PatchId,
    weight: f64,
}

impl PatchDependencyWeight {
    /// Construct one finite positive patch weight.
    pub fn new(patch_id: PatchId, weight: f64) -> Result<Self, PatchDependencyWeightingError> {
        if !weight.is_finite() || weight <= 0.0 {
            return Err(PatchDependencyWeightingError::InvalidWeight);
        }
        Ok(Self { patch_id, weight })
    }

    /// Exact patch identity.
    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    /// Positive unnormalized within-specimen aggregation weight.
    pub fn weight(&self) -> f64 {
        self.weight
    }
}

/// Overlap-component weighting and descriptive effective patch count.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchDependencyWeighting {
    patch_count: u32,
    dependency_group_count: u32,
    effective_independent_patch_count: f64,
    overlap_logical_digest: ContentDigest,
    inferential_unit: &'static str,
}

impl PatchDependencyWeighting {
    /// Number of uniquely weighted patches.
    pub fn patch_count(&self) -> u32 {
        self.patch_count
    }

    /// Number of positive-area overlap components represented.
    pub fn dependency_group_count(&self) -> u32 {
        self.dependency_group_count
    }

    /// Kish effective count after normalized patch weights are aggregated by overlap component.
    pub fn effective_independent_patch_count(&self) -> f64 {
        self.effective_independent_patch_count
    }

    /// Exact overlap-graph identity.
    pub fn overlap_logical_digest(&self) -> ContentDigest {
        self.overlap_logical_digest
    }

    /// Required inferential-unit policy; patch weights remain descriptive within specimens.
    pub fn inferential_unit(&self) -> &'static str {
        self.inferential_unit
    }
}

/// Invalid patch weighting or overlap binding.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PatchDependencyWeightingError {
    /// A weight is zero, negative, NaN, or infinite.
    #[error("patch dependency weight must be finite and positive")]
    InvalidWeight,
    /// A patch appears more than once.
    #[error("patch dependency weights contain a duplicate patch")]
    DuplicatePatch,
    /// A weighted patch is absent from the overlap graph.
    #[error("patch dependency weight references an unknown overlap-graph patch")]
    UnknownPatch,
    /// No patches were supplied.
    #[error("patch dependency weighting requires at least one patch")]
    Empty,
    /// Count or numeric accumulation overflowed.
    #[error("patch dependency weighting overflowed")]
    NumericOverflow,
}

#[derive(Clone, Copy, Default)]
struct StableSum {
    sum: f64,
    correction: f64,
}

impl StableSum {
    fn add(&mut self, value: f64) -> Result<(), PatchDependencyWeightingError> {
        if !value.is_finite() {
            return Err(PatchDependencyWeightingError::NumericOverflow);
        }
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(PatchDependencyWeightingError::NumericOverflow);
        }
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        if !self.correction.is_finite() {
            return Err(PatchDependencyWeightingError::NumericOverflow);
        }
        self.sum = next;
        Ok(())
    }

    fn total(self) -> Result<f64, PatchDependencyWeightingError> {
        let result = self.sum + self.correction;
        if result.is_finite() {
            Ok(result)
        } else {
            Err(PatchDependencyWeightingError::NumericOverflow)
        }
    }
}

/// Aggregate patch weights by exact overlap component and compute a descriptive effective count.
///
/// The returned weights are for within-specimen aggregation only. Patients remain the required
/// inferential units; this function never converts patches into independent replicates.
pub fn patch_dependency_weighting(
    overlap: &PatchOverlapGraph,
    patch_weights: Vec<PatchDependencyWeight>,
) -> Result<PatchDependencyWeighting, PatchDependencyWeightingError> {
    if patch_weights.is_empty() {
        return Err(PatchDependencyWeightingError::Empty);
    }
    let mut seen = HashSet::new();
    let mut total = StableSum::default();
    for patch in &patch_weights {
        if !seen.insert(patch.patch_id.clone()) {
            return Err(PatchDependencyWeightingError::DuplicatePatch);
        }
        if overlap.component_id(&patch.patch_id).is_none() {
            return Err(PatchDependencyWeightingError::UnknownPatch);
        }
        total.add(patch.weight)?;
    }
    let total = total.total()?;
    if total <= 0.0 {
        return Err(PatchDependencyWeightingError::NumericOverflow);
    }
    let mut groups = BTreeMap::<PatchId, StableSum>::new();
    for patch in patch_weights {
        let component = overlap
            .component_id(&patch.patch_id)
            .ok_or(PatchDependencyWeightingError::UnknownPatch)?
            .clone();
        groups
            .entry(component)
            .or_default()
            .add(patch.weight / total)?;
    }
    let mut squared = StableSum::default();
    for weight in groups.values().copied() {
        let weight = weight.total()?;
        squared.add(weight * weight)?;
    }
    let effective = 1.0 / squared.total()?;
    if !effective.is_finite() {
        return Err(PatchDependencyWeightingError::NumericOverflow);
    }
    Ok(PatchDependencyWeighting {
        patch_count: u32::try_from(seen.len())
            .map_err(|_| PatchDependencyWeightingError::NumericOverflow)?,
        dependency_group_count: u32::try_from(groups.len())
            .map_err(|_| PatchDependencyWeightingError::NumericOverflow)?,
        effective_independent_patch_count: effective,
        overlap_logical_digest: overlap.logical_digest(),
        inferential_unit: "patient_not_patch",
    })
}
