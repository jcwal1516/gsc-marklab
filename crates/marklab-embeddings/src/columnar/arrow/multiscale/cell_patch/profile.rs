use arrow::datatypes::Schema;
use arrow_ipc::{Endianness, Type};

use crate::{
    columnar::{
        multiscale::{cell_patch_metadata, validate_cell_patch_domain, CELL_PATCH_METADATA_KEYS},
        ArrowIpcFailure, ColumnarError,
    },
    multiscale::physical::{SpatialArtifactRole, SpatialPhysicalEncoding},
    CellPatchLink,
};

pub(super) const METADATA_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CellPatchArrowProfile {
    Assignment,
    Edge,
}

impl CellPatchArrowProfile {
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

    pub(super) fn field_count(self) -> usize {
        match self {
            Self::Assignment => 6,
            Self::Edge => 4,
        }
    }

    pub(super) fn buffer_count(self) -> usize {
        match self {
            Self::Assignment => 14,
            Self::Edge => 9,
        }
    }
}

pub(super) fn assignment_schema(link: &CellPatchLink) -> Result<Schema, ColumnarError> {
    validate_cell_patch_domain(link)?;
    Ok(Schema::new_with_metadata(
        crate::columnar::multiscale::cell_patch_assignment_fields(),
        metadata(CellPatchArrowProfile::Assignment, link),
    ))
}

pub(super) fn edge_schema(link: &CellPatchLink) -> Result<Schema, ColumnarError> {
    validate_cell_patch_domain(link)?;
    Ok(Schema::new_with_metadata(
        crate::columnar::multiscale::cell_patch_edge_fields(),
        metadata(CellPatchArrowProfile::Edge, link),
    ))
}

pub(super) fn expected_schema(
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
) -> Result<Schema, ColumnarError> {
    match profile {
        CellPatchArrowProfile::Assignment => assignment_schema(link),
        CellPatchArrowProfile::Edge => edge_schema(link),
    }
}

pub(super) fn validate_flatbuffer_schema(
    schema: arrow_ipc::Schema<'_>,
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
) -> Result<(), ColumnarError> {
    validate_schema_shape(schema, profile)?;
    validate_metadata(schema.custom_metadata(), profile, link)?;
    validate_cell_patch_domain(link)
}

fn validate_schema_shape(
    schema: arrow_ipc::Schema<'_>,
    profile: CellPatchArrowProfile,
) -> Result<(), ColumnarError> {
    if schema.endianness() != Endianness::Little
        || schema
            .features()
            .is_some_and(|features| !features.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    let fields = schema
        .fields()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    if fields.len() != profile.field_count() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    match profile {
        CellPatchArrowProfile::Assignment => {
            validate_utf8_field(fields.get(0), "cell_id", false)?;
            validate_utf8_field(fields.get(1), "assignment_status", false)?;
            validate_u64_field(fields.get(2), "anchor_x_bits", false)?;
            validate_u64_field(fields.get(3), "anchor_y_bits", false)?;
            validate_u64_field(fields.get(4), "edge_start", false)?;
            validate_u64_field(fields.get(5), "edge_count", false)
        }
        CellPatchArrowProfile::Edge => {
            validate_u64_field(fields.get(0), "assignment_row", false)?;
            validate_utf8_field(fields.get(1), "patch_id", false)?;
            validate_u64_field(fields.get(2), "weight_numerator", true)?;
            validate_u64_field(fields.get(3), "weight_denominator", true)
        }
    }
}

fn metadata(profile: CellPatchArrowProfile, link: &CellPatchLink) -> [(String, String); 11] {
    cell_patch_metadata(profile.role(), SpatialPhysicalEncoding::Arrow, link)
}

fn validate_metadata(
    entries: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<arrow_ipc::KeyValue<'_>>>>,
    profile: CellPatchArrowProfile,
    link: &CellPatchLink,
) -> Result<(), ColumnarError> {
    let expected = metadata(profile, link);
    let entries =
        entries.ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
    if entries.len() != CELL_PATCH_METADATA_KEYS.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    let mut total = 0_usize;
    for (entry, (expected_key, expected_value)) in entries.iter().zip(expected) {
        let key = entry
            .key()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
        let value = entry
            .value()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
        total = total
            .checked_add(key.len())
            .and_then(|sum| sum.checked_add(value.len()))
            .ok_or(ColumnarError::SizeOverflow)?;
        if key != expected_key || value != expected_value || value.is_empty() {
            return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
        }
    }
    if total > METADATA_LIMIT {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    Ok(())
}

fn validate_utf8_field(
    field: arrow_ipc::Field<'_>,
    name: &str,
    nullable: bool,
) -> Result<(), ColumnarError> {
    if field.name() != Some(name)
        || field.nullable() != nullable
        || field.dictionary().is_some()
        || field
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || field
            .children()
            .is_some_and(|children| !children.is_empty())
        || field.type_type() != Type::Utf8
        || field.type_as_utf_8().is_none()
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(())
}

fn validate_u64_field(
    field: arrow_ipc::Field<'_>,
    name: &str,
    nullable: bool,
) -> Result<(), ColumnarError> {
    let integer = field.type_as_int();
    if field.name() != Some(name)
        || field.nullable() != nullable
        || field.dictionary().is_some()
        || field
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || field
            .children()
            .is_some_and(|children| !children.is_empty())
        || field.type_type() != Type::Int
        || integer.is_none_or(|integer| integer.bitWidth() != 64 || integer.is_signed())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(())
}

pub(super) fn arrow_failure(reason: ArrowIpcFailure) -> ColumnarError {
    ColumnarError::Arrow { reason }
}
