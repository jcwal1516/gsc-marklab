use arrow::datatypes::{DataType, Field, Schema};
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::{ExpectedPatchSet, PatchFootprintSet, PatchOverlapGraph};

use super::super::profile::writer_properties_with_metadata;
use crate::multiscale::physical::{
    encoding_version, schema_id, SpatialArtifactRole, SpatialPhysicalEncoding,
};

pub(super) const FOOTPRINT_ROOT: &str = "marklab_patch_footprint_table";
pub(super) const OVERLAP_ROOT: &str = "marklab_patch_overlap_edge_table";

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
pub(super) enum SpatialParquetProfile {
    Footprint,
    Overlap,
}

impl SpatialParquetProfile {
    pub(super) fn column_count(self) -> usize {
        match self {
            Self::Footprint => 3,
            Self::Overlap => 2,
        }
    }

    pub(super) fn root(self) -> &'static str {
        match self {
            Self::Footprint => FOOTPRINT_ROOT,
            Self::Overlap => OVERLAP_ROOT,
        }
    }
}

pub(super) fn footprint_schema() -> Schema {
    Schema::new(vec![
        Field::new("patch_id", DataType::Utf8, false),
        Field::new("origin_x_px", DataType::Int64, false),
        Field::new("origin_y_px", DataType::Int64, false),
    ])
}

pub(super) fn overlap_schema() -> Schema {
    Schema::new(vec![
        Field::new("left_patch_id", DataType::Utf8, false),
        Field::new("right_patch_id", DataType::Utf8, false),
    ])
}

pub(super) fn expected_schema(profile: SpatialParquetProfile) -> Schema {
    match profile {
        SpatialParquetProfile::Footprint => footprint_schema(),
        SpatialParquetProfile::Overlap => overlap_schema(),
    }
}

pub(super) fn writer_properties(
    profile: SpatialParquetProfile,
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> WriterProperties {
    let metadata = metadata_entries(profile, expected, footprints, overlap)
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, Some(value)))
        .collect();
    writer_properties_with_metadata(metadata)
}

pub(super) fn metadata_entries(
    profile: SpatialParquetProfile,
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Vec<(String, String)> {
    match profile {
        SpatialParquetProfile::Footprint => {
            let values = [
                encoding_version(
                    SpatialArtifactRole::Footprint,
                    SpatialPhysicalEncoding::Parquet,
                )
                .to_owned(),
                footprints.expected_patches_artifact_id().to_string(),
                footprints.logical_digest().to_string(),
                expected.owning_slide_id().as_str().to_owned(),
                footprints.patch_context_artifact_id().to_string(),
                schema_id(SpatialArtifactRole::Footprint).to_owned(),
                "1".to_owned(),
            ];
            FOOTPRINT_METADATA_KEYS
                .iter()
                .zip(values)
                .map(|(key, value)| ((*key).to_owned(), value))
                .collect()
        }
        SpatialParquetProfile::Overlap => {
            let overlap = overlap.expect("validated overlap profile requires overlap domain");
            let values = [
                encoding_version(
                    SpatialArtifactRole::Overlap,
                    SpatialPhysicalEncoding::Parquet,
                )
                .to_owned(),
                overlap.expected_patches_artifact_id().to_string(),
                overlap.patch_footprints_artifact_id().to_string(),
                overlap.logical_digest().to_string(),
                expected.owning_slide_id().as_str().to_owned(),
                footprints.patch_context_artifact_id().to_string(),
                schema_id(SpatialArtifactRole::Overlap).to_owned(),
                "1".to_owned(),
            ];
            OVERLAP_METADATA_KEYS
                .iter()
                .zip(values)
                .map(|(key, value)| ((*key).to_owned(), value))
                .collect()
        }
    }
}
