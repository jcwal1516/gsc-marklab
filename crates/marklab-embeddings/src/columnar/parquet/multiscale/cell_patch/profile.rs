use arrow::datatypes::{DataType, Field, Schema};
use parquet::{file::properties::WriterProperties, format::KeyValue};

use crate::{
    columnar::{
        multiscale::{cell_patch_metadata, validate_cell_patch_domain},
        parquet::profile::writer_properties_with_metadata,
        MultiscaleColumnarError,
    },
    multiscale::physical::{SpatialArtifactRole, SpatialPhysicalEncoding},
    CellPatchLink,
};

pub(super) const ASSIGNMENT_ROOT: &str = "marklab_cell_patch_assignment_table";
pub(super) const EDGE_ROOT: &str = "marklab_cell_patch_edge_table";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CellPatchParquetProfile {
    Assignment,
    Edge,
}

impl CellPatchParquetProfile {
    pub(super) fn role(self) -> SpatialArtifactRole {
        match self {
            Self::Assignment => SpatialArtifactRole::CellPatchAssignment,
            Self::Edge => SpatialArtifactRole::CellPatchEdge,
        }
    }

    pub(super) fn row_count(self, link: &CellPatchLink) -> usize {
        match self {
            Self::Assignment => link.assignment_count(),
            Self::Edge => link.edge_count(),
        }
    }

    pub(super) fn column_count(self) -> usize {
        match self {
            Self::Assignment => 6,
            Self::Edge => 4,
        }
    }

    pub(super) fn root(self) -> &'static str {
        match self {
            Self::Assignment => ASSIGNMENT_ROOT,
            Self::Edge => EDGE_ROOT,
        }
    }
}

pub(super) fn assignment_schema() -> Schema {
    Schema::new(vec![
        Field::new("cell_id", DataType::Utf8, false),
        Field::new("assignment_status", DataType::Utf8, false),
        Field::new("anchor_x_bits", DataType::UInt64, false),
        Field::new("anchor_y_bits", DataType::UInt64, false),
        Field::new("edge_start", DataType::UInt64, false),
        Field::new("edge_count", DataType::UInt64, false),
    ])
}

pub(super) fn edge_schema() -> Schema {
    Schema::new(vec![
        Field::new("assignment_row", DataType::UInt64, false),
        Field::new("patch_id", DataType::Utf8, false),
        Field::new("weight_numerator", DataType::UInt64, true),
        Field::new("weight_denominator", DataType::UInt64, true),
    ])
}

pub(super) fn expected_schema(profile: CellPatchParquetProfile) -> Schema {
    match profile {
        CellPatchParquetProfile::Assignment => assignment_schema(),
        CellPatchParquetProfile::Edge => edge_schema(),
    }
}

pub(super) fn writer_properties(
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<WriterProperties, MultiscaleColumnarError> {
    validate_cell_patch_domain(link)?;
    let metadata = cell_patch_metadata(profile.role(), SpatialPhysicalEncoding::Parquet, link)
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, Some(value)))
        .collect();
    Ok(writer_properties_with_metadata(metadata))
}
