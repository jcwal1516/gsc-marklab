use arrow::datatypes::Schema;
use parquet::file::{metadata::KeyValue, properties::WriterProperties};

use crate::columnar::{
    multiscale::matrix::{matrix_metadata, validate_matrix_domain, MultiscaleMatrixTable},
    parquet::profile::writer_properties_with_metadata,
    ColumnarError,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

pub(in crate::columnar::parquet) const COLUMN_COUNT: usize = 3;

pub(in crate::columnar::parquet) fn schema(
    table: &dyn MultiscaleMatrixTable,
) -> Result<Schema, ColumnarError> {
    Ok(Schema::new(
        crate::columnar::multiscale::matrix_schema_fields(
            table.profile(),
            table.dimension(),
            SpatialPhysicalEncoding::Parquet,
        )?,
    ))
}

pub(in crate::columnar::parquet) fn writer_properties(
    table: &dyn MultiscaleMatrixTable,
) -> Result<WriterProperties, ColumnarError> {
    validate_matrix_domain(table)?;
    let metadata = matrix_metadata(SpatialPhysicalEncoding::Parquet, table)?
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, Some(value)))
        .collect();
    Ok(writer_properties_with_metadata(metadata))
}
