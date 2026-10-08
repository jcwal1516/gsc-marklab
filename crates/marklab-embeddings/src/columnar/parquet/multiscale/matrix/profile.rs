use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema};
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::columnar::{
    multiscale::matrix::{matrix_metadata, validate_matrix_domain, MultiscaleMatrixTable},
    parquet::profile::writer_properties_with_metadata,
    MultiscaleColumnarError,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

pub(super) const COLUMN_COUNT: usize = 3;

pub(super) fn schema(table: &dyn MultiscaleMatrixTable) -> Result<Schema, MultiscaleColumnarError> {
    let dimension =
        i32::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    Ok(Schema::new(vec![
        Field::new(table.profile().id_column(), DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("element", DataType::Float32, false)),
                dimension,
            ),
            false,
        ),
        Field::new("embedding_status", DataType::Utf8, false),
    ]))
}

pub(super) fn writer_properties(
    table: &dyn MultiscaleMatrixTable,
) -> Result<WriterProperties, MultiscaleColumnarError> {
    validate_matrix_domain(table)?;
    let metadata = matrix_metadata(SpatialPhysicalEncoding::Parquet, table)?
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, Some(value)))
        .collect();
    Ok(writer_properties_with_metadata(metadata))
}
