use std::mem::size_of;

use marklab_project::ArtifactId;

use crate::{
    multiscale::physical::{
        encoding_version, schema_id, SpatialArtifactRole, SpatialPhysicalEncoding,
    },
    rational::greatest_common_divisor,
    PatchRegionDeclaration, PatchRegionLink, PatchRegionRelation,
};

use super::MultiscaleColumnarError;

pub(crate) const PATCH_REGION_METADATA_KEYS: [&str; 13] = [
    "marklab.assessed_pair_count",
    "marklab.assessment_artifact_id",
    "marklab.assessment_policy",
    "marklab.converter_artifact_id",
    "marklab.encoding_version",
    "marklab.expected_patches_artifact_id",
    "marklab.expected_regions_artifact_id",
    "marklab.footprint_artifact_id",
    "marklab.link_logical_digest",
    "marklab.owning_slide_id",
    "marklab.patch_context_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
];

pub(crate) fn patch_region_dependencies(link: &PatchRegionLink) -> [ArtifactId; 6] {
    let mut dependencies = [
        link.expected_patches_artifact_id(),
        link.expected_regions_artifact_id(),
        link.patch_context_artifact_id(),
        link.patch_footprints_artifact_id(),
        link.converter_artifact_id(),
        link.assessment_artifact_id(),
    ];
    dependencies.sort_unstable();
    dependencies
}

pub(crate) fn patch_region_metadata(
    encoding: SpatialPhysicalEncoding,
    link: &PatchRegionLink,
) -> [(String, String); 13] {
    let role = SpatialArtifactRole::PatchRegion;
    let values = [
        link.assessed_pair_count().to_string(),
        link.assessment_artifact_id().to_string(),
        "expected_cartesian_exhaustive".to_owned(),
        link.converter_artifact_id().to_string(),
        encoding_version(role, encoding).to_owned(),
        link.expected_patches_artifact_id().to_string(),
        link.expected_regions_artifact_id().to_string(),
        link.patch_footprints_artifact_id().to_string(),
        link.logical_digest().to_string(),
        link.owning_slide_id().as_str().to_owned(),
        link.patch_context_artifact_id().to_string(),
        schema_id(role).to_owned(),
        "1".to_owned(),
    ];
    std::array::from_fn(|index| {
        (
            PATCH_REGION_METADATA_KEYS[index].to_owned(),
            values[index].clone(),
        )
    })
}

pub(crate) fn validate_patch_region_domain(
    link: &PatchRegionLink,
) -> Result<(), MultiscaleColumnarError> {
    let dependencies = patch_region_dependencies(link);
    if dependencies.windows(2).any(|pair| pair[0] == pair[1])
        || u64::try_from(link.nonzero_relation_count())
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?
            > link.assessed_pair_count()
        || link.nonzero_relations().windows(2).any(|pair| {
            (pair[0].patch_id(), pair[0].region_id()) >= (pair[1].patch_id(), pair[1].region_id())
        })
        || link
            .nonzero_relations()
            .iter()
            .any(|row| match row.relation() {
                PatchRegionRelation::FullyContained => {
                    row.numerator() != 1 || row.denominator() != 1
                }
                PatchRegionRelation::PartialOverlap => {
                    row.numerator() == 0
                        || row.numerator() >= row.denominator()
                        || greatest_common_divisor(row.numerator(), row.denominator()) != 1
                }
            })
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn patch_region_decoded_bytes(
    rows: &[PatchRegionDeclaration],
) -> Result<usize, MultiscaleColumnarError> {
    let text = rows.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.patch_id().as_str().len())
            .and_then(|value| value.checked_add(row.region_id().as_str().len()))
            .and_then(|value| value.checked_add(row.relation().wire_name().len()))
            .ok_or(MultiscaleColumnarError::SizeOverflow)
    })?;
    let validity = rows
        .len()
        .checked_add(7)
        .map(|value| value / 8)
        .and_then(|value| value.checked_mul(5))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let offsets = rows
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .and_then(|value| value.checked_mul(3))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let values = rows
        .len()
        .checked_mul(2 * size_of::<u64>())
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    validity
        .checked_add(offsets)
        .and_then(|value| value.checked_add(text))
        .and_then(|value| value.checked_add(values))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}
