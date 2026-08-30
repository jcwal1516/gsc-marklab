use thiserror::Error;

use super::EmbeddingColumnarBudgets;

/// Closed Arrow IPC profile rejection reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrowIpcFailure {
    /// Leading or trailing Arrow file magic is absent.
    InvalidMagic,
    /// The signed footer length is zero, negative, oversized, or outside the file.
    InvalidFooterLength,
    /// The bounded footer FlatBuffer is malformed or exceeds verifier limits.
    InvalidFooter,
    /// IPC metadata is not exact version V5.
    UnsupportedVersion,
    /// Dictionary blocks are present.
    DictionaryBatch,
    /// File-level custom metadata is present.
    FileCustomMetadata,
    /// The exact field order, type, nullability, child, or field metadata differs.
    InvalidSchema,
    /// Application schema metadata differs from the exact bounded map.
    InvalidApplicationMetadata,
    /// A record-batch block has a negative, overflowing, misaligned, or overlapping range.
    InvalidBlock,
    /// A record-batch message or its continuation framing is invalid.
    InvalidMessage,
    /// A batch row count or aggregate row count differs from the frozen declaration.
    InvalidRowCount,
    /// Field-node declarations are inconsistent with the exact schema and dimension.
    InvalidFieldNodes,
    /// Body-buffer declarations are negative, overlapping, misaligned, or out of range.
    InvalidBuffers,
    /// A bounded seek or read failed without exposing the underlying artifact path.
    ArtifactRead,
    /// The stock Arrow decoder rejected bytes that passed raw preflight.
    StockDecode,
    /// Decoded cell identities are invalid, duplicated, reordered, or unexpected.
    InvalidCellOrder,
    /// A decoded extraction-validity token is unknown or disagrees with row linkage.
    InvalidStatus,
    /// A component is non-finite, noncanonical zero, or invalid for its status.
    InvalidComponent,
    /// Decoded canonical content does not match the declared logical digest.
    LogicalDigestMismatch,
}

/// Closed, privacy-safe rejection code for canonical Parquet inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParquetFailure {
    /// Leading or trailing `PAR1` magic is absent.
    InvalidMagic,
    /// Footer length is negative, overflowing, or outside the file.
    InvalidFooterLength,
    /// Compact-Thrift footer syntax or bounds are invalid.
    InvalidFooter,
    /// Physical Parquet schema differs from the frozen profile.
    InvalidSchema,
    /// Application or writer metadata differs from the frozen profile.
    InvalidMetadata,
    /// Declared file or row-group row counts are inconsistent.
    InvalidRowCount,
    /// A row-group declaration violates ordering, bounds, or size limits.
    InvalidRowGroup,
    /// A column-chunk declaration violates the exact profile.
    InvalidColumnChunk,
    /// A compact-Thrift page header is malformed or exceeds its limits.
    InvalidPageHeader,
    /// A page range, count, or profile declaration is invalid.
    InvalidPage,
    /// A forbidden value or level encoding was declared.
    UnsupportedEncoding,
    /// A forbidden compression codec was declared.
    UnsupportedCompression,
    /// Statistics, indexes, dictionaries, bloom filters, or encryption were declared.
    ForbiddenAuxiliaryData,
    /// Canonical cell identifiers are absent, duplicated, or out of order.
    InvalidCellOrder,
    /// Status and nullable linkage semantics disagree.
    InvalidStatus,
    /// A vector component, source row, or hidden filler is invalid.
    InvalidComponent,
    /// Decoded rows do not reproduce the declared logical digest.
    LogicalDigestMismatch,
    /// Stock Parquet/Arrow decoding failed after bounded preflight.
    StockDecode,
    /// A bounded artifact range read failed.
    ArtifactRead,
}

/// Bounded, redacted embedding physical-format failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum EmbeddingColumnarError {
    /// Encoded bytes exceed the caller's file limit.
    #[error("columnar file bytes {observed} exceed budget {maximum}")]
    FileByteBudgetExceeded {
        /// Observed or attempted encoded bytes.
        observed: u64,
        /// Caller-provided maximum.
        maximum: u64,
    },
    /// A retained allocation estimate exceeds the caller's operation limit.
    #[error("columnar retained bytes {required} exceed budget {maximum}")]
    RetainedByteBudgetExceeded {
        /// Conservatively required bytes.
        required: usize,
        /// Caller-provided maximum.
        maximum: usize,
    },
    /// One physical record batch or row group exceeds the caller's limit.
    #[error("columnar row-group bytes {required} exceed budget {maximum}")]
    RowGroupByteBudgetExceeded {
        /// Required physical bytes for the batch or row group.
        required: usize,
        /// Caller-provided maximum.
        maximum: usize,
    },
    /// Decoded value, offset, validity, or string bytes exceed the caller's limit.
    #[error("columnar decoded bytes {required} exceed budget {maximum}")]
    DecodedByteBudgetExceeded {
        /// Required decoded bytes.
        required: u64,
        /// Caller-provided maximum.
        maximum: u64,
    },
    /// A checked physical offset, length, row, or component calculation overflowed.
    #[error("columnar size calculation overflowed")]
    SizeOverflow,
    /// Explicit artifact IDs or logical identities do not match the domain table.
    #[error("columnar artifact binding mismatch")]
    ArtifactBindingMismatch,
    /// A fallible bounded allocation failed after all declared sizes were checked.
    #[error("columnar allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Requested allocation bytes.
        requested: usize,
    },
    /// Canonical Arrow writer construction or output failed without exposing library text.
    #[error("canonical Arrow writer failed")]
    ArrowWriter,
    /// Canonical Parquet writer construction or output failed without exposing library text.
    #[error("canonical Parquet writer failed")]
    ParquetWriter,
    /// Bounded Arrow IPC preflight rejected the physical declaration.
    #[error("Arrow IPC file violates the canonical profile: {reason:?}")]
    Arrow {
        /// Closed rejection reason.
        reason: ArrowIpcFailure,
    },
    /// Bounded Parquet preflight rejected the physical declaration.
    #[error("Parquet file violates the canonical profile: {reason:?}")]
    Parquet {
        /// Closed rejection reason.
        reason: ParquetFailure,
    },
}

pub(crate) fn enforce_file_budget(
    observed: u64,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if observed > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn enforce_retained_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn enforce_row_group_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_row_group_bytes() {
        return Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum: budgets.maximum_row_group_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn enforce_decoded_budget(
    required: u64,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_decoded_bytes() {
        return Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_decoded_bytes(),
        });
    }
    Ok(())
}
