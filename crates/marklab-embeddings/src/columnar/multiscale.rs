use marklab_project::ContentDigest;

mod cell_patch;
pub(crate) mod matrix;
mod patch_region;

pub(crate) use crate::multiscale::physical::MatrixPhysicalProfile;
pub(crate) use cell_patch::{
    assignment_decoded_bytes, cell_patch_assignment_fields, cell_patch_dependencies,
    cell_patch_edge_fields, cell_patch_metadata, edge_decoded_bytes, validate_cell_patch_domain,
    CELL_PATCH_METADATA_KEYS,
};
pub(crate) use matrix::{
    matrix_decoded_bytes, matrix_dependencies, matrix_metadata, matrix_schema_fields,
    validate_matrix_domain, MatrixPhysicalAccumulator, MultiscaleMatrixTable, MATRIX_METADATA_KEYS,
};
pub(crate) use patch_region::{
    patch_region_decoded_bytes, patch_region_dependencies, patch_region_fields,
    patch_region_metadata, validate_patch_region_domain, PATCH_REGION_METADATA_KEYS,
};

/// Exact encoded identity emitted by one canonical spatial-table writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpatialColumnarWriteSummary {
    content_digest: ContentDigest,
    encoded_byte_len: u64,
    row_count: u64,
}

impl SpatialColumnarWriteSummary {
    pub(crate) fn new(
        content_digest: ContentDigest,
        encoded_byte_len: u64,
        row_count: u64,
    ) -> Self {
        Self {
            content_digest,
            encoded_byte_len,
            row_count,
        }
    }

    /// SHA-256 of the exact encoded bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }

    /// Exact encoded byte length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// Number of encoded canonical rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }
}

pub(crate) use super::error::{
    enforce_decoded_budget, enforce_file_budget, enforce_retained_budget, enforce_row_group_budget,
};
pub(crate) use super::{ArrowIpcFailure, ColumnarError};

pub(crate) fn validate_footprint_domain(
    expected: &crate::ExpectedPatchSet,
    context: &crate::PatchEmbeddingContext,
    footprints: &crate::PatchFootprintSet,
) -> Result<(), ColumnarError> {
    if expected.owning_slide_id() != context.owning_slide_id()
        || footprints.expected_patches_artifact_id() == footprints.patch_context_artifact_id()
        || expected.logical_digest() != footprints.expected_patches_logical_digest()
        || context.logical_digest() != footprints.patch_context_logical_digest()
        || expected.ids().len() != footprints.footprints().len()
        || expected
            .ids()
            .iter()
            .zip(footprints.footprints())
            .any(|(expected_id, footprint)| expected_id != footprint.patch_id())
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn validate_overlap_domain(
    expected: &crate::ExpectedPatchSet,
    context: &crate::PatchEmbeddingContext,
    footprints: &crate::PatchFootprintSet,
    overlap: &crate::PatchOverlapGraph,
) -> Result<(), ColumnarError> {
    validate_footprint_domain(expected, context, footprints)?;
    if overlap.expected_patches_artifact_id() == overlap.patch_footprints_artifact_id()
        || overlap.patch_footprints_artifact_id() == footprints.patch_context_artifact_id()
        || overlap.expected_patches_artifact_id() != footprints.expected_patches_artifact_id()
        || overlap.expected_patches_logical_digest() != expected.logical_digest()
        || overlap.patch_footprints_logical_digest() != footprints.logical_digest()
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn footprint_fields() -> Vec<arrow::datatypes::Field> {
    use arrow::datatypes::{DataType, Field};
    vec![
        Field::new("patch_id", DataType::Utf8, false),
        Field::new("origin_x_px", DataType::Int64, false),
        Field::new("origin_y_px", DataType::Int64, false),
    ]
}

pub(crate) fn overlap_fields() -> Vec<arrow::datatypes::Field> {
    use arrow::datatypes::{DataType, Field};
    vec![
        Field::new("left_patch_id", DataType::Utf8, false),
        Field::new("right_patch_id", DataType::Utf8, false),
    ]
}
