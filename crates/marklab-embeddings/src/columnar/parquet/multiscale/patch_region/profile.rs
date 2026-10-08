use arrow::datatypes::{DataType, Field, Schema};
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::PatchRegionLink;

use crate::columnar::{
    multiscale::{patch_region_dependencies, patch_region_metadata, validate_patch_region_domain},
    parquet::profile::writer_properties_with_metadata,
    MultiscaleColumnarError,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

pub(super) const ROOT: &str = "marklab_patch_region_link";
pub(super) const COLUMN_COUNT: usize = 5;

pub(super) fn schema() -> Schema {
    Schema::new(vec![
        Field::new("patch_id", DataType::Utf8, false),
        Field::new("region_id", DataType::Utf8, false),
        Field::new("relation", DataType::Utf8, false),
        Field::new("overlap_numerator", DataType::UInt64, false),
        Field::new("overlap_denominator", DataType::UInt64, false),
    ])
}

pub(super) fn writer_properties(
    link: &PatchRegionLink,
) -> Result<WriterProperties, MultiscaleColumnarError> {
    validate_patch_region_domain(link)?;
    let metadata = patch_region_metadata(SpatialPhysicalEncoding::Parquet, link)
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, Some(value)))
        .collect();
    Ok(writer_properties_with_metadata(metadata))
}

pub(super) fn dependencies(link: &PatchRegionLink) -> [marklab_project::ArtifactId; 6] {
    patch_region_dependencies(link)
}
