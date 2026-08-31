use arrow::datatypes::{DataType, Field, Schema};
use arrow_ipc::{Endianness, Type};

use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

use super::super::super::multiscale::{
    validate_footprint_domain, validate_overlap_domain, MultiscaleColumnarError,
    SpatialArrowFailure,
};
use super::physical::validate_required_utf8_field;
use crate::multiscale::physical::{
    encoding_version, schema_id, SpatialArtifactRole, SpatialPhysicalEncoding,
};

pub(super) const METADATA_LIMIT: usize = 64 * 1024;

pub(super) const FOOTPRINT_METADATA_KEYS: [&str; 7] = [
    "marklab.encoding_version",
    "marklab.expected_patches_artifact_id",
    "marklab.logical_digest",
    "marklab.owning_slide_id",
    "marklab.patch_context_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
];

pub(super) const OVERLAP_METADATA_KEYS: [&str; 8] = [
    "marklab.encoding_version",
    "marklab.expected_patches_artifact_id",
    "marklab.footprint_artifact_id",
    "marklab.logical_digest",
    "marklab.owning_slide_id",
    "marklab.patch_context_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SpatialArrowProfile {
    Footprint,
    Overlap,
}

impl SpatialArrowProfile {
    pub(super) fn field_count(self) -> usize {
        match self {
            Self::Footprint => 3,
            Self::Overlap => 2,
        }
    }

    pub(super) fn buffer_count(self) -> usize {
        match self {
            Self::Footprint => 7,
            Self::Overlap => 6,
        }
    }
}

pub(super) fn footprint_schema(
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
) -> Result<Schema, MultiscaleColumnarError> {
    validate_footprint_domain(expected, context, footprints)?;
    Ok(Schema::new_with_metadata(
        vec![
            Field::new("patch_id", DataType::Utf8, false),
            Field::new("origin_x_px", DataType::Int64, false),
            Field::new("origin_y_px", DataType::Int64, false),
        ],
        footprint_metadata(expected, footprints)
            .into_iter()
            .collect(),
    ))
}

pub(super) fn overlap_schema(
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
) -> Result<Schema, MultiscaleColumnarError> {
    validate_overlap_domain(expected, context, footprints, overlap)?;
    Ok(Schema::new_with_metadata(
        vec![
            Field::new("left_patch_id", DataType::Utf8, false),
            Field::new("right_patch_id", DataType::Utf8, false),
        ],
        overlap_metadata(expected, footprints, overlap)
            .into_iter()
            .collect(),
    ))
}

pub(super) fn validate_flatbuffer_schema(
    schema: arrow_ipc::Schema<'_>,
    profile: SpatialArrowProfile,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<(), MultiscaleColumnarError> {
    validate_schema_shape(schema, profile)?;
    validate_metadata(
        schema.custom_metadata(),
        profile,
        expected,
        footprints,
        overlap,
    )?;
    match profile {
        SpatialArrowProfile::Footprint => validate_footprint_domain(expected, context, footprints),
        SpatialArrowProfile::Overlap => validate_overlap_domain(
            expected,
            context,
            footprints,
            overlap.ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?,
        ),
    }
}

fn validate_schema_shape(
    schema: arrow_ipc::Schema<'_>,
    profile: SpatialArrowProfile,
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
    if fields.len() != profile.field_count() {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    match profile {
        SpatialArrowProfile::Footprint => {
            validate_required_utf8_field(fields.get(0), "patch_id")?;
            validate_i64_field(fields.get(1), "origin_x_px")?;
            validate_i64_field(fields.get(2), "origin_y_px")
        }
        SpatialArrowProfile::Overlap => {
            validate_required_utf8_field(fields.get(0), "left_patch_id")?;
            validate_required_utf8_field(fields.get(1), "right_patch_id")
        }
    }
}

pub(super) fn expected_schema(
    profile: SpatialArrowProfile,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<Schema, MultiscaleColumnarError> {
    match profile {
        SpatialArrowProfile::Footprint => footprint_schema(expected, context, footprints),
        SpatialArrowProfile::Overlap => overlap_schema(
            expected,
            context,
            footprints,
            overlap.ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?,
        ),
    }
}

fn footprint_metadata(
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
) -> [(String, String); 7] {
    let values = [
        encoding_version(
            SpatialArtifactRole::Footprint,
            SpatialPhysicalEncoding::Arrow,
        )
        .to_owned(),
        footprints.expected_patches_artifact_id().to_string(),
        footprints.logical_digest().to_string(),
        expected.owning_slide_id().as_str().to_owned(),
        footprints.patch_context_artifact_id().to_string(),
        schema_id(SpatialArtifactRole::Footprint).to_owned(),
        "1".to_owned(),
    ];
    std::array::from_fn(|index| {
        (
            FOOTPRINT_METADATA_KEYS[index].to_owned(),
            values[index].clone(),
        )
    })
}

fn overlap_metadata(
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
) -> [(String, String); 8] {
    let values = [
        encoding_version(SpatialArtifactRole::Overlap, SpatialPhysicalEncoding::Arrow).to_owned(),
        overlap.expected_patches_artifact_id().to_string(),
        overlap.patch_footprints_artifact_id().to_string(),
        overlap.logical_digest().to_string(),
        expected.owning_slide_id().as_str().to_owned(),
        footprints.patch_context_artifact_id().to_string(),
        schema_id(SpatialArtifactRole::Overlap).to_owned(),
        "1".to_owned(),
    ];
    std::array::from_fn(|index| {
        (
            OVERLAP_METADATA_KEYS[index].to_owned(),
            values[index].clone(),
        )
    })
}

fn validate_metadata(
    metadata: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<arrow_ipc::KeyValue<'_>>>,
    >,
    profile: SpatialArrowProfile,
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<(), MultiscaleColumnarError> {
    let expected_entries: Vec<(String, String)> = match profile {
        SpatialArrowProfile::Footprint => footprint_metadata(expected, footprints).into(),
        SpatialArrowProfile::Overlap => overlap_metadata(
            expected,
            footprints,
            overlap.ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?,
        )
        .into(),
    };
    let metadata =
        metadata.ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidApplicationMetadata))?;
    if metadata.len() != expected_entries.len() {
        return Err(arrow_failure(
            SpatialArrowFailure::InvalidApplicationMetadata,
        ));
    }
    let mut total = 0_usize;
    for (entry, (expected_key, expected_value)) in metadata.iter().zip(expected_entries) {
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

fn validate_i64_field(
    field: arrow_ipc::Field<'_>,
    name: &str,
) -> Result<(), MultiscaleColumnarError> {
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
        || integer.is_none_or(|integer| integer.bitWidth() != 64 || !integer.is_signed())
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    Ok(())
}

pub(super) fn arrow_failure(reason: SpatialArrowFailure) -> MultiscaleColumnarError {
    MultiscaleColumnarError::Arrow { reason }
}

#[cfg(test)]
mod tests {
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow_ipc::{
        convert::IpcSchemaEncoder, root_as_schema, writer::DictionaryTracker, Feature,
        Schema as FlatSchema, SchemaArgs,
    };
    use flatbuffers::FlatBufferBuilder;

    use super::*;

    fn encoded_schema(schema: &Schema) -> Vec<u8> {
        let mut dictionaries = DictionaryTracker::new(true);
        IpcSchemaEncoder::new()
            .with_dictionary_tracker(&mut dictionaries)
            .schema_to_fb(schema)
            .finished_data()
            .to_vec()
    }

    #[test]
    fn c05_arrow_schema_shape_rejects_extra_fields_and_field_dictionaries() {
        let extra = Schema::new(vec![
            Field::new("patch_id", DataType::Utf8, false),
            Field::new("origin_x_px", DataType::Int64, false),
            Field::new("origin_y_px", DataType::Int64, false),
            Field::new("extra", DataType::Int64, false),
        ]);
        let extra_bytes = encoded_schema(&extra);
        assert!(matches!(
            validate_schema_shape(
                root_as_schema(&extra_bytes).expect("extra-field schema"),
                SpatialArrowProfile::Footprint,
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidSchema,
            })
        ));

        let dictionary = Schema::new(vec![
            Field::new(
                "patch_id",
                DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new("origin_x_px", DataType::Int64, false),
            Field::new("origin_y_px", DataType::Int64, false),
        ]);
        let dictionary_bytes = encoded_schema(&dictionary);
        assert!(matches!(
            validate_schema_shape(
                root_as_schema(&dictionary_bytes).expect("dictionary schema"),
                SpatialArrowProfile::Footprint,
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidSchema,
            })
        ));
    }

    #[test]
    fn c05_arrow_schema_shape_rejects_declared_features() {
        let mut builder = FlatBufferBuilder::new();
        let empty_fields =
            builder.create_vector::<flatbuffers::WIPOffset<arrow_ipc::Field<'_>>>(&[]);
        let features = builder.create_vector(&[Feature::COMPRESSED_BODY]);
        let schema = FlatSchema::create(
            &mut builder,
            &SchemaArgs {
                endianness: Endianness::Little,
                fields: Some(empty_fields),
                custom_metadata: None,
                features: Some(features),
            },
        );
        builder.finish(schema, None);
        assert!(matches!(
            validate_schema_shape(
                root_as_schema(builder.finished_data()).expect("feature schema"),
                SpatialArrowProfile::Footprint,
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidSchema,
            })
        ));
    }
}
