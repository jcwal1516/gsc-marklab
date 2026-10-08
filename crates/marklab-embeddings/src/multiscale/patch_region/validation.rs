use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

use super::{
    resources::MAX_PATCH_REGION_ROWS,
    types::{valid_relation, PatchRegionAssessmentBindings, PatchRegionDeclaration},
};
use crate::multiscale::{
    context::PatchEmbeddingContext,
    error::MultiscaleEmbeddingError,
    expected::{ExpectedPatchSet, ExpectedRegionSet},
    footprint::PatchFootprintSet,
};

#[derive(Clone, Eq, PartialEq)]
pub(super) struct CommonBindings {
    pub(super) owning_slide_id: SlideId,
    pub(super) expected_patches_artifact_id: ArtifactId,
    pub(super) expected_patches_logical_digest: ContentDigest,
    pub(super) expected_regions_artifact_id: ArtifactId,
    pub(super) expected_regions_logical_digest: ContentDigest,
    pub(super) patch_context_artifact_id: ArtifactId,
    pub(super) patch_context_logical_digest: ContentDigest,
    pub(super) patch_footprints_artifact_id: ArtifactId,
    pub(super) patch_footprints_logical_digest: ContentDigest,
    pub(super) converter_artifact_id: ArtifactId,
    pub(super) converter_content_digest: ContentDigest,
    pub(super) assessed_pair_count: u64,
}

pub(super) fn validate(
    expected_patches: &ExpectedPatchSet,
    expected_regions: &ExpectedRegionSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    bindings: &PatchRegionAssessmentBindings,
    declarations: &[PatchRegionDeclaration],
) -> Result<CommonBindings, MultiscaleEmbeddingError> {
    validate_row_count(declarations.len())?;
    if expected_patches.owning_slide_id() != expected_regions.owning_slide_id()
        || expected_patches.owning_slide_id() != context.owning_slide_id()
        || expected_patches.logical_digest() != footprints.expected_patches_logical_digest()
        || context.logical_digest() != footprints.patch_context_logical_digest()
        || expected_patches.ids().len() != footprints.footprints().len()
        || expected_patches
            .ids()
            .iter()
            .zip(footprints.footprints())
            .any(|(expected, footprint)| expected != footprint.patch_id())
    {
        return Err(MultiscaleEmbeddingError::PatchRegionInputMismatch);
    }
    validate_rows(expected_patches, expected_regions, declarations)?;
    let dependencies = [
        footprints.expected_patches_artifact_id(),
        bindings.expected_regions_artifact_id,
        footprints.patch_context_artifact_id(),
        bindings.patch_footprints_artifact_id,
        bindings.converter_artifact_id,
    ];
    let mut sorted = dependencies;
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(MultiscaleEmbeddingError::DuplicatePatchRegionArtifactDependency);
    }
    let patch_count = u64::try_from(expected_patches.ids().len())
        .map_err(|_| MultiscaleEmbeddingError::PatchRegionPairCountOverflow)?;
    let region_count = u64::try_from(expected_regions.ids().len())
        .map_err(|_| MultiscaleEmbeddingError::PatchRegionPairCountOverflow)?;
    let assessed_pair_count = checked_assessed_pair_count(patch_count, region_count)?;
    Ok(CommonBindings {
        owning_slide_id: context.owning_slide_id().clone(),
        expected_patches_artifact_id: footprints.expected_patches_artifact_id(),
        expected_patches_logical_digest: footprints.expected_patches_logical_digest(),
        expected_regions_artifact_id: bindings.expected_regions_artifact_id,
        expected_regions_logical_digest: expected_regions.logical_digest(),
        patch_context_artifact_id: footprints.patch_context_artifact_id(),
        patch_context_logical_digest: footprints.patch_context_logical_digest(),
        patch_footprints_artifact_id: bindings.patch_footprints_artifact_id,
        patch_footprints_logical_digest: footprints.logical_digest(),
        converter_artifact_id: bindings.converter_artifact_id,
        converter_content_digest: bindings.converter_content_digest,
        assessed_pair_count,
    })
}

fn checked_assessed_pair_count(
    patch_count: u64,
    region_count: u64,
) -> Result<u64, MultiscaleEmbeddingError> {
    patch_count
        .checked_mul(region_count)
        .ok_or(MultiscaleEmbeddingError::PatchRegionPairCountOverflow)
}

fn validate_row_count(observed: usize) -> Result<(), MultiscaleEmbeddingError> {
    if observed > MAX_PATCH_REGION_ROWS {
        return Err(MultiscaleEmbeddingError::PatchRegionRowCountExceeded {
            observed,
            maximum: MAX_PATCH_REGION_ROWS,
        });
    }
    Ok(())
}

fn validate_rows(
    expected_patches: &ExpectedPatchSet,
    expected_regions: &ExpectedRegionSet,
    declarations: &[PatchRegionDeclaration],
) -> Result<(), MultiscaleEmbeddingError> {
    for (row, declaration) in declarations.iter().enumerate() {
        if !valid_relation(declaration)
            || expected_patches
                .ids()
                .binary_search(&declaration.patch_id)
                .is_err()
            || expected_regions
                .ids()
                .binary_search(&declaration.region_id)
                .is_err()
        {
            return Err(MultiscaleEmbeddingError::InvalidPatchRegionDeclarations { row });
        }
        if row > 0 {
            let previous = &declarations[row - 1];
            if (&previous.patch_id, &previous.region_id)
                >= (&declaration.patch_id, &declaration.region_id)
            {
                return Err(MultiscaleEmbeddingError::InvalidPatchRegionDeclarations { row });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{checked_assessed_pair_count, validate_row_count, MAX_PATCH_REGION_ROWS};
    use crate::multiscale::error::MultiscaleEmbeddingError;

    #[test]
    fn patch_region_row_limit_rejects_the_first_excess_row() {
        assert_eq!(validate_row_count(MAX_PATCH_REGION_ROWS), Ok(()));
        assert_eq!(
            validate_row_count(MAX_PATCH_REGION_ROWS + 1),
            Err(MultiscaleEmbeddingError::PatchRegionRowCountExceeded {
                observed: MAX_PATCH_REGION_ROWS + 1,
                maximum: MAX_PATCH_REGION_ROWS,
            })
        );
    }

    #[test]
    fn assessed_pair_count_rejects_u64_product_overflow() {
        assert_eq!(checked_assessed_pair_count(u64::MAX, 1), Ok(u64::MAX));
        assert_eq!(
            checked_assessed_pair_count(u64::MAX, 2),
            Err(MultiscaleEmbeddingError::PatchRegionPairCountOverflow)
        );
    }
}
