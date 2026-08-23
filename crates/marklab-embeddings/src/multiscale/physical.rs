use marklab_project::{
    ArtifactRecord, TableColumn, TableColumnType, TableFormat, TableManifest, TableManifestError,
    TableScalarType,
};

#[cfg(feature = "parquet")]
use super::EmbeddingEntityKind;

/// Closed physical profiles for the three typed C-05 embedding matrices.
#[cfg(feature = "parquet")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MatrixPhysicalProfile {
    Patch,
    Region,
    Slide,
}

#[cfg(feature = "parquet")]
impl MatrixPhysicalProfile {
    pub(crate) fn from_entity_kind(kind: EmbeddingEntityKind) -> Self {
        match kind {
            EmbeddingEntityKind::Patch => Self::Patch,
            EmbeddingEntityKind::Region => Self::Region,
            EmbeddingEntityKind::Slide => Self::Slide,
        }
    }

    pub(crate) fn entity_kind(self) -> EmbeddingEntityKind {
        match self {
            Self::Patch => EmbeddingEntityKind::Patch,
            Self::Region => EmbeddingEntityKind::Region,
            Self::Slide => EmbeddingEntityKind::Slide,
        }
    }

    pub(crate) fn logical_domain(self) -> &'static [u8] {
        match self {
            Self::Patch => b"marklab-patch-embedding-logical-v1",
            Self::Region => b"marklab-region-embedding-logical-v1",
            Self::Slide => b"marklab-slide-embedding-logical-v1",
        }
    }

    pub(crate) fn id_column(self) -> &'static str {
        match self {
            Self::Patch => "patch_id",
            Self::Region => "region_id",
            Self::Slide => "slide_id",
        }
    }

    pub(crate) fn schema_id(self) -> &'static str {
        match self {
            Self::Patch => "marklab.patch_embedding_table",
            Self::Region => "marklab.region_embedding_table",
            Self::Slide => "marklab.slide_embedding_table",
        }
    }

    pub(crate) fn encoding_version(self, encoding: SpatialPhysicalEncoding) -> &'static str {
        match (self, encoding) {
            (Self::Patch, SpatialPhysicalEncoding::Arrow) => {
                "marklab.arrow-ipc.patch-embedding-table.v1"
            }
            (Self::Patch, SpatialPhysicalEncoding::Parquet) => {
                "marklab.parquet.patch-embedding-table.v1"
            }
            (Self::Region, SpatialPhysicalEncoding::Arrow) => {
                "marklab.arrow-ipc.region-embedding-table.v1"
            }
            (Self::Region, SpatialPhysicalEncoding::Parquet) => {
                "marklab.parquet.region-embedding-table.v1"
            }
            (Self::Slide, SpatialPhysicalEncoding::Arrow) => {
                "marklab.arrow-ipc.slide-embedding-table.v1"
            }
            (Self::Slide, SpatialPhysicalEncoding::Parquet) => {
                "marklab.parquet.slide-embedding-table.v1"
            }
        }
    }

    pub(crate) fn content_kind(self, encoding: SpatialPhysicalEncoding) -> &'static str {
        match (self, encoding) {
            (Self::Patch, SpatialPhysicalEncoding::Arrow) => {
                "application/vnd.marklab.patch-embedding-table.v1+arrow"
            }
            (Self::Patch, SpatialPhysicalEncoding::Parquet) => {
                "application/vnd.marklab.patch-embedding-table.v1+parquet"
            }
            (Self::Region, SpatialPhysicalEncoding::Arrow) => {
                "application/vnd.marklab.region-embedding-table.v1+arrow"
            }
            (Self::Region, SpatialPhysicalEncoding::Parquet) => {
                "application/vnd.marklab.region-embedding-table.v1+parquet"
            }
            (Self::Slide, SpatialPhysicalEncoding::Arrow) => {
                "application/vnd.marklab.slide-embedding-table.v1+arrow"
            }
            (Self::Slide, SpatialPhysicalEncoding::Parquet) => {
                "application/vnd.marklab.slide-embedding-table.v1+parquet"
            }
        }
    }

    pub(crate) fn parquet_root(self) -> &'static str {
        match self {
            Self::Patch => "marklab_patch_embedding_table",
            Self::Region => "marklab_region_embedding_table",
            Self::Slide => "marklab_slide_embedding_table",
        }
    }

    pub(crate) fn table_manifest(
        self,
        encoding: SpatialPhysicalEncoding,
        row_count: u64,
        dimension: u32,
    ) -> Result<TableManifest, TableManifestError> {
        let columns = vec![
            scalar_column(self.id_column(), TableScalarType::Utf8)?,
            TableColumn::new(
                "embedding",
                TableColumnType::FixedSizeList {
                    element: TableScalarType::F32,
                    length: dimension,
                },
                false,
            )?,
            scalar_column("embedding_status", TableScalarType::Utf8)?,
        ];
        TableManifest::new(
            table_format(encoding),
            self.encoding_version(encoding),
            row_count,
            columns,
            vec![self.id_column().to_owned()],
        )
    }

    pub(crate) fn record_matches_encoding(
        self,
        record: &ArtifactRecord,
        encoding: SpatialPhysicalEncoding,
        row_count: u64,
        dimension: u32,
    ) -> bool {
        record.content().kind() == self.content_kind(encoding)
            && record.schema().id() == self.schema_id()
            && record.schema().version() == 1
            && record.semantic_metadata().is_empty()
            && self
                .table_manifest(encoding, row_count, dimension)
                .is_ok_and(|expected| record.table() == Some(&expected))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpatialArtifactRole {
    Footprint,
    Overlap,
    #[cfg(feature = "parquet")]
    CellPatchAssignment,
    #[cfg(feature = "parquet")]
    CellPatchEdge,
    #[cfg(feature = "parquet")]
    PatchRegion,
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
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_ASSIGNMENT_SCHEMA_ID: &str = "marklab.cell_patch_assignment_table";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_EDGE_SCHEMA_ID: &str = "marklab.cell_patch_edge_table";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_ASSIGNMENT_ARROW_ENCODING: &str =
    "marklab.arrow-ipc.cell-patch-assignment-table.v1";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_ASSIGNMENT_PARQUET_ENCODING: &str =
    "marklab.parquet.cell-patch-assignment-table.v1";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_EDGE_ARROW_ENCODING: &str =
    "marklab.arrow-ipc.cell-patch-edge-table.v1";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_EDGE_PARQUET_ENCODING: &str =
    "marklab.parquet.cell-patch-edge-table.v1";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_ASSIGNMENT_ARROW_KIND: &str =
    "application/vnd.marklab.cell-patch-assignment-table.v1+arrow";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_ASSIGNMENT_PARQUET_KIND: &str =
    "application/vnd.marklab.cell-patch-assignment-table.v1+parquet";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_EDGE_ARROW_KIND: &str =
    "application/vnd.marklab.cell-patch-edge-table.v1+arrow";
#[cfg(feature = "parquet")]
pub(crate) const CELL_PATCH_EDGE_PARQUET_KIND: &str =
    "application/vnd.marklab.cell-patch-edge-table.v1+parquet";
#[cfg(feature = "parquet")]
pub(crate) const PATCH_REGION_SCHEMA_ID: &str = "marklab.patch_region_link";
#[cfg(feature = "parquet")]
pub(crate) const PATCH_REGION_ARROW_ENCODING: &str = "marklab.arrow-ipc.patch-region-link.v1";
#[cfg(feature = "parquet")]
pub(crate) const PATCH_REGION_PARQUET_ENCODING: &str = "marklab.parquet.patch-region-link.v1";
#[cfg(feature = "parquet")]
pub(crate) const PATCH_REGION_ARROW_KIND: &str =
    "application/vnd.marklab.patch-region-link.v1+arrow";
#[cfg(feature = "parquet")]
pub(crate) const PATCH_REGION_PARQUET_KIND: &str =
    "application/vnd.marklab.patch-region-link.v1+parquet";

pub(crate) fn schema_id(role: SpatialArtifactRole) -> &'static str {
    match role {
        SpatialArtifactRole::Footprint => FOOTPRINT_SCHEMA_ID,
        SpatialArtifactRole::Overlap => OVERLAP_SCHEMA_ID,
        #[cfg(feature = "parquet")]
        SpatialArtifactRole::CellPatchAssignment => CELL_PATCH_ASSIGNMENT_SCHEMA_ID,
        #[cfg(feature = "parquet")]
        SpatialArtifactRole::CellPatchEdge => CELL_PATCH_EDGE_SCHEMA_ID,
        #[cfg(feature = "parquet")]
        SpatialArtifactRole::PatchRegion => PATCH_REGION_SCHEMA_ID,
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
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchAssignment, SpatialPhysicalEncoding::Arrow) => {
            CELL_PATCH_ASSIGNMENT_ARROW_ENCODING
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchAssignment, SpatialPhysicalEncoding::Parquet) => {
            CELL_PATCH_ASSIGNMENT_PARQUET_ENCODING
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchEdge, SpatialPhysicalEncoding::Arrow) => {
            CELL_PATCH_EDGE_ARROW_ENCODING
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchEdge, SpatialPhysicalEncoding::Parquet) => {
            CELL_PATCH_EDGE_PARQUET_ENCODING
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::PatchRegion, SpatialPhysicalEncoding::Arrow) => {
            PATCH_REGION_ARROW_ENCODING
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::PatchRegion, SpatialPhysicalEncoding::Parquet) => {
            PATCH_REGION_PARQUET_ENCODING
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
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchAssignment, SpatialPhysicalEncoding::Arrow) => {
            CELL_PATCH_ASSIGNMENT_ARROW_KIND
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchAssignment, SpatialPhysicalEncoding::Parquet) => {
            CELL_PATCH_ASSIGNMENT_PARQUET_KIND
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchEdge, SpatialPhysicalEncoding::Arrow) => {
            CELL_PATCH_EDGE_ARROW_KIND
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::CellPatchEdge, SpatialPhysicalEncoding::Parquet) => {
            CELL_PATCH_EDGE_PARQUET_KIND
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::PatchRegion, SpatialPhysicalEncoding::Arrow) => {
            PATCH_REGION_ARROW_KIND
        }
        #[cfg(feature = "parquet")]
        (SpatialArtifactRole::PatchRegion, SpatialPhysicalEncoding::Parquet) => {
            PATCH_REGION_PARQUET_KIND
        }
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
        #[cfg(feature = "parquet")]
        SpatialArtifactRole::CellPatchAssignment => (
            vec![
                scalar_column("cell_id", TableScalarType::Utf8)?,
                scalar_column("assignment_status", TableScalarType::Utf8)?,
                scalar_column("anchor_x_bits", TableScalarType::U64)?,
                scalar_column("anchor_y_bits", TableScalarType::U64)?,
                scalar_column("edge_start", TableScalarType::U64)?,
                scalar_column("edge_count", TableScalarType::U64)?,
            ],
            vec!["cell_id".to_owned()],
        ),
        #[cfg(feature = "parquet")]
        SpatialArtifactRole::CellPatchEdge => (
            vec![
                scalar_column("assignment_row", TableScalarType::U64)?,
                scalar_column("patch_id", TableScalarType::Utf8)?,
                nullable_scalar_column("weight_numerator", TableScalarType::U64)?,
                nullable_scalar_column("weight_denominator", TableScalarType::U64)?,
            ],
            vec!["assignment_row".to_owned(), "patch_id".to_owned()],
        ),
        #[cfg(feature = "parquet")]
        SpatialArtifactRole::PatchRegion => (
            vec![
                scalar_column("patch_id", TableScalarType::Utf8)?,
                scalar_column("region_id", TableScalarType::Utf8)?,
                scalar_column("relation", TableScalarType::Utf8)?,
                scalar_column("overlap_numerator", TableScalarType::U64)?,
                scalar_column("overlap_denominator", TableScalarType::U64)?,
            ],
            vec!["patch_id".to_owned(), "region_id".to_owned()],
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

#[cfg(feature = "parquet")]
fn nullable_scalar_column(
    name: &str,
    scalar: TableScalarType,
) -> Result<TableColumn, TableManifestError> {
    TableColumn::new(name, TableColumnType::Scalar(scalar), true)
}
