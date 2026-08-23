use std::{collections::BTreeMap, fmt, str::FromStr};

use thiserror::Error;

use crate::{ArtifactRef, ContentDigest, ContentDigestParseError};

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

impl ArtifactSchema {
    /// Build a schema from a restricted identifier and positive version.
    pub fn new(id: impl Into<String>, version: u32) -> Result<Self, ArtifactRecordError> {
        let id = id.into();
        if !is_identifier(&id) {
            return Err(ArtifactRecordError::InvalidSchemaId { value: id });
        }
        if version == 0 {
            return Err(ArtifactRecordError::InvalidSchemaVersion);
        }
        Ok(Self { id, version })
    }

    /// Stable schema identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Positive schema version.
    pub fn version(&self) -> u32 {
        self.version
    }
}

/// Schema- and semantics-bound identity of an immutable artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactId(ContentDigest);

impl ArtifactId {
    /// Return the underlying SHA-256 digest.
    pub fn digest(self) -> ContentDigest {
        self.0
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ArtifactId {
    type Err = ContentDigestParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ContentDigest::from_str(value).map(Self)
    }
}

/// Physical container declared for an immutable native table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TableFormat {
    /// Random-access Arrow IPC file, not an Arrow IPC stream.
    ArrowIpcFile,
    /// Apache Parquet file.
    ParquetFile,
}

impl TableFormat {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::ArrowIpcFile => "arrow_ipc_file",
            Self::ParquetFile => "parquet_file",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Result<Self, TableManifestError> {
        match value {
            "arrow_ipc_file" => Ok(Self::ArrowIpcFile),
            "parquet_file" => Ok(Self::ParquetFile),
            _ => Err(TableManifestError::InvalidFormat {
                value: value.to_owned(),
            }),
        }
    }
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

impl TableScalarType {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Utf8 => "utf8",
            Self::Binary => "binary",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Result<Self, TableManifestError> {
        match value {
            "boolean" => Ok(Self::Boolean),
            "i64" => Ok(Self::I64),
            "u64" => Ok(Self::U64),
            "f32" => Ok(Self::F32),
            "f64" => Ok(Self::F64),
            "utf8" => Ok(Self::Utf8),
            "binary" => Ok(Self::Binary),
            _ => Err(TableManifestError::InvalidColumnType {
                value: value.to_owned(),
            }),
        }
    }

    fn stable_primary_key(self) -> bool {
        !matches!(self, Self::F32 | Self::F64)
    }
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

impl TableColumn {
    /// Build a column after validating its stable name and type bounds.
    pub fn new(
        name: impl Into<String>,
        column_type: TableColumnType,
        nullable: bool,
    ) -> Result<Self, TableManifestError> {
        let name = name.into();
        if !is_identifier(&name) {
            return Err(TableManifestError::InvalidColumnName { value: name });
        }
        if matches!(
            column_type,
            TableColumnType::FixedSizeList { length: 0, .. }
        ) {
            return Err(TableManifestError::InvalidFixedListLength { column: name });
        }
        Ok(Self {
            name,
            column_type,
            nullable,
        })
    }

    /// Column name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declared column type.
    pub fn column_type(&self) -> &TableColumnType {
        &self.column_type
    }

    /// Whether null values are permitted.
    pub fn nullable(&self) -> bool {
        self.nullable
    }
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

impl TableManifest {
    /// Build and validate an exact ordered table declaration.
    pub fn new(
        format: TableFormat,
        encoding_version: impl Into<String>,
        row_count: u64,
        columns: Vec<TableColumn>,
        primary_key: Vec<String>,
    ) -> Result<Self, TableManifestError> {
        let encoding_version = encoding_version.into();
        if encoding_version.is_empty()
            || encoding_version.len() > MAX_IDENTIFIER_BYTES
            || !encoding_version.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(TableManifestError::InvalidEncodingVersion {
                value: encoding_version,
            });
        }
        if columns.is_empty() || columns.len() > MAX_TABLE_COLUMNS {
            return Err(TableManifestError::InvalidColumnCount {
                observed: columns.len(),
                maximum: MAX_TABLE_COLUMNS,
            });
        }
        let mut by_name = BTreeMap::new();
        for (index, column) in columns.iter().enumerate() {
            if by_name.insert(column.name.clone(), index).is_some() {
                return Err(TableManifestError::DuplicateColumn {
                    column: column.name.clone(),
                });
            }
        }
        if primary_key.is_empty() {
            return Err(TableManifestError::EmptyPrimaryKey);
        }
        let mut seen_keys = BTreeMap::new();
        for key in &primary_key {
            if seen_keys.insert(key.clone(), ()).is_some() {
                return Err(TableManifestError::DuplicatePrimaryKey {
                    column: key.clone(),
                });
            }
            let Some(index) = by_name.get(key) else {
                return Err(TableManifestError::MissingPrimaryKeyColumn {
                    column: key.clone(),
                });
            };
            let column = &columns[*index];
            if column.nullable {
                return Err(TableManifestError::NullablePrimaryKey {
                    column: key.clone(),
                });
            }
            let stable = match column.column_type {
                TableColumnType::Scalar(scalar) => scalar.stable_primary_key(),
                TableColumnType::FixedSizeList { .. } => false,
            };
            if !stable {
                return Err(TableManifestError::UnstablePrimaryKeyType {
                    column: key.clone(),
                });
            }
        }
        Ok(Self {
            format,
            encoding_version,
            row_count,
            columns,
            primary_key,
        })
    }

    /// Declared file format.
    pub fn format(&self) -> TableFormat {
        self.format
    }

    /// Explicit writer/encoding contract version.
    pub fn encoding_version(&self) -> &str {
        &self.encoding_version
    }

    /// Exact row count.
    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    /// Ordered columns.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Ordered non-null stable primary-key columns.
    pub fn primary_key(&self) -> &[String] {
        &self.primary_key
    }

    /// Require an observed declaration to match every semantic field exactly.
    pub fn require_exact(&self, observed: &Self) -> Result<(), TableManifestError> {
        let field = if self.format != observed.format {
            "format"
        } else if self.encoding_version != observed.encoding_version {
            "encoding_version"
        } else if self.row_count != observed.row_count {
            "row_count"
        } else if self.columns != observed.columns {
            "columns"
        } else if self.primary_key != observed.primary_key {
            "primary_key"
        } else {
            return Ok(());
        };
        Err(TableManifestError::Mismatch { field })
    }
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

impl StoreId {
    /// Validate a restricted portable store identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ArtifactRecordError> {
        let value = value.into();
        if !is_identifier(&value) {
            return Err(ArtifactRecordError::InvalidStoreId { value });
        }
        Ok(Self(value))
    }

    /// Borrow the validated binding name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Normalized store-relative UTF-8 artifact key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactKey(String);

impl ArtifactKey {
    /// Validate a portable relative key without assigning namespace policy.
    pub fn new(value: impl Into<String>) -> Result<Self, ArtifactRecordError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_ARTIFACT_KEY_BYTES
            || value.starts_with('/')
            || value.starts_with("//")
            || value.contains('\\')
            || value.contains(':')
            || value.chars().any(char::is_control)
        {
            return Err(ArtifactRecordError::InvalidArtifactKey { value });
        }
        for component in value.split('/') {
            if component.is_empty()
                || component == "."
                || component == ".."
                || component.len() > MAX_KEY_COMPONENT_BYTES
            {
                return Err(ArtifactRecordError::InvalidArtifactKey { value });
            }
        }
        Ok(Self(value))
    }

    /// Borrow the normalized relative key.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn managed(id: ArtifactId) -> Self {
        let digest = id.to_string();
        Self(format!("objects/sha256/{}/{digest}", &digest[..2]))
    }

    pub(crate) fn is_reserved(&self) -> bool {
        matches!(
            self.0.split('/').next(),
            Some("objects" | ".marklab-staging" | ".marklab-quarantine" | ".marklab-store.lock")
        )
    }
}

/// Portable location of one immutable artifact replica.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactLocator {
    store_id: StoreId,
    key: ArtifactKey,
    object_version: Option<String>,
}

impl ArtifactLocator {
    /// Build an external reference-only locator outside managed namespaces.
    pub fn new(
        store_id: StoreId,
        key: ArtifactKey,
        object_version: Option<String>,
    ) -> Result<Self, ArtifactRecordError> {
        if key.is_reserved() {
            return Err(ArtifactRecordError::ReservedArtifactKey { key: key.0.clone() });
        }
        Self::build(store_id, key, object_version)
    }

    pub(crate) fn managed(store_id: StoreId, id: ArtifactId) -> Self {
        Self {
            store_id,
            key: ArtifactKey::managed(id),
            object_version: None,
        }
    }

    pub(crate) fn decoded(
        store_id: StoreId,
        key: ArtifactKey,
        object_version: Option<String>,
        record_id: ArtifactId,
    ) -> Result<Self, ArtifactRecordError> {
        if key.is_reserved() && key != ArtifactKey::managed(record_id) {
            return Err(ArtifactRecordError::ReservedArtifactKey { key: key.0.clone() });
        }
        Self::build(store_id, key, object_version)
    }

    fn build(
        store_id: StoreId,
        key: ArtifactKey,
        object_version: Option<String>,
    ) -> Result<Self, ArtifactRecordError> {
        if let Some(version) = &object_version {
            if version.is_empty()
                || version.len() > MAX_OBJECT_VERSION_BYTES
                || !version.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':')
                })
            {
                return Err(ArtifactRecordError::InvalidObjectVersion {
                    value: version.clone(),
                });
            }
        }
        Ok(Self {
            store_id,
            key,
            object_version,
        })
    }

    /// Logical store binding.
    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    /// Store-relative key.
    pub fn key(&self) -> &ArtifactKey {
        &self.key
    }

    /// Optional non-secret object-generation provenance.
    pub fn object_version(&self) -> Option<&str> {
        self.object_version.as_deref()
    }
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

impl ArtifactRecord {
    /// Build a validated record and compute its location-independent identity.
    pub fn new(
        content: ArtifactRef,
        schema: ArtifactSchema,
        table: Option<TableManifest>,
        mut dependencies: Vec<ArtifactId>,
        semantic_metadata: BTreeMap<String, String>,
        mut locations: Vec<ArtifactLocator>,
    ) -> Result<Self, ArtifactRecordError> {
        dependencies.sort_unstable();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ArtifactRecordError::DuplicateDependency);
        }
        validate_metadata(&semantic_metadata)?;
        if locations.is_empty() || locations.len() > 16 {
            return Err(ArtifactRecordError::InvalidLocationCount {
                observed: locations.len(),
            });
        }
        locations.sort();
        for pair in locations.windows(2) {
            if pair[0].store_id == pair[1].store_id {
                return Err(ArtifactRecordError::DuplicateStoreLocator {
                    store_id: pair[0].store_id.0.clone(),
                });
            }
        }
        let semantic_digest = compute_semantic_digest(
            &content,
            &schema,
            table.as_ref(),
            &dependencies,
            &semantic_metadata,
        );
        let id = compute_artifact_id(&content, &schema, semantic_digest);
        Ok(Self {
            id,
            semantic_digest,
            content,
            schema,
            table,
            dependencies,
            semantic_metadata,
            locations,
        })
    }

    /// Schema- and content-bound artifact identity.
    pub fn id(&self) -> ArtifactId {
        self.id
    }

    /// Location-independent digest of the semantic declaration.
    pub fn semantic_digest(&self) -> ContentDigest {
        self.semantic_digest
    }

    /// Exact encoded-byte reference.
    pub fn content(&self) -> &ArtifactRef {
        &self.content
    }

    /// Versioned semantic schema.
    pub fn schema(&self) -> &ArtifactSchema {
        &self.schema
    }

    /// Optional exact native-table declaration.
    pub fn table(&self) -> Option<&TableManifest> {
        self.table.as_ref()
    }

    /// Sorted direct artifact dependencies.
    pub fn dependencies(&self) -> &[ArtifactId] {
        &self.dependencies
    }

    /// Sorted bounded semantic metadata.
    pub fn semantic_metadata(&self) -> &BTreeMap<String, String> {
        &self.semantic_metadata
    }

    /// Canonically sorted replica locations, at most one per store.
    pub fn locations(&self) -> &[ArtifactLocator] {
        &self.locations
    }

    pub(crate) fn merge_locations(
        &mut self,
        incoming: &[ArtifactLocator],
    ) -> Result<bool, ArtifactRecordError> {
        let mut merged = self.locations.clone();
        let mut changed = false;
        for locator in incoming {
            match merged.binary_search_by(|existing| existing.store_id.cmp(&locator.store_id)) {
                Ok(index) if merged[index] == *locator => {}
                Ok(_) => {
                    return Err(ArtifactRecordError::ConflictingStoreLocator {
                        store_id: locator.store_id.0.clone(),
                    });
                }
                Err(index) => {
                    if merged.len() == 16 {
                        return Err(ArtifactRecordError::InvalidLocationCount { observed: 17 });
                    }
                    merged.insert(index, locator.clone());
                    changed = true;
                }
            }
        }
        if changed {
            self.locations = merged;
        }
        Ok(changed)
    }

    pub(crate) fn same_semantics(&self, other: &Self) -> bool {
        self.id == other.id
            && self.semantic_digest == other.semantic_digest
            && self.content == other.content
            && self.schema == other.schema
            && self.table == other.table
            && self.dependencies == other.dependencies
            && self.semantic_metadata == other.semantic_metadata
    }
}

fn compute_semantic_digest(
    content: &ArtifactRef,
    schema: &ArtifactSchema,
    table: Option<&TableManifest>,
    dependencies: &[ArtifactId],
    metadata: &BTreeMap<String, String>,
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-artifact-semantics-v1".to_vec(),
        b"content-kind".to_vec(),
        content.kind().as_bytes().to_vec(),
        b"schema-id".to_vec(),
        schema.id.as_bytes().to_vec(),
        b"schema-version".to_vec(),
        schema.version.to_be_bytes().to_vec(),
    ];
    match table {
        Some(table) => {
            fields.push(b"table-present".to_vec());
            fields.push(table.format.wire_name().as_bytes().to_vec());
            fields.push(table.encoding_version.as_bytes().to_vec());
            fields.push(table.row_count.to_be_bytes().to_vec());
            for column in &table.columns {
                fields.push(b"column".to_vec());
                fields.push(column.name.as_bytes().to_vec());
                match column.column_type {
                    TableColumnType::Scalar(scalar) => {
                        fields.push(b"scalar".to_vec());
                        fields.push(scalar.wire_name().as_bytes().to_vec());
                    }
                    TableColumnType::FixedSizeList { element, length } => {
                        fields.push(b"fixed-size-list".to_vec());
                        fields.push(element.wire_name().as_bytes().to_vec());
                        fields.push(length.to_be_bytes().to_vec());
                    }
                }
                fields.push(vec![u8::from(column.nullable)]);
            }
            for key in &table.primary_key {
                fields.push(b"primary-key".to_vec());
                fields.push(key.as_bytes().to_vec());
            }
        }
        None => fields.push(b"table-absent".to_vec()),
    }
    for dependency in dependencies {
        fields.push(b"dependency".to_vec());
        fields.push(dependency.digest().as_bytes().to_vec());
    }
    for (key, value) in metadata {
        fields.push(b"metadata".to_vec());
        fields.push(key.as_bytes().to_vec());
        fields.push(value.as_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn compute_artifact_id(
    content: &ArtifactRef,
    schema: &ArtifactSchema,
    semantic_digest: ContentDigest,
) -> ArtifactId {
    let byte_len = content.byte_len().to_be_bytes();
    let version = schema.version.to_be_bytes();
    ArtifactId(ContentDigest::from_framed([
        b"marklab-artifact-id-v1".as_slice(),
        schema.id.as_bytes(),
        version.as_slice(),
        content.digest().as_bytes().as_slice(),
        byte_len.as_slice(),
        semantic_digest.as_bytes().as_slice(),
    ]))
}

fn validate_metadata(metadata: &BTreeMap<String, String>) -> Result<(), ArtifactRecordError> {
    if metadata.len() > MAX_METADATA_ENTRIES {
        return Err(ArtifactRecordError::TooManyMetadataEntries {
            observed: metadata.len(),
            maximum: MAX_METADATA_ENTRIES,
        });
    }
    for (key, value) in metadata {
        if !is_identifier(key) {
            return Err(ArtifactRecordError::InvalidMetadataKey { key: key.clone() });
        }
        let drive_prefix = value.as_bytes().get(1) == Some(&b':')
            && value
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic);
        if value.len() > MAX_METADATA_VALUE_BYTES
            || value.chars().any(char::is_control)
            || value.starts_with('/')
            || value.starts_with("\\\\")
            || drive_prefix
            || value.contains("://")
            || value.contains('?')
        {
            return Err(ArtifactRecordError::InvalidMetadataValue { key: key.clone() });
        }
    }
    Ok(())
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
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
        /// Maximum count.
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
