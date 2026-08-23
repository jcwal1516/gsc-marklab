use std::{fmt, mem::size_of};

use marklab_data::{CohortHierarchy, HierarchyId, HierarchyKind, PatchId};
use marklab_project::{ArtifactId, ContentDigest};

use crate::PatchBoundaryPolicy;

use super::{
    context::PatchEmbeddingContext, digest::LogicalDigest, error::MultiscaleEmbeddingError,
    expected::ExpectedPatchSet,
};

const FOOTPRINT_DOMAIN: &[u8] = b"marklab-patch-footprint-set-logical-v1";

/// One typed patch origin in the context's exact half-open image frame.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchFootprint {
    patch_id: PatchId,
    origin_px: [i64; 2],
}

impl fmt::Debug for PatchFootprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchFootprint")
            .finish_non_exhaustive()
    }
}

impl PatchFootprint {
    /// Declare one patch origin; set construction validates context and boundary semantics.
    pub fn new(patch_id: PatchId, origin_px: [i64; 2]) -> Self {
        Self {
            patch_id,
            origin_px,
        }
    }

    /// Typed patch identity.
    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    /// Signed X/Y origin in image pixels.
    pub fn origin_px(&self) -> [i64; 2] {
        self.origin_px
    }
}

/// Canonical once-per-patch sampled-support footprint set.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchFootprintSet {
    footprints: Box<[PatchFootprint]>,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    patch_context_artifact_id: ArtifactId,
    patch_context_logical_digest: ContentDigest,
    logical_digest: ContentDigest,
}

impl fmt::Debug for PatchFootprintSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchFootprintSet")
            .field("row_count", &self.footprints.len())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchFootprintSet {
    /// Validate exact expected order, slide ancestry, half-open boundaries, and retained budget.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hierarchy: &CohortHierarchy,
        expected: &ExpectedPatchSet,
        expected_patches_artifact_id: ArtifactId,
        context: &PatchEmbeddingContext,
        patch_context_artifact_id: ArtifactId,
        footprints: Vec<PatchFootprint>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if expected.owning_slide_id() != context.owning_slide_id()
            || footprints.len() != expected.ids().len()
            || footprints
                .iter()
                .zip(expected.ids())
                .any(|(footprint, expected_id)| footprint.patch_id() != expected_id)
        {
            return Err(MultiscaleEmbeddingError::FootprintSetMismatch);
        }
        validate_hierarchy(hierarchy, expected, &footprints)?;
        for footprint in &footprints {
            validate_boundary(context, footprint.origin_px)?;
        }
        let required = retained_bytes(&footprints)?;
        if required > maximum_retained_bytes {
            return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
                required,
                maximum: maximum_retained_bytes,
            });
        }
        let expected_patches_logical_digest = expected.logical_digest();
        let patch_context_logical_digest = context.logical_digest();
        let logical_digest = footprint_digest(
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            patch_context_artifact_id,
            patch_context_logical_digest,
            &footprints,
        )?;
        Ok(Self {
            footprints: footprints.into_boxed_slice(),
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            patch_context_artifact_id,
            patch_context_logical_digest,
            logical_digest,
        })
    }

    /// Number of exact expected patch footprints.
    pub fn row_count(&self) -> usize {
        self.footprints.len()
    }

    /// Canonical footprint rows in expected-patch order.
    pub fn footprints(&self) -> &[PatchFootprint] {
        &self.footprints
    }

    /// Domain-separated format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    /// Expected-patch artifact identity bound by this set.
    pub fn expected_patches_artifact_id(&self) -> ArtifactId {
        self.expected_patches_artifact_id
    }

    /// Expected-patch logical identity bound by this set.
    pub fn expected_patches_logical_digest(&self) -> ContentDigest {
        self.expected_patches_logical_digest
    }

    /// Exact patch-context artifact identity bound by this set.
    pub fn patch_context_artifact_id(&self) -> ArtifactId {
        self.patch_context_artifact_id
    }

    /// Exact patch-context logical identity bound by this set.
    pub fn patch_context_logical_digest(&self) -> ContentDigest {
        self.patch_context_logical_digest
    }
}

fn validate_hierarchy(
    hierarchy: &CohortHierarchy,
    expected: &ExpectedPatchSet,
    footprints: &[PatchFootprint],
) -> Result<(), MultiscaleEmbeddingError> {
    let owning = HierarchyId::from(expected.owning_slide_id().clone());
    for footprint in footprints {
        let mut current = HierarchyId::from(footprint.patch_id.clone());
        if !hierarchy.contains(&current) {
            return Err(MultiscaleEmbeddingError::HierarchyOwnershipMismatch);
        }
        loop {
            if current.kind() == HierarchyKind::Slide {
                if current != owning {
                    return Err(MultiscaleEmbeddingError::HierarchyOwnershipMismatch);
                }
                break;
            }
            current = hierarchy
                .parent(&current)
                .cloned()
                .ok_or(MultiscaleEmbeddingError::HierarchyOwnershipMismatch)?;
        }
    }
    Ok(())
}

fn validate_boundary(
    context: &PatchEmbeddingContext,
    origin: [i64; 2],
) -> Result<(), MultiscaleEmbeddingError> {
    let source = context.source_image_px();
    let patch = context.patch_px().map(i64::from);
    for axis in 0..2 {
        let end = origin[axis]
            .checked_add(patch[axis])
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let nonnegative_end = u64::try_from(end).ok();
        match context.boundary_policy() {
            PatchBoundaryPolicy::FullyContainedOnly => {
                if origin[axis] < 0 || nonnegative_end.is_none_or(|end| end > source[axis]) {
                    return Err(MultiscaleEmbeddingError::InvalidPatchFootprint);
                }
            }
            PatchBoundaryPolicy::Reflect | PatchBoundaryPolicy::ConstantRgb(_) => {
                let intersection_start = u64::try_from(origin[axis].max(0))
                    .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
                let intersection_end = nonnegative_end.unwrap_or(0).min(source[axis]);
                if intersection_start >= intersection_end {
                    return Err(MultiscaleEmbeddingError::InvalidPatchFootprint);
                }
                if origin[axis] < 0 {
                    let left = origin[axis]
                        .checked_neg()
                        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
                    if left >= patch[axis] {
                        return Err(MultiscaleEmbeddingError::InvalidPatchFootprint);
                    }
                }
                if let Some(end) = nonnegative_end {
                    if end > source[axis]
                        && end - source[axis]
                            >= u64::try_from(patch[axis])
                                .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?
                    {
                        return Err(MultiscaleEmbeddingError::InvalidPatchFootprint);
                    }
                }
            }
        }
    }
    Ok(())
}

fn retained_bytes(footprints: &Vec<PatchFootprint>) -> Result<usize, MultiscaleEmbeddingError> {
    let row_bytes = footprints
        .capacity()
        .checked_mul(size_of::<PatchFootprint>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    footprints.iter().try_fold(
        size_of::<PatchFootprintSet>()
            .checked_add(row_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        |total, footprint| {
            total
                .checked_add(footprint.patch_id.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        },
    )
}

fn footprint_digest(
    expected_artifact_id: ArtifactId,
    expected_logical_digest: ContentDigest,
    context_artifact_id: ArtifactId,
    context_logical_digest: ContentDigest,
    footprints: &[PatchFootprint],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(FOOTPRINT_DOMAIN);
    digest.artifact_id(expected_artifact_id);
    digest.content_digest(expected_logical_digest);
    digest.artifact_id(context_artifact_id);
    digest.content_digest(context_logical_digest);
    digest.array_len(footprints.len())?;
    for footprint in footprints {
        digest.text(footprint.patch_id.as_str());
        digest.i64(footprint.origin_px[0]);
        digest.i64(footprint.origin_px[1]);
    }
    Ok(digest.finish())
}
