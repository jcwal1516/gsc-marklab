use arrow::datatypes::Schema;
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::PatchRegionLink;

use crate::columnar::{
    multiscale::{patch_region_dependencies, patch_region_metadata, validate_patch_region_domain},
    parquet::profile::writer_properties_with_metadata,
    ColumnarError,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

pub(in crate::columnar::parquet) const ROOT: &str = "marklab_patch_region_link";
pub(in crate::columnar::parquet) const COLUMN_COUNT: usize = 5;

pub(in crate::columnar::parquet) fn schema() -> Schema {
    Schema::new(crate::columnar::multiscale::patch_region_fields())
}

pub(in crate::columnar::parquet) fn writer_properties(
    link: &PatchRegionLink,
) -> Result<WriterProperties, ColumnarError> {
    validate_patch_region_domain(link)?;
    let metadata = patch_region_metadata(SpatialPhysicalEncoding::Parquet, link)
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, Some(value)))
        .collect();
    Ok(writer_properties_with_metadata(metadata))
}

pub(in crate::columnar::parquet) fn dependencies(
    link: &PatchRegionLink,
) -> [marklab_project::ArtifactId; 6] {
    patch_region_dependencies(link)
}
