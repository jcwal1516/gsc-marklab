use thiserror::Error;

/// Closed source-file roles used by privacy-safe resource errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFileKind {
    /// CellViT vector matrix in NPY format.
    Npy,
    /// Frozen CellViT source-cell CSV.
    Csv,
    /// Bounded reconciliation-only source manifest.
    Manifest,
}

/// Exact frozen CSV fields, in wire order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellVitCsvField {
    /// Source-local cell identifier.
    CellId,
    /// Source-local case identifier.
    CaseId,
    /// Source-local specimen identifier.
    SpecimenId,
    /// Source-local timepoint identifier.
    Timepoint,
    /// Source-local fragment identifier.
    FragmentId,
    /// Source-local region-of-interest identifier.
    RoiId,
    /// Zero-based source cell row.
    NativeRow,
    /// Zero-based source embedding row.
    EmbeddingRow,
    /// X image coordinate in pixels.
    XPx,
    /// Y image coordinate in pixels.
    YPx,
    /// X physical coordinate in micrometres.
    XUm,
    /// Y physical coordinate in micrometres.
    YUm,
    /// Source cell-type integer identifier.
    CellTypeId,
    /// Source cell-type label.
    CellTypeLabel,
    /// Source type probability.
    TypeProbability,
    /// Nucleus area in square micrometres.
    NucleusAreaUm2,
    /// Nucleus perimeter in micrometres.
    NucleusPerimeterUm,
    /// Nucleus eccentricity.
    Eccentricity,
    /// Nucleus solidity.
    Solidity,
    /// Nucleus circularity.
    Circularity,
    /// Exact source quality-control predicate.
    QcPass,
    /// Source 500-micrometre block identifier.
    Block500Id,
    /// Frozen source split token.
    Split,
}

impl CellVitCsvField {
    #[cfg(feature = "csv")]
    pub(crate) fn index(self) -> usize {
        match self {
            Self::CellId => 0,
            Self::CaseId => 1,
            Self::SpecimenId => 2,
            Self::Timepoint => 3,
            Self::FragmentId => 4,
            Self::RoiId => 5,
            Self::NativeRow => 6,
            Self::EmbeddingRow => 7,
            Self::XPx => 8,
            Self::YPx => 9,
            Self::XUm => 10,
            Self::YUm => 11,
            Self::CellTypeId => 12,
            Self::CellTypeLabel => 13,
            Self::TypeProbability => 14,
            Self::NucleusAreaUm2 => 15,
            Self::NucleusPerimeterUm => 16,
            Self::Eccentricity => 17,
            Self::Solidity => 18,
            Self::Circularity => 19,
            Self::QcPass => 20,
            Self::Block500Id => 21,
            Self::Split => 22,
        }
    }
}

/// Closed source CSV rejection reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CsvFailure {
    /// RFC 4180 quote or record-termination syntax is malformed.
    InvalidRecordSyntax,
    /// Header bytes do not exactly equal the frozen ordered profile.
    WrongHeader,
    /// A data record does not have exactly 23 fields.
    WrongFieldCount,
    /// A required field is empty.
    EmptyField,
    /// A field is not valid UTF-8.
    InvalidUtf8,
    /// A bounded source identifier or label is invalid.
    InvalidIdentifier,
    /// A source-local cell identifier is repeated.
    DuplicateSourceCellId,
    /// A canonical unsigned integer is malformed or out of range.
    InvalidUnsigned,
    /// A source decimal violates the frozen non-exponent grammar.
    InvalidDecimal,
    /// A valid source decimal lies outside its closed numeric range.
    OutOfRange,
    /// The quality-control field is not exact `True`.
    InvalidQcPass,
    /// A bounded ASCII token is malformed.
    InvalidToken,
    /// A declared source row differs from the zero-based CSV data-row index.
    SourceRowMismatch,
    /// The profile has no data rows.
    EmptySource,
}

/// Closed structural NPY rejection reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NpyFailure {
    /// The six-byte NPY magic prefix is absent.
    InvalidMagic,
    /// The NPY major/minor version is not frozen version 1.0 or 2.0.
    UnsupportedVersion,
    /// The version-specific header length is zero, truncated, or inconsistent.
    InvalidHeaderLength,
    /// Header bytes are not bounded ASCII ending in one newline.
    InvalidHeaderEncoding,
    /// The complete NPY prefix and header are not aligned to 16 bytes.
    InvalidHeaderAlignment,
    /// The header violates the closed dictionary grammar.
    InvalidHeaderGrammar,
    /// A required header key occurs more than once.
    DuplicateHeaderKey,
    /// A header key is outside the exact three-key vocabulary.
    UnknownHeaderKey,
    /// The dtype descriptor is not exact little-endian binary32.
    UnsupportedDescriptor,
    /// The source declares Fortran ordering.
    FortranOrder,
    /// The shape is not a valid two-dimensional canonical integer tuple.
    InvalidShape,
    /// The matrix width is not the frozen raw CellViT dimension.
    WrongDimension,
    /// The declared row count exceeds the frozen source-table maximum.
    RowLimitExceeded,
    /// Bytes between the header dictionary and final newline are not spaces.
    InvalidPadding,
    /// The encoded payload is truncated or has trailing extension bytes.
    PayloadLengthMismatch,
}

/// Closed promotion-import rejection reasons that do not expose source identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportFailure {
    /// A promotion import cannot construct a table without expected cells.
    EmptyExpectedCells,
    /// Explicit artifact roles are duplicated or disagree across supplied inputs.
    ArtifactBindingMismatch,
    /// A bound managed source artifact is unavailable or fails integrity verification.
    ArtifactUnavailable,
    /// The identity-map artifact does not exactly bind the CSV domain and expected-cell range.
    IdentityMapMismatch,
    /// A completed verified artifact graph does not bind this exact import candidate.
    VerifiedGraphMismatch,
    /// One or more expected cells are absent from the supplied cohort hierarchy.
    HierarchyMismatch,
    /// Validated source inputs could not satisfy a downstream embedding-domain invariant.
    DomainConstruction,
}

/// Closed reconciliation-manifest rejection reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestFailure {
    /// JSON syntax, duplicate keys, unknown keys, or a required JSON type is invalid.
    InvalidJson,
    /// A fixed schema, scalar, source count, or source-profile value differs.
    InvalidProfile,
    /// A required source content digest is not exact lowercase SHA-256 text.
    InvalidHash,
    /// A bounded reconciliation-only source string is empty, too long, or contains controls.
    InvalidSensitiveString,
    /// Manifest `n_cells` differs from the validated NPY/CSV row count.
    RowCountMismatch,
}

/// Closed multi-bundle reconciliation rejection reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationFailure {
    /// No validated source bundle was supplied.
    Empty,
    /// Adding another bundle would exceed the explicit bundle-count maximum.
    BundleLimitExceeded,
    /// The exact NPY/CSV/manifest digest and shape tuple was already supplied.
    DuplicateBundle,
    /// Validated bundles disagree on their fixed embedding dimension.
    DimensionMismatch,
}

/// Bounded reader operation that failed without exposing a path or source value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIoOperation {
    /// Inspect encoded file length.
    InspectLength,
    /// Seek to a required encoded offset.
    Seek,
    /// Read bounded encoded bytes.
    Read,
}

/// Privacy-safe I/O failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIoFailure {
    /// The reader ended before its previously declared length.
    UnexpectedEnd,
    /// The reader rejected or failed an operation for another reason.
    Other,
}

/// Bounded, privacy-safe source-bundle parsing or validation failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SourceBundleError {
    /// One encoded source exceeds its caller-provided file budget.
    #[error("{file:?} bytes {observed} exceed file budget {maximum}")]
    FileByteBudgetExceeded {
        /// Encoded source role.
        file: SourceFileKind,
        /// Observed encoded bytes.
        observed: u64,
        /// Caller-provided maximum.
        maximum: u64,
    },
    /// The declared NPY header exceeds the frozen header maximum.
    #[error("NPY header bytes {observed} exceed maximum {maximum}")]
    HeaderByteBudgetExceeded {
        /// Declared header bytes.
        observed: u64,
        /// Frozen maximum header bytes.
        maximum: u64,
    },
    /// One logical RFC 4180 record exceeds the fixed record-byte maximum.
    #[error("CSV record {record} bytes {observed} exceed maximum {maximum}")]
    CsvRecordByteBudgetExceeded {
        /// Zero-based logical record, including header record zero.
        record: u64,
        /// Observed encoded record bytes.
        observed: u64,
        /// Frozen maximum encoded record bytes.
        maximum: u64,
    },
    /// Parsed source metadata exceeds its caller-provided retained-memory budget.
    #[error("source retained bytes {required} exceed budget {maximum}")]
    RetainedByteBudgetExceeded {
        /// Conservatively required retained bytes.
        required: usize,
        /// Caller-provided maximum retained bytes.
        maximum: usize,
    },
    /// The declared decoded vector payload exceeds its caller-provided budget.
    #[error("decoded source bytes {required} exceed budget {maximum}")]
    DecodedByteBudgetExceeded {
        /// Required decoded bytes.
        required: u64,
        /// Caller-provided maximum.
        maximum: u64,
    },
    /// A checked source size or offset calculation overflowed.
    #[error("source-bundle size calculation overflowed")]
    SizeOverflow,
    /// A bounded allocation failed after its size was validated.
    #[error("source-bundle allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Requested allocation bytes.
        requested: usize,
    },
    /// NPY bytes violate one closed structural rule.
    #[error("NPY source violates the frozen profile: {reason:?}")]
    Npy {
        /// Closed structural reason.
        reason: NpyFailure,
    },
    /// CSV bytes violate one closed profile rule.
    #[error("CSV source violates the frozen profile at row {row:?}, field {field:?}: {reason:?}")]
    Csv {
        /// Zero-based data row, excluding the header, when available.
        row: Option<u64>,
        /// Exact affected field, when available.
        field: Option<CellVitCsvField>,
        /// Closed rejection reason.
        reason: CsvFailure,
    },
    /// NPY and CSV declare different source row counts.
    #[error("source row count mismatch: NPY {npy}, CSV {csv}")]
    RowCountMismatch {
        /// Validated NPY row count.
        npy: u64,
        /// Validated CSV data-row count.
        csv: u64,
    },
    /// Source inputs cannot be promoted under the explicit identity and artifact bindings.
    #[error("source promotion import failed: {reason:?}")]
    Import {
        /// Closed privacy-safe import rejection reason.
        reason: ImportFailure,
    },
    /// A bounded reconciliation-only manifest violates its frozen profile.
    #[error("source reconciliation manifest failed: {reason:?}")]
    Manifest {
        /// Closed privacy-safe manifest rejection reason.
        reason: ManifestFailure,
    },
    /// Aggregate reconciliation cannot be completed.
    #[error("source-bundle reconciliation failed: {reason:?}")]
    Reconciliation {
        /// Closed aggregate rejection reason.
        reason: ReconciliationFailure,
    },
    /// One source component is NaN or infinite.
    #[error("NPY source component is non-finite at row {row}, column {column}")]
    NonFiniteSourceComponent {
        /// Zero-based source vector row.
        row: u64,
        /// Zero-based component column.
        column: u32,
    },
    /// A requested borrowed source component is outside the validated matrix.
    #[error("NPY source component index is outside the validated matrix")]
    SourceValueOutOfBounds,
    /// A reader failed without exposing implementation text or an ambient path.
    #[error("source reader failed during {operation:?}: {reason:?}")]
    Io {
        /// Failed bounded operation.
        operation: SourceIoOperation,
        /// Redacted failure category.
        reason: SourceIoFailure,
    },
}
