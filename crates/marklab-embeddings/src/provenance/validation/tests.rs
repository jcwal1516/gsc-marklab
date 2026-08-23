use std::collections::BTreeMap;

use marklab_project::{
    ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema, StoreId,
    TableColumn, TableColumnType, TableFormat, TableManifest, TableScalarType,
};

use super::{require_row_link_manifest, EmbeddingArtifactGraphError};

fn columns(cell_nullable: bool) -> Vec<TableColumn> {
    vec![
        TableColumn::new(
            "cell_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            cell_nullable,
        )
        .expect("cell column"),
        TableColumn::new(
            "source_cell_row",
            TableColumnType::Scalar(TableScalarType::U64),
            false,
        )
        .expect("source cell row"),
        TableColumn::new(
            "source_embedding_row",
            TableColumnType::Scalar(TableScalarType::U64),
            true,
        )
        .expect("source embedding row"),
    ]
}

fn record(
    kind: &str,
    format: TableFormat,
    encoding: &str,
    row_count: u64,
    columns: Vec<TableColumn>,
    primary_key: Vec<String>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, b"row-link").expect("content"),
        ArtifactSchema::new("marklab.cell_embedding_row_link", 1).expect("schema"),
        Some(
            TableManifest::new(format, encoding, row_count, columns, primary_key)
                .expect("table manifest"),
        ),
        Vec::new(),
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("store"),
            ArtifactKey::new("fixtures/row-link").expect("key"),
            None,
        )
        .expect("locator")],
    )
    .expect("record")
}

#[test]
fn row_link_manifest_accepts_both_frozen_physical_profiles() {
    let arrow = record(
        "application/vnd.marklab.embedding-row-link.v1+arrow",
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.embedding-row-link.v1",
        7,
        columns(false),
        vec!["cell_id".to_owned()],
    );
    let parquet = record(
        "application/vnd.marklab.embedding-row-link.v1+parquet",
        TableFormat::ParquetFile,
        "marklab.parquet.embedding-row-link.v1",
        7,
        columns(false),
        vec!["cell_id".to_owned()],
    );
    require_row_link_manifest(&arrow, 7).expect("Arrow row-link manifest");
    require_row_link_manifest(&parquet, 7).expect("Parquet row-link manifest");
}

#[test]
fn row_link_manifest_rejects_format_encoding_count_column_and_key_drift() {
    let mut reordered = columns(false);
    reordered.swap(1, 2);
    let cases = [
        record(
            "application/vnd.marklab.embedding-row-link.v1+arrow",
            TableFormat::ParquetFile,
            "marklab.arrow-ipc.embedding-row-link.v1",
            7,
            columns(false),
            vec!["cell_id".to_owned()],
        ),
        record(
            "application/vnd.marklab.embedding-row-link.v1+arrow",
            TableFormat::ArrowIpcFile,
            "marklab.arrow-ipc.embedding-row-link.v2",
            7,
            columns(false),
            vec!["cell_id".to_owned()],
        ),
        record(
            "application/vnd.marklab.embedding-row-link.v1+arrow",
            TableFormat::ArrowIpcFile,
            "marklab.arrow-ipc.embedding-row-link.v1",
            8,
            columns(false),
            vec!["cell_id".to_owned()],
        ),
        record(
            "application/vnd.marklab.embedding-row-link.v1+arrow",
            TableFormat::ArrowIpcFile,
            "marklab.arrow-ipc.embedding-row-link.v1",
            7,
            reordered,
            vec!["cell_id".to_owned()],
        ),
        record(
            "application/vnd.marklab.embedding-row-link.v1+arrow",
            TableFormat::ArrowIpcFile,
            "marklab.arrow-ipc.embedding-row-link.v1",
            7,
            columns(true),
            vec!["source_cell_row".to_owned()],
        ),
    ];
    for case in cases {
        assert_eq!(
            require_row_link_manifest(&case, 7),
            Err(EmbeddingArtifactGraphError::RowLinkManifestMismatch)
        );
    }
}
