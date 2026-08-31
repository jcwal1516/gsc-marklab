use std::collections::BTreeMap;

use thiserror::Error;

use crate::{ArtifactRef, ContentDigest};

mod identity;
mod locator;
mod record;
mod table;

const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_ARTIFACT_KEY_BYTES: usize = 4_096;
const MAX_KEY_COMPONENT_BYTES: usize = 255;
const MAX_OBJECT_VERSION_BYTES: usize = 255;
const MAX_METADATA_ENTRIES: usize = 256;
const MAX_METADATA_VALUE_BYTES: usize = 4_096;
const MAX_TABLE_COLUMNS: usize = 4_096;

/// A versioned semantic schema for an immutable artifact.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactSchema {
    id: String,
    version: u32,
}

/// Schema- and semantics-bound identity of an immutable artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactId(ContentDigest);

/// Physical container declared for an immutable native table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TableFormat {
    /// Random-access Arrow IPC file, not an Arrow IPC stream.
    ArrowIpcFile,
    /// Apache Parquet file.
    ParquetFile,
}

/// Closed scalar types supported by the first immutable-table manifest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TableScalarType {
    /// Boolean value.
    Boolean,
    /// Signed 64-bit integer.
    I64,
    /// Unsigned 64-bit integer.
    U64,
    /// IEEE-754 binary32 value.
    F32,
    /// IEEE-754 binary64 value.
    F64,
    /// UTF-8 text.
    Utf8,
    /// Arbitrary bytes.
    Binary,
}

/// Type declaration for one native table column.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TableColumnType {
    /// One scalar value per row.
    Scalar(TableScalarType),
    /// A fixed-size list of scalar values per row.
    FixedSizeList {
        /// Scalar element type.
        element: TableScalarType,
        /// Positive number of elements.
        length: u32,
    },
}

/// Ordered column declaration for an immutable native table.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TableColumn {
    name: String,
    column_type: TableColumnType,
    nullable: bool,
}

/// Exact semantic declaration of an immutable Arrow IPC or Parquet table.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TableManifest {
    format: TableFormat,
    encoding_version: String,
    row_count: u64,
    columns: Vec<TableColumn>,
    primary_key: Vec<String>,
}

/// Invalid immutable table declaration or exact-schema comparison.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum TableManifestError {
    /// Wire format name is not supported.
    #[error("unsupported table format {value:?}")]
    InvalidFormat {
        /// Rejected format name.
        value: String,
    },
    /// Encoding version is empty, too long, or contains non-visible ASCII.
    #[error("table encoding version must be 1-128 visible ASCII bytes")]
    InvalidEncodingVersion {
        /// Rejected value.
        value: String,
    },
    /// Table has no columns or exceeds the fixed bound.
    #[error("table has {observed} columns; expected 1..={maximum}")]
    InvalidColumnCount {
        /// Observed number of columns.
        observed: usize,
        /// Maximum permitted number of columns.
        maximum: usize,
    },
    /// Column name violates the shared identifier grammar.
    #[error("invalid table column name {value:?}")]
    InvalidColumnName {
        /// Rejected name.
        value: String,
    },
    /// Fixed-size list length is zero.
    #[error("fixed-size list column {column:?} must have positive length")]
    InvalidFixedListLength {
        /// Affected column.
        column: String,
    },
    /// Wire scalar type name is not supported.
    #[error("unsupported table scalar type {value:?}")]
    InvalidColumnType {
        /// Rejected type name.
        value: String,
    },
    /// Two columns share a name.
    #[error("duplicate table column {column:?}")]
    DuplicateColumn {
        /// Duplicated column.
        column: String,
    },
    /// Primary key is empty.
    #[error("table primary key must not be empty")]
    EmptyPrimaryKey,
    /// Primary-key column is repeated.
    #[error("duplicate primary-key column {column:?}")]
    DuplicatePrimaryKey {
        /// Duplicated key column.
        column: String,
    },
    /// Primary-key column is absent from the table.
    #[error("primary-key column {column:?} is missing")]
    MissingPrimaryKeyColumn {
        /// Missing key column.
        column: String,
    },
    /// Primary-key column permits null values.
    #[error("primary-key column {column:?} must be non-nullable")]
    NullablePrimaryKey {
        /// Nullable key column.
        column: String,
    },
    /// Primary-key type has unstable durable equality semantics.
    #[error("primary-key column {column:?} uses an unstable key type")]
    UnstablePrimaryKeyType {
        /// Affected key column.
        column: String,
    },
    /// Observed table declaration differs from the expected declaration.
    #[error("table manifest mismatch in {field}")]
    Mismatch {
        /// First field that differs in deterministic comparison order.
        field: &'static str,
    },
}

/// Logical runtime binding name for an artifact store.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StoreId(String);

/// Normalized store-relative UTF-8 artifact key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactKey(String);

/// Portable location of one immutable artifact replica.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactLocator {
    store_id: StoreId,
    key: ArtifactKey,
    object_version: Option<String>,
}

/// Location-free declaration for a new immutable artifact owned by a publishing store.
///
/// A draft has stable content and semantic identity but is not an `ArtifactRecord`, has no
/// replica declaration, and cannot enter a catalog. Successful store publication converts it
/// into a normal record containing only the truthful managed locator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactDraft {
    id: ArtifactId,
    semantic_digest: ContentDigest,
    content: ArtifactRef,
    schema: ArtifactSchema,
    table: Option<TableManifest>,
    dependencies: Vec<ArtifactId>,
    semantic_metadata: BTreeMap<String, String>,
}

/// Immutable schema, semantic identity, dependencies, and replica declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRecord {
    id: ArtifactId,
    semantic_digest: ContentDigest,
    content: ArtifactRef,
    schema: ArtifactSchema,
    table: Option<TableManifest>,
    dependencies: Vec<ArtifactId>,
    semantic_metadata: BTreeMap<String, String>,
    locations: Vec<ArtifactLocator>,
}

/// Invalid artifact schema, locator, dependency, or semantic metadata.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ArtifactRecordError {
    /// Schema identifier violates the shared token grammar.
    #[error("invalid artifact schema ID {value:?}")]
    InvalidSchemaId {
        /// Rejected value.
        value: String,
    },
    /// Schema version is zero.
    #[error("artifact schema version must be positive")]
    InvalidSchemaVersion,
    /// Store identifier violates the shared token grammar.
    #[error("invalid artifact store ID {value:?}")]
    InvalidStoreId {
        /// Rejected value.
        value: String,
    },
    /// Artifact key is not a normalized bounded relative key.
    #[error("invalid artifact key {value:?}")]
    InvalidArtifactKey {
        /// Rejected key.
        value: String,
    },
    /// External locator attempts to enter a managed or recovery namespace.
    #[error("artifact key {key:?} uses a reserved store namespace")]
    ReservedArtifactKey {
        /// Rejected key.
        key: String,
    },
    /// Object version is not a bounded non-secret token.
    #[error("invalid artifact object version {value:?}")]
    InvalidObjectVersion {
        /// Rejected value.
        value: String,
    },
    /// Record has no location or exceeds the fixed replica limit.
    #[error("artifact has {observed} locations; expected 1..=16")]
    InvalidLocationCount {
        /// Observed location count.
        observed: usize,
    },
    /// One record declares two locators for the same store.
    #[error("artifact declares multiple locators for store {store_id:?}")]
    DuplicateStoreLocator {
        /// Conflicting store.
        store_id: String,
    },
    /// Replica merge changes the key/version already assigned to a store.
    #[error("artifact replica conflicts with existing store {store_id:?}")]
    ConflictingStoreLocator {
        /// Conflicting store.
        store_id: String,
    },
    /// Direct dependency is repeated.
    #[error("artifact declares a dependency more than once")]
    DuplicateDependency,
    /// Semantic metadata exceeds the fixed pair bound.
    #[error("artifact has {observed} metadata entries; maximum is {maximum}")]
    TooManyMetadataEntries {
        /// Observed count.
        observed: usize,
        /// Maximum permitted number of entries.
        maximum: usize,
    },
    /// Semantic metadata key violates the shared token grammar.
    #[error("invalid semantic metadata key {key:?}")]
    InvalidMetadataKey {
        /// Rejected key.
        key: String,
    },
    /// Semantic metadata value is oversized, control-bearing, or path/URI/query-like.
    #[error("invalid semantic metadata value for key {key:?}")]
    InvalidMetadataValue {
        /// Affected key.
        key: String,
    },
}
