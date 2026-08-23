use marklab_project::{
    ArtifactRecord, TableColumn, TableColumnType, TableFormat, TableManifest, TableManifestError,
    TableScalarType,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpatialArtifactRole {
    Footprint,
    Overlap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpatialPhysicalEncoding {
    Arrow,
    Parquet,
}

pub(crate) const FOOTPRINT_SCHEMA_ID: &str = "marklab.patch_footprint_table";
pub(crate) const OVERLAP_SCHEMA_ID: &str = "marklab.patch_overlap_edge_table";
pub(crate) const FOOTPRINT_ARROW_ENCODING: &str = "marklab.arrow-ipc.patch-footprint-table.v1";
pub(crate) const FOOTPRINT_PARQUET_ENCODING: &str = "marklab.parquet.patch-footprint-table.v1";
pub(crate) const OVERLAP_ARROW_ENCODING: &str = "marklab.arrow-ipc.patch-overlap-edge-table.v1";
pub(crate) const OVERLAP_PARQUET_ENCODING: &str = "marklab.parquet.patch-overlap-edge-table.v1";
pub(crate) const FOOTPRINT_ARROW_KIND: &str =
    "application/vnd.marklab.patch-footprint-table.v1+arrow";
pub(crate) const FOOTPRINT_PARQUET_KIND: &str =
    "application/vnd.marklab.patch-footprint-table.v1+parquet";
pub(crate) const OVERLAP_ARROW_KIND: &str =
    "application/vnd.marklab.patch-overlap-edge-table.v1+arrow";
pub(crate) const OVERLAP_PARQUET_KIND: &str =
    "application/vnd.marklab.patch-overlap-edge-table.v1+parquet";

pub(crate) fn schema_id(role: SpatialArtifactRole) -> &'static str {
    match role {
        SpatialArtifactRole::Footprint => FOOTPRINT_SCHEMA_ID,
        SpatialArtifactRole::Overlap => OVERLAP_SCHEMA_ID,
    }
}

pub(crate) fn encoding_version(
    role: SpatialArtifactRole,
    encoding: SpatialPhysicalEncoding,
) -> &'static str {
    match (role, encoding) {
        (SpatialArtifactRole::Footprint, SpatialPhysicalEncoding::Arrow) => {
            FOOTPRINT_ARROW_ENCODING
        }
        (SpatialArtifactRole::Footprint, SpatialPhysicalEncoding::Parquet) => {
            FOOTPRINT_PARQUET_ENCODING
        }
        (SpatialArtifactRole::Overlap, SpatialPhysicalEncoding::Arrow) => OVERLAP_ARROW_ENCODING,
        (SpatialArtifactRole::Overlap, SpatialPhysicalEncoding::Parquet) => {
            OVERLAP_PARQUET_ENCODING
        }
    }
}

pub(crate) fn content_kind(
    role: SpatialArtifactRole,
    encoding: SpatialPhysicalEncoding,
) -> &'static str {
    match (role, encoding) {
        (SpatialArtifactRole::Footprint, SpatialPhysicalEncoding::Arrow) => FOOTPRINT_ARROW_KIND,
        (SpatialArtifactRole::Footprint, SpatialPhysicalEncoding::Parquet) => {
            FOOTPRINT_PARQUET_KIND
        }
        (SpatialArtifactRole::Overlap, SpatialPhysicalEncoding::Arrow) => OVERLAP_ARROW_KIND,
        (SpatialArtifactRole::Overlap, SpatialPhysicalEncoding::Parquet) => OVERLAP_PARQUET_KIND,
    }
}

pub(crate) fn table_manifest(
    role: SpatialArtifactRole,
    encoding: SpatialPhysicalEncoding,
    row_count: u64,
) -> Result<TableManifest, TableManifestError> {
    let (columns, primary_key) = match role {
        SpatialArtifactRole::Footprint => (
            vec![
                scalar_column("patch_id", TableScalarType::Utf8)?,
                scalar_column("origin_x_px", TableScalarType::I64)?,
                scalar_column("origin_y_px", TableScalarType::I64)?,
            ],
            vec!["patch_id".to_owned()],
        ),
        SpatialArtifactRole::Overlap => (
            vec![
                scalar_column("left_patch_id", TableScalarType::Utf8)?,
                scalar_column("right_patch_id", TableScalarType::Utf8)?,
            ],
            vec!["left_patch_id".to_owned(), "right_patch_id".to_owned()],
        ),
    };
    TableManifest::new(
        table_format(encoding),
        encoding_version(role, encoding),
        row_count,
        columns,
        primary_key,
    )
}

pub(crate) fn record_matches(
    record: &ArtifactRecord,
    role: SpatialArtifactRole,
    row_count: u64,
) -> bool {
    if record.schema().id() != schema_id(role)
        || record.schema().version() != 1
        || !record.semantic_metadata().is_empty()
    {
        return false;
    }
    let encoding = match record.content().kind() {
        kind if kind == content_kind(role, SpatialPhysicalEncoding::Arrow) => {
            SpatialPhysicalEncoding::Arrow
        }
        kind if kind == content_kind(role, SpatialPhysicalEncoding::Parquet) => {
            SpatialPhysicalEncoding::Parquet
        }
        _ => return false,
    };
    table_manifest(role, encoding, row_count)
        .is_ok_and(|expected| record.table() == Some(&expected))
}

#[cfg(feature = "parquet")]
pub(crate) fn record_matches_encoding(
    record: &ArtifactRecord,
    role: SpatialArtifactRole,
    encoding: SpatialPhysicalEncoding,
    row_count: u64,
) -> bool {
    record.content().kind() == content_kind(role, encoding)
        && record.schema().id() == schema_id(role)
        && record.schema().version() == 1
        && record.semantic_metadata().is_empty()
        && table_manifest(role, encoding, row_count)
            .is_ok_and(|expected| record.table() == Some(&expected))
}

fn table_format(encoding: SpatialPhysicalEncoding) -> TableFormat {
    match encoding {
        SpatialPhysicalEncoding::Arrow => TableFormat::ArrowIpcFile,
        SpatialPhysicalEncoding::Parquet => TableFormat::ParquetFile,
    }
}

fn scalar_column(name: &str, scalar: TableScalarType) -> Result<TableColumn, TableManifestError> {
    TableColumn::new(name, TableColumnType::Scalar(scalar), false)
}
