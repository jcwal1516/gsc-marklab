use thiserror::Error;

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
    /// Bounded Arrow IPC preflight rejected the physical declaration.
    #[error("Arrow IPC file violates the canonical profile: {reason:?}")]
    Arrow {
        /// Closed rejection reason.
        reason: ArrowIpcFailure,
    },
}
