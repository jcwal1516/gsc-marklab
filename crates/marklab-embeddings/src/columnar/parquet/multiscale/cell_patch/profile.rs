use arrow::datatypes::Schema;
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::{
    columnar::{
        multiscale::{cell_patch_metadata, validate_cell_patch_domain},
        parquet::profile::writer_properties_with_metadata,
        ColumnarError,
    },
    multiscale::physical::{SpatialArtifactRole, SpatialPhysicalEncoding},
    CellPatchLink,
};

pub(in crate::columnar::parquet) const ASSIGNMENT_ROOT: &str =
    "marklab_cell_patch_assignment_table";
pub(in crate::columnar::parquet) const EDGE_ROOT: &str = "marklab_cell_patch_edge_table";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::columnar::parquet) enum CellPatchParquetProfile {
    Assignment,
    Edge,
}

impl CellPatchParquetProfile {
    pub(in crate::columnar::parquet) fn role(self) -> SpatialArtifactRole {
        match self {
            Self::Assignment => SpatialArtifactRole::CellPatchAssignment,
            Self::Edge => SpatialArtifactRole::CellPatchEdge,
        }
    }

    pub(in crate::columnar::parquet) fn row_count(self, link: &CellPatchLink) -> usize {
        match self {
            Self::Assignment => link.assignment_count(),
            Self::Edge => link.edge_count(),
        }
    }

    pub(in crate::columnar::parquet) fn column_count(self) -> usize {
        match self {
            Self::Assignment => 6,
            Self::Edge => 4,
        }
    }

    pub(in crate::columnar::parquet) fn root(self) -> &'static str {
        match self {
            Self::Assignment => ASSIGNMENT_ROOT,
            Self::Edge => EDGE_ROOT,
        }
    }
}

pub(in crate::columnar::parquet) fn assignment_schema() -> Schema {
    Schema::new(crate::columnar::multiscale::cell_patch_assignment_fields())
}

pub(in crate::columnar::parquet) fn edge_schema() -> Schema {
    Schema::new(crate::columnar::multiscale::cell_patch_edge_fields())
}

pub(in crate::columnar::parquet) fn expected_schema(profile: CellPatchParquetProfile) -> Schema {
    match profile {
        CellPatchParquetProfile::Assignment => assignment_schema(),
        CellPatchParquetProfile::Edge => edge_schema(),
    }
}

pub(in crate::columnar::parquet) fn writer_properties(
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<WriterProperties, ColumnarError> {
    validate_cell_patch_domain(link)?;
    let metadata = cell_patch_metadata(profile.role(), SpatialPhysicalEncoding::Parquet, link)
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, Some(value)))
        .collect();
    Ok(writer_properties_with_metadata(metadata))
}
