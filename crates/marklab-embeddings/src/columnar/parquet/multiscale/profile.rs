use arrow::datatypes::Schema;
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::{ExpectedPatchSet, PatchFootprintSet, PatchOverlapGraph};

use super::super::profile::writer_properties_with_metadata;
use crate::multiscale::physical::{
    encoding_version, schema_id, SpatialArtifactRole, SpatialPhysicalEncoding,
};

pub(in crate::columnar::parquet) const FOOTPRINT_ROOT: &str = "marklab_patch_footprint_table";
pub(in crate::columnar::parquet) const OVERLAP_ROOT: &str = "marklab_patch_overlap_edge_table";

pub(in crate::columnar::parquet) const FOOTPRINT_METADATA_KEYS: [&str; 7] = [
    "marklab.encoding_version",
    "marklab.expected_patches_artifact_id",
    "marklab.logical_digest",
    "marklab.owning_slide_id",
    "marklab.patch_context_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
];

pub(in crate::columnar::parquet) const OVERLAP_METADATA_KEYS: [&str; 8] = [
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
pub(in crate::columnar::parquet) enum SpatialParquetProfile {
    Footprint,
    Overlap,
}

impl SpatialParquetProfile {
    pub(in crate::columnar::parquet) fn column_count(self) -> usize {
        match self {
            Self::Footprint => 3,
            Self::Overlap => 2,
        }
    }

    pub(in crate::columnar::parquet) fn root(self) -> &'static str {
        match self {
            Self::Footprint => FOOTPRINT_ROOT,
            Self::Overlap => OVERLAP_ROOT,
        }
    }
}

pub(in crate::columnar::parquet) fn footprint_schema() -> Schema {
    Schema::new(crate::columnar::multiscale::footprint_fields())
}

pub(in crate::columnar::parquet) fn overlap_schema() -> Schema {
    Schema::new(crate::columnar::multiscale::overlap_fields())
}

pub(in crate::columnar::parquet) fn expected_schema(profile: SpatialParquetProfile) -> Schema {
    match profile {
        SpatialParquetProfile::Footprint => footprint_schema(),
        SpatialParquetProfile::Overlap => overlap_schema(),
    }
}

pub(in crate::columnar::parquet) fn writer_properties(
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

pub(in crate::columnar::parquet) fn metadata_entries(
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
