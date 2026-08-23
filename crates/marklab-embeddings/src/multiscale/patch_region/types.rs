use std::fmt;

use marklab_data::{PatchId, RegionId};
use marklab_project::{ArtifactId, ContentDigest};

use crate::multiscale::error::MultiscaleEmbeddingError;

/// Closed producer-declared nonzero patch-region relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchRegionRelation {
    /// Producer declares the full patch footprint contained by the region.
    FullyContained,
    /// Producer declares a strict positive fraction of patch area overlapping the region.
    PartialOverlap,
}

impl PatchRegionRelation {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::FullyContained => "fully_contained",
            Self::PartialOverlap => "partial_overlap",
        }
    }
}

/// One canonical nonzero producer declaration for a patch-region pair.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchRegionDeclaration {
    pub(super) patch_id: PatchId,
    pub(super) region_id: RegionId,
    pub(super) relation: PatchRegionRelation,
    pub(super) numerator: u64,
    pub(super) denominator: u64,
}

impl fmt::Debug for PatchRegionDeclaration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchRegionDeclaration")
            .field("relation", &self.relation)
            .finish()
    }
}

impl PatchRegionDeclaration {
    /// Declare exact `1/1` full containment supplied by the producer.
    pub fn fully_contained(patch_id: PatchId, region_id: RegionId) -> Self {
        Self {
            patch_id,
            region_id,
            relation: PatchRegionRelation::FullyContained,
            numerator: 1,
            denominator: 1,
        }
    }

    /// Declare one reduced strict fraction between zero and one.
    pub fn partial_overlap(
        patch_id: PatchId,
        region_id: RegionId,
        numerator: u64,
        denominator: u64,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if numerator == 0
            || denominator == 0
            || numerator >= denominator
            || gcd(numerator, denominator) != 1
        {
            return Err(MultiscaleEmbeddingError::InvalidPatchRegionFraction);
        }
        Ok(Self {
            patch_id,
            region_id,
            relation: PatchRegionRelation::PartialOverlap,
            numerator,
            denominator,
        })
    }

    /// Declared patch identity.
    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    /// Declared region identity.
    pub fn region_id(&self) -> &RegionId {
        &self.region_id
    }

    /// Closed relation category.
    pub fn relation(&self) -> PatchRegionRelation {
        self.relation
    }

    /// Exact positive overlap numerator.
    pub fn numerator(&self) -> u64 {
        self.numerator
    }

    /// Exact positive overlap denominator.
    pub fn denominator(&self) -> u64 {
        self.denominator
    }
}

/// Exact artifact bindings for one exhaustive declared assessment.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchRegionAssessmentBindings {
    pub(super) expected_regions_artifact_id: ArtifactId,
    pub(super) patch_footprints_artifact_id: ArtifactId,
    pub(super) converter_artifact_id: ArtifactId,
    pub(super) converter_content_digest: ContentDigest,
}

impl fmt::Debug for PatchRegionAssessmentBindings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchRegionAssessmentBindings")
            .finish_non_exhaustive()
    }
}

impl PatchRegionAssessmentBindings {
    /// Bind expected regions, exact footprints, and converter content.
    pub fn new(
        expected_regions_artifact_id: ArtifactId,
        patch_footprints_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        converter_content_digest: ContentDigest,
    ) -> Self {
        Self {
            expected_regions_artifact_id,
            patch_footprints_artifact_id,
            converter_artifact_id,
            converter_content_digest,
        }
    }

    /// Expected-region artifact identity.
    pub fn expected_regions_artifact_id(&self) -> ArtifactId {
        self.expected_regions_artifact_id
    }

    /// Exact patch-footprint artifact identity.
    pub fn patch_footprints_artifact_id(&self) -> ArtifactId {
        self.patch_footprints_artifact_id
    }

    /// Converter artifact identity.
    pub fn converter_artifact_id(&self) -> ArtifactId {
        self.converter_artifact_id
    }

    /// Converter exact content digest.
    pub fn converter_content_digest(&self) -> ContentDigest {
        self.converter_content_digest
    }
}

pub(super) fn valid_relation(row: &PatchRegionDeclaration) -> bool {
    match row.relation {
        PatchRegionRelation::FullyContained => row.numerator == 1 && row.denominator == 1,
        PatchRegionRelation::PartialOverlap => {
            row.numerator > 0
                && row.numerator < row.denominator
                && gcd(row.numerator, row.denominator) == 1
        }
    }
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
