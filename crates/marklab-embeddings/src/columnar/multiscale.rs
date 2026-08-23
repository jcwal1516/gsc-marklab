use marklab_project::ContentDigest;
use thiserror::Error;

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

/// Closed Arrow rejection reasons for C-05 footprint and overlap profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialArrowFailure {
    /// Leading or trailing Arrow file magic or fixed header padding is invalid.
    InvalidMagic,
    /// Footer length or end-of-stream framing is invalid.
    InvalidFooterLength,
    /// The bounded footer is malformed or outside verifier limits.
    InvalidFooter,
    /// The IPC version is not exact metadata version V5.
    UnsupportedVersion,
    /// Dictionaries, compression, features, or custom message metadata were declared.
    ForbiddenFeature,
    /// Exact field order, type, nullability, child, or field metadata differs.
    InvalidSchema,
    /// Exact sorted application metadata differs.
    InvalidApplicationMetadata,
    /// A record block is negative, overflowing, misaligned, overlapping, or out of range.
    InvalidBlock,
    /// A bounded schema or record message is malformed.
    InvalidMessage,
    /// Batch or aggregate row counts differ from the canonical domain value.
    InvalidRowCount,
    /// Field-node declarations differ from the exact profile.
    InvalidFieldNodes,
    /// Buffer ranges, offsets, validity bits, or padding differ from the exact profile.
    InvalidBuffers,
    /// Decoded patch rows differ from the canonical domain order or values.
    InvalidCanonicalRows,
    /// A bounded artifact seek or read failed.
    ArtifactRead,
    /// Stock Arrow decode failed after raw preflight.
    StockDecode,
}

/// Closed Parquet rejection reasons for C-05 footprint and overlap profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialParquetFailure {
    /// Leading or trailing `PAR1` magic is invalid.
    InvalidMagic,
    /// Footer length is invalid or out of range.
    InvalidFooterLength,
    /// The bounded compact-Thrift footer is malformed or noncanonical.
    InvalidFooter,
    /// Physical schema, root, annotations, or field order differs.
    InvalidSchema,
    /// Writer or exact sorted application metadata differs.
    InvalidMetadata,
    /// File or row-group counts differ from the canonical domain value.
    InvalidRowCount,
    /// A row-group or column-chunk declaration is invalid.
    InvalidRowGroup,
    /// A bounded page header or page range is invalid.
    InvalidPage,
    /// Encoding, compression, dictionary, statistics, index, bloom, or encryption is forbidden.
    ForbiddenFeature,
    /// Decoded rows differ from the canonical domain order or values.
    InvalidCanonicalRows,
    /// Stock Parquet decode failed after raw preflight.
    StockDecode,
    /// A bounded artifact seek or read failed.
    ArtifactRead,
}

/// Bounded, redacted C-05 spatial physical-format failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum MultiscaleColumnarError {
    /// Encoded bytes exceed the caller's file limit.
    #[error("spatial columnar file bytes {observed} exceed budget {maximum}")]
    FileByteBudgetExceeded {
        /// Observed or attempted bytes.
        observed: u64,
        /// Caller-provided maximum.
        maximum: u64,
    },
    /// A retained allocation estimate exceeds the caller's limit.
    #[error("spatial columnar retained bytes {required} exceed budget {maximum}")]
    RetainedByteBudgetExceeded {
        /// Conservatively required bytes.
        required: usize,
        /// Caller-provided maximum.
        maximum: usize,
    },
    /// One batch or row group exceeds the caller's limit.
    #[error("spatial columnar row-group bytes {required} exceed budget {maximum}")]
    RowGroupByteBudgetExceeded {
        /// Required physical bytes.
        required: usize,
        /// Caller-provided maximum.
        maximum: usize,
    },
    /// Decoded values, offsets, validity, or text exceed the caller's limit.
    #[error("spatial columnar decoded bytes {required} exceed budget {maximum}")]
    DecodedByteBudgetExceeded {
        /// Required decoded bytes.
        required: u64,
        /// Caller-provided maximum.
        maximum: u64,
    },
    /// A checked size, count, or offset calculation overflowed.
    #[error("spatial columnar size calculation overflowed")]
    SizeOverflow,
    /// Exact logical, structural-graph, artifact, or dependency bindings disagree.
    #[error("spatial columnar artifact binding mismatch")]
    ArtifactBindingMismatch,
    /// A fallible bounded allocation failed.
    #[error("spatial columnar allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Requested allocation size.
        requested: usize,
    },
    /// Canonical Arrow encoding failed without exposing library or sink details.
    #[error("canonical spatial Arrow writer failed")]
    ArrowWriter,
    /// Canonical Parquet encoding failed without exposing library or sink details.
    #[error("canonical spatial Parquet writer failed")]
    ParquetWriter,
    /// Raw Arrow preflight or stock validation rejected the exact profile.
    #[error("Arrow spatial table violates the canonical profile: {reason:?}")]
    Arrow {
        /// Closed rejection category.
        reason: SpatialArrowFailure,
    },
    /// Raw Parquet preflight or stock validation rejected the exact profile.
    #[error("Parquet spatial table violates the canonical profile: {reason:?}")]
    Parquet {
        /// Closed rejection category.
        reason: SpatialParquetFailure,
    },
}

pub(crate) fn enforce_file_budget(
    observed: u64,
    budgets: super::EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    if observed > budgets.maximum_file_bytes() {
        return Err(MultiscaleColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn enforce_decoded_budget(
    required: u64,
    budgets: super::EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    if required > budgets.maximum_decoded_bytes() {
        return Err(MultiscaleColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_decoded_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn enforce_row_group_budget(
    required: usize,
    budgets: super::EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    if required > budgets.maximum_row_group_bytes() {
        return Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum: budgets.maximum_row_group_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn enforce_retained_budget(
    required: usize,
    budgets: super::EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(MultiscaleColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn validate_footprint_domain(
    expected: &crate::ExpectedPatchSet,
    context: &crate::PatchEmbeddingContext,
    footprints: &crate::PatchFootprintSet,
) -> Result<(), MultiscaleColumnarError> {
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
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn validate_overlap_domain(
    expected: &crate::ExpectedPatchSet,
    context: &crate::PatchEmbeddingContext,
    footprints: &crate::PatchFootprintSet,
    overlap: &crate::PatchOverlapGraph,
) -> Result<(), MultiscaleColumnarError> {
    validate_footprint_domain(expected, context, footprints)?;
    if overlap.expected_patches_artifact_id() == overlap.patch_footprints_artifact_id()
        || overlap.patch_footprints_artifact_id() == footprints.patch_context_artifact_id()
        || overlap.expected_patches_artifact_id() != footprints.expected_patches_artifact_id()
        || overlap.expected_patches_logical_digest() != expected.logical_digest()
        || overlap.patch_footprints_logical_digest() != footprints.logical_digest()
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}
