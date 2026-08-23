use crate::{
    columnar::{
        multiscale::{
            patch_region_dependencies, patch_region_metadata, validate_patch_region_domain,
            PATCH_REGION_METADATA_KEYS,
        },
        MultiscaleColumnarError, SpatialArrowFailure,
    },
    multiscale::physical::{SpatialArtifactRole, SpatialPhysicalEncoding},
    PatchRegionLink,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow_ipc::{Endianness, Type};

pub(super) const METADATA_LIMIT: usize = 64 * 1024;
pub(super) const FIELD_COUNT: usize = 5;
pub(super) const BUFFER_COUNT: usize = 13;

pub(super) fn dependencies(link: &PatchRegionLink) -> [marklab_project::ArtifactId; 6] {
    patch_region_dependencies(link)
}

pub(super) fn schema(link: &PatchRegionLink) -> Result<Schema, MultiscaleColumnarError> {
    validate_patch_region_domain(link)?;
    Ok(Schema::new_with_metadata(
        vec![
            Field::new("patch_id", DataType::Utf8, false),
            Field::new("region_id", DataType::Utf8, false),
            Field::new("relation", DataType::Utf8, false),
            Field::new("overlap_numerator", DataType::UInt64, false),
            Field::new("overlap_denominator", DataType::UInt64, false),
        ],
        metadata(link).into_iter().collect(),
    ))
}

pub(super) fn validate_flatbuffer_schema(
    schema: arrow_ipc::Schema<'_>,
    link: &PatchRegionLink,
) -> Result<(), MultiscaleColumnarError> {
    if schema.endianness() != Endianness::Little
        || schema
            .features()
            .is_some_and(|features| !features.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    let fields = schema
        .fields()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidSchema))?;
    if fields.len() != FIELD_COUNT {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    validate_utf8(fields.get(0), "patch_id")?;
    validate_utf8(fields.get(1), "region_id")?;
    validate_utf8(fields.get(2), "relation")?;
    validate_u64(fields.get(3), "overlap_numerator")?;
    validate_u64(fields.get(4), "overlap_denominator")?;
    validate_metadata(schema.custom_metadata(), link)?;
    validate_patch_region_domain(link)
}

fn metadata(link: &PatchRegionLink) -> [(String, String); 13] {
    patch_region_metadata(SpatialPhysicalEncoding::Arrow, link)
}

fn validate_metadata(
    entries: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<arrow_ipc::KeyValue<'_>>>>,
    link: &PatchRegionLink,
) -> Result<(), MultiscaleColumnarError> {
    let expected = metadata(link);
    let entries =
        entries.ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidApplicationMetadata))?;
    if entries.len() != PATCH_REGION_METADATA_KEYS.len() {
        return Err(arrow_failure(
            SpatialArrowFailure::InvalidApplicationMetadata,
        ));
    }
    let mut total = 0_usize;
    for (entry, (expected_key, expected_value)) in entries.iter().zip(expected) {
        let key = entry
            .key()
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidApplicationMetadata))?;
        let value = entry
            .value()
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidApplicationMetadata))?;
        total = total
            .checked_add(key.len())
            .and_then(|sum| sum.checked_add(value.len()))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if key != expected_key || value != expected_value || value.is_empty() {
            return Err(arrow_failure(
                SpatialArrowFailure::InvalidApplicationMetadata,
            ));
        }
    }
    if total > METADATA_LIMIT {
        return Err(arrow_failure(
            SpatialArrowFailure::InvalidApplicationMetadata,
        ));
    }
    Ok(())
}

fn validate_utf8(field: arrow_ipc::Field<'_>, name: &str) -> Result<(), MultiscaleColumnarError> {
    if field.name() != Some(name)
        || field.nullable()
        || field.dictionary().is_some()
        || field
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || field
            .children()
            .is_some_and(|children| !children.is_empty())
        || field.type_type() != Type::Utf8
        || field.type_as_utf_8().is_none()
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    Ok(())
}

fn validate_u64(field: arrow_ipc::Field<'_>, name: &str) -> Result<(), MultiscaleColumnarError> {
    let integer = field.type_as_int();
    if field.name() != Some(name)
        || field.nullable()
        || field.dictionary().is_some()
        || field
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || field
            .children()
            .is_some_and(|children| !children.is_empty())
        || field.type_type() != Type::Int
        || integer.is_none_or(|integer| integer.bitWidth() != 64 || integer.is_signed())
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    Ok(())
}

pub(super) fn role() -> SpatialArtifactRole {
    SpatialArtifactRole::PatchRegion
}

pub(super) fn arrow_failure(reason: SpatialArrowFailure) -> MultiscaleColumnarError {
    MultiscaleColumnarError::Arrow { reason }
}
