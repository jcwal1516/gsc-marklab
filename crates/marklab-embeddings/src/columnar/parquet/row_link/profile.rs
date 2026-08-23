use arrow::datatypes::{DataType, Field, Schema};
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::CellEmbeddingRowLink;

use super::super::profile::writer_properties_with_metadata;

pub(super) const SCHEMA_ID: &str = "marklab.cell_embedding_row_link";
const SCHEMA_VERSION: &str = "1";
pub(super) const ENCODING_VERSION: &str = "marklab.parquet.embedding-row-link.v1";
pub(super) const CONTENT_KIND: &str = "application/vnd.marklab.embedding-row-link.v1+parquet";
pub(super) const ROOT_NAME: &str = "marklab_cell_embedding_row_link";

pub(super) const METADATA_KEYS: [&str; 9] = [
    "marklab.converter_artifact_id",
    "marklab.encoding_version",
    "marklab.expected_cells_artifact_id",
    "marklab.identity_map_artifact_id",
    "marklab.row_link_digest",
    "marklab.schema_id",
    "marklab.schema_version",
    "marklab.source_cells_artifact_id",
    "marklab.source_vectors_artifact_id",
];

pub(super) fn row_link_schema() -> Schema {
    Schema::new(vec![
        Field::new("cell_id", DataType::Utf8, false),
        Field::new("source_cell_row", DataType::UInt64, false),
        Field::new("source_embedding_row", DataType::UInt64, true),
    ])
}

pub(super) fn metadata_values(row_link: &CellEmbeddingRowLink) -> [String; 9] {
    [
        row_link.converter_artifact_id().to_string(),
        ENCODING_VERSION.to_owned(),
        row_link.expected_cells_artifact_id().to_string(),
        row_link.identity_map_artifact_id().to_string(),
        row_link.logical_digest().to_string(),
        SCHEMA_ID.to_owned(),
        SCHEMA_VERSION.to_owned(),
        row_link.source_cells_artifact_id().to_string(),
        row_link.source_vectors_artifact_id().to_string(),
    ]
}

pub(super) fn writer_properties(row_link: &CellEmbeddingRowLink) -> WriterProperties {
    let metadata = METADATA_KEYS
        .iter()
        .zip(metadata_values(row_link))
        .map(|(key, value)| KeyValue::new((*key).to_owned(), Some(value)))
        .collect();
    writer_properties_with_metadata(metadata)
}
