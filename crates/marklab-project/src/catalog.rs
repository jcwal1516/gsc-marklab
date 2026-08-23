use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::{
    ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRecordError, ArtifactSchema,
    StoreId, TableColumn, TableColumnType, TableFormat, TableManifest, TableManifestError,
    TableScalarType,
};
use crate::{ArtifactRef, ContentDigest, ProjectError};

const CATALOG_FORMAT: &str = "marklab.artifact_catalog";
const CATALOG_VERSION: u32 = 1;
const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;
const MAX_CATALOG_ARTIFACTS: usize = 100_000;

/// Validated in-memory artifact catalog with deterministic immutable snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArtifactCatalog {
    artifacts: BTreeMap<ArtifactId, ArtifactRecord>,
}

impl ArtifactCatalog {
    /// Create an empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a catalog while allowing dependency records in any input order.
    pub fn from_records(
        records: impl IntoIterator<Item = ArtifactRecord>,
    ) -> Result<Self, ArtifactCatalogError> {
        let mut artifacts = BTreeMap::new();
        for record in records {
            let id = record.id();
            if artifacts.insert(id, record).is_some() {
                return Err(ArtifactCatalogError::DuplicateArtifact { artifact: id });
            }
            if artifacts.len() > MAX_CATALOG_ARTIFACTS {
                return Err(ArtifactCatalogError::TooManyArtifacts {
                    observed: artifacts.len(),
                    maximum: MAX_CATALOG_ARTIFACTS,
                });
            }
        }
        validate_graph(&artifacts)?;
        Ok(Self { artifacts })
    }

    /// Register one dependency-complete record or deterministically add replicas.
    ///
    /// The operation is failure-atomic. A new record must name only artifacts
    /// already present; use [`Self::from_records`] for a forward-referenced batch.
    pub fn register(&mut self, record: ArtifactRecord) -> Result<(), ArtifactCatalogError> {
        let id = record.id();
        if let Some(existing) = self.artifacts.get(&id) {
            if !existing.same_semantics(&record) {
                return Err(ArtifactCatalogError::ConflictingArtifact { artifact: id });
            }
            let mut merged = existing.clone();
            merged.merge_locations(record.locations())?;
            self.artifacts.insert(id, merged);
            return Ok(());
        }
        if self.artifacts.len() == MAX_CATALOG_ARTIFACTS {
            return Err(ArtifactCatalogError::TooManyArtifacts {
                observed: self.artifacts.len() + 1,
                maximum: MAX_CATALOG_ARTIFACTS,
            });
        }
        validate_record_dependencies(&self.artifacts, &record)?;
        self.artifacts.insert(id, record);
        Ok(())
    }

    /// Find a record by its exact schema-bound ID.
    pub fn get(&self, id: ArtifactId) -> Option<&ArtifactRecord> {
        self.artifacts.get(&id)
    }

    /// Whether a schema-bound ID is declared in this catalog.
    pub fn contains(&self, id: ArtifactId) -> bool {
        self.artifacts.contains_key(&id)
    }

    /// Number of unique schema-bound artifacts.
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Whether no schema-bound artifacts are present.
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Iterate in canonical artifact-ID order.
    pub fn iter(&self) -> impl Iterator<Item = (&ArtifactId, &ArtifactRecord)> {
        self.artifacts.iter()
    }

    /// Encode a deterministic strict catalog snapshot with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, ArtifactCatalogError> {
        ensure_catalog_size(self.minimum_encoded_bytes())?;
        let wire = WireCatalog {
            format: CATALOG_FORMAT.to_owned(),
            version: CATALOG_VERSION,
            artifacts: self.artifacts.values().map(WireRecord::from).collect(),
        };
        let mut encoded = BoundedCatalogWriter::default();
        if let Err(source) = serde_json::to_writer(&mut encoded, &wire) {
            return Err(encoded.map_write_error(source));
        }
        if let Err(source) = encoded.write_all(b"\n") {
            return Err(encoded.map_io_error(source));
        }
        Ok(encoded.bytes)
    }

    /// Decode and validate bounded JSON without requiring canonical whitespace/order.
    ///
    /// Successful decoding validates declarations only. It does not read or
    /// establish availability of any referenced artifact bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self, ArtifactCatalogError> {
        ensure_catalog_size(bytes.len())?;
        let wire: WireCatalog =
            serde_json::from_slice(bytes).map_err(|source| ArtifactCatalogError::Json {
                reason: source.to_string(),
            })?;
        if wire.format != CATALOG_FORMAT {
            return Err(ArtifactCatalogError::InvalidCatalogFormat {
                observed: wire.format,
            });
        }
        if wire.version != CATALOG_VERSION {
            return Err(ArtifactCatalogError::UnsupportedCatalogVersion {
                observed: wire.version,
            });
        }
        if wire.artifacts.len() > MAX_CATALOG_ARTIFACTS {
            return Err(ArtifactCatalogError::TooManyArtifacts {
                observed: wire.artifacts.len(),
                maximum: MAX_CATALOG_ARTIFACTS,
            });
        }

        let mut decoded = BTreeMap::<ArtifactId, DecodedRecord>::new();
        for wire_record in wire.artifacts {
            let record = DecodedRecord::try_from(wire_record)?;
            let claimed_id = record.claimed_id;
            if decoded.insert(claimed_id, record).is_some() {
                return Err(ArtifactCatalogError::DuplicateArtifact {
                    artifact: claimed_id,
                });
            }
        }

        validate_decoded_graph(&decoded)?;
        let mut artifacts = BTreeMap::new();
        for (claimed_id, decoded_record) in decoded {
            if decoded_record.record.semantic_digest() != decoded_record.claimed_semantic_digest {
                return Err(ArtifactCatalogError::SemanticDigestMismatch {
                    artifact: claimed_id,
                    expected: decoded_record.claimed_semantic_digest,
                    observed: decoded_record.record.semantic_digest(),
                });
            }
            if decoded_record.record.id() != claimed_id {
                return Err(ArtifactCatalogError::ArtifactIdMismatch {
                    expected: claimed_id,
                    observed: decoded_record.record.id(),
                });
            }
            artifacts.insert(claimed_id, decoded_record.record);
        }
        Ok(Self { artifacts })
    }

    /// Decode a catalog and require byte-for-byte canonical fixed-point form.
    pub fn from_canonical_json(bytes: &[u8]) -> Result<Self, ArtifactCatalogError> {
        let catalog = Self::from_json(bytes)?;
        if catalog.to_canonical_json()? != bytes {
            return Err(ArtifactCatalogError::NonCanonicalCatalog);
        }
        Ok(catalog)
    }

    /// Digest the exact canonical snapshot bytes.
    pub fn digest(&self) -> Result<ContentDigest, ArtifactCatalogError> {
        Ok(ContentDigest::from_bytes(&self.to_canonical_json()?))
    }

    fn minimum_encoded_bytes(&self) -> usize {
        self.artifacts.values().fold(0_usize, |total, record| {
            let table_bytes = record.table().map_or(0, |table| {
                table
                    .columns()
                    .iter()
                    .map(|column| column.name().len())
                    .chain(table.primary_key().iter().map(String::len))
                    .fold(table.encoding_version().len(), usize::saturating_add)
            });
            let dependency_bytes = record.dependencies().len().saturating_mul(64);
            let metadata_bytes = record
                .semantic_metadata()
                .iter()
                .map(|(key, value)| key.len().saturating_add(value.len()))
                .fold(0, usize::saturating_add);
            let location_bytes = record
                .locations()
                .iter()
                .map(|location| {
                    location
                        .store_id()
                        .as_str()
                        .len()
                        .saturating_add(location.key().as_str().len())
                        .saturating_add(location.object_version().map_or(0, str::len))
                })
                .fold(0, usize::saturating_add);
            total
                .saturating_add(64 * 3)
                .saturating_add(record.content().kind().len())
                .saturating_add(record.schema().id().len())
                .saturating_add(table_bytes)
                .saturating_add(dependency_bytes)
                .saturating_add(metadata_bytes)
                .saturating_add(location_bytes)
        })
    }
}

#[derive(Default)]
struct BoundedCatalogWriter {
    bytes: Vec<u8>,
    overflow_at: Option<usize>,
}

impl BoundedCatalogWriter {
    fn map_write_error(&self, source: serde_json::Error) -> ArtifactCatalogError {
        if let Some(observed) = self.overflow_at {
            ArtifactCatalogError::CatalogTooLarge {
                observed,
                maximum: MAX_CATALOG_BYTES,
            }
        } else {
            ArtifactCatalogError::Json {
                reason: source.to_string(),
            }
        }
    }

    fn map_io_error(&self, source: io::Error) -> ArtifactCatalogError {
        if let Some(observed) = self.overflow_at {
            ArtifactCatalogError::CatalogTooLarge {
                observed,
                maximum: MAX_CATALOG_BYTES,
            }
        } else {
            ArtifactCatalogError::Json {
                reason: source.to_string(),
            }
        }
    }
}

impl Write for BoundedCatalogWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let observed = self.bytes.len().saturating_add(buffer.len());
        if observed > MAX_CATALOG_BYTES {
            self.overflow_at = Some(observed);
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "artifact catalog exceeds encoded-size bound",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn ensure_catalog_size(observed: usize) -> Result<(), ArtifactCatalogError> {
    if observed > MAX_CATALOG_BYTES {
        Err(ArtifactCatalogError::CatalogTooLarge {
            observed,
            maximum: MAX_CATALOG_BYTES,
        })
    } else {
        Ok(())
    }
}

fn validate_record_dependencies(
    artifacts: &BTreeMap<ArtifactId, ArtifactRecord>,
    record: &ArtifactRecord,
) -> Result<(), ArtifactCatalogError> {
    for dependency in record.dependencies() {
        if *dependency == record.id() {
            return Err(ArtifactCatalogError::SelfDependency {
                artifact: record.id(),
            });
        }
        if !artifacts.contains_key(dependency) {
            return Err(ArtifactCatalogError::MissingDependency {
                artifact: record.id(),
                dependency: *dependency,
            });
        }
    }
    Ok(())
}

fn validate_graph(
    artifacts: &BTreeMap<ArtifactId, ArtifactRecord>,
) -> Result<(), ArtifactCatalogError> {
    for record in artifacts.values() {
        validate_record_dependencies(artifacts, record)?;
    }
    validate_acyclic(
        artifacts
            .iter()
            .map(|(id, record)| (*id, record.dependencies())),
    )
}

fn validate_decoded_graph(
    records: &BTreeMap<ArtifactId, DecodedRecord>,
) -> Result<(), ArtifactCatalogError> {
    for (id, decoded) in records {
        for dependency in decoded.record.dependencies() {
            if dependency == id {
                return Err(ArtifactCatalogError::SelfDependency { artifact: *id });
            }
            if !records.contains_key(dependency) {
                return Err(ArtifactCatalogError::MissingDependency {
                    artifact: *id,
                    dependency: *dependency,
                });
            }
        }
    }
    validate_acyclic(
        records
            .iter()
            .map(|(id, record)| (*id, record.record.dependencies())),
    )
}

fn validate_acyclic<'a>(
    records: impl IntoIterator<Item = (ArtifactId, &'a [ArtifactId])>,
) -> Result<(), ArtifactCatalogError> {
    let records = records.into_iter().collect::<BTreeMap<_, _>>();
    let mut remaining = records
        .iter()
        .map(|(id, dependencies)| (*id, dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<ArtifactId, Vec<ArtifactId>>::new();
    for (id, dependencies) in &records {
        for dependency in *dependencies {
            dependents.entry(*dependency).or_default().push(*id);
        }
    }
    let mut ready = remaining
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(*id))
        .collect::<BTreeSet<_>>();
    let mut visited = 0;
    while let Some(id) = ready.pop_first() {
        visited += 1;
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let count = remaining
                    .get_mut(child)
                    .expect("validated dependent must exist");
                *count -= 1;
                if *count == 0 {
                    ready.insert(*child);
                }
            }
        }
    }
    if visited == records.len() {
        Ok(())
    } else {
        Err(ArtifactCatalogError::Cycle {
            artifacts: remaining
                .into_iter()
                .filter_map(|(id, count)| (count > 0).then_some(id))
                .collect(),
        })
    }
}

#[derive(Debug)]
struct DecodedRecord {
    claimed_id: ArtifactId,
    claimed_semantic_digest: ContentDigest,
    record: ArtifactRecord,
}

impl TryFrom<WireRecord> for DecodedRecord {
    type Error = ArtifactCatalogError;

    fn try_from(wire: WireRecord) -> Result<Self, Self::Error> {
        let claimed_id = parse_artifact_id(&wire.id)?;
        let claimed_semantic_digest = parse_digest(&wire.semantic_digest)?;
        let content_digest = parse_digest(&wire.content.digest)?;
        let content = ArtifactRef::new(wire.content.kind, content_digest, wire.content.byte_len)
            .map_err(|source| ArtifactCatalogError::InvalidContent(Box::new(source)))?;
        let schema = ArtifactSchema::new(wire.schema.id, wire.schema.version)?;
        let table = wire.table.map(TableManifest::try_from).transpose()?;
        let dependencies = wire
            .dependencies
            .into_iter()
            .map(|value| parse_artifact_id(&value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut metadata = BTreeMap::new();
        for entry in wire.semantic_metadata {
            let key = entry.key;
            if metadata.insert(key.clone(), entry.value).is_some() {
                return Err(ArtifactCatalogError::DuplicateMetadataKey { key });
            }
        }
        let locations = wire
            .locations
            .into_iter()
            .map(|location| {
                let store_id = StoreId::new(location.store_id)?;
                let key = ArtifactKey::new(location.key)?;
                ArtifactLocator::decoded(store_id, key, location.object_version, claimed_id)
            })
            .collect::<Result<Vec<_>, ArtifactRecordError>>()?;
        let record =
            ArtifactRecord::new(content, schema, table, dependencies, metadata, locations)?;
        Ok(Self {
            claimed_id,
            claimed_semantic_digest,
            record,
        })
    }
}

fn parse_digest(value: &str) -> Result<ContentDigest, ArtifactCatalogError> {
    ContentDigest::from_str(value).map_err(|source| ArtifactCatalogError::InvalidDigest {
        value: value.to_owned(),
        source,
    })
}

fn parse_artifact_id(value: &str) -> Result<ArtifactId, ArtifactCatalogError> {
    ArtifactId::from_str(value).map_err(|source| ArtifactCatalogError::InvalidDigest {
        value: value.to_owned(),
        source,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCatalog {
    format: String,
    version: u32,
    artifacts: Vec<WireRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRecord {
    id: String,
    semantic_digest: String,
    content: WireContent,
    schema: WireSchema,
    table: Option<WireTable>,
    dependencies: Vec<String>,
    semantic_metadata: Vec<WireMetadata>,
    locations: Vec<WireLocator>,
}

impl From<&ArtifactRecord> for WireRecord {
    fn from(record: &ArtifactRecord) -> Self {
        Self {
            id: record.id().to_string(),
            semantic_digest: record.semantic_digest().to_string(),
            content: WireContent {
                kind: record.content().kind().to_owned(),
                digest: record.content().digest().to_string(),
                byte_len: record.content().byte_len(),
            },
            schema: WireSchema {
                id: record.schema().id().to_owned(),
                version: record.schema().version(),
            },
            table: record.table().map(WireTable::from),
            dependencies: record
                .dependencies()
                .iter()
                .map(ToString::to_string)
                .collect(),
            semantic_metadata: record
                .semantic_metadata()
                .iter()
                .map(|(key, value)| WireMetadata {
                    key: key.clone(),
                    value: value.clone(),
                })
                .collect(),
            locations: record
                .locations()
                .iter()
                .map(|location| WireLocator {
                    store_id: location.store_id().as_str().to_owned(),
                    key: location.key().as_str().to_owned(),
                    object_version: location.object_version().map(ToOwned::to_owned),
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContent {
    kind: String,
    digest: String,
    byte_len: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSchema {
    id: String,
    version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTable {
    format: String,
    encoding_version: String,
    row_count: u64,
    columns: Vec<WireColumn>,
    primary_key: Vec<String>,
}

impl From<&TableManifest> for WireTable {
    fn from(table: &TableManifest) -> Self {
        Self {
            format: table.format().wire_name().to_owned(),
            encoding_version: table.encoding_version().to_owned(),
            row_count: table.row_count(),
            columns: table.columns().iter().map(WireColumn::from).collect(),
            primary_key: table.primary_key().to_vec(),
        }
    }
}

impl TryFrom<WireTable> for TableManifest {
    type Error = TableManifestError;

    fn try_from(table: WireTable) -> Result<Self, Self::Error> {
        TableManifest::new(
            TableFormat::from_wire(&table.format)?,
            table.encoding_version,
            table.row_count,
            table
                .columns
                .into_iter()
                .map(TableColumn::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            table.primary_key,
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireColumn {
    name: String,
    kind: String,
    scalar: String,
    length: Option<u32>,
    nullable: bool,
}

impl From<&TableColumn> for WireColumn {
    fn from(column: &TableColumn) -> Self {
        let (kind, scalar, length) = match column.column_type() {
            TableColumnType::Scalar(scalar) => {
                ("scalar".to_owned(), scalar.wire_name().to_owned(), None)
            }
            TableColumnType::FixedSizeList { element, length } => (
                "fixed_size_list".to_owned(),
                element.wire_name().to_owned(),
                Some(*length),
            ),
        };
        Self {
            name: column.name().to_owned(),
            kind,
            scalar,
            length,
            nullable: column.nullable(),
        }
    }
}

impl TryFrom<WireColumn> for TableColumn {
    type Error = TableManifestError;

    fn try_from(column: WireColumn) -> Result<Self, Self::Error> {
        let scalar = TableScalarType::from_wire(&column.scalar)?;
        let column_type = match (column.kind.as_str(), column.length) {
            ("scalar", None) => TableColumnType::Scalar(scalar),
            ("fixed_size_list", Some(length)) => TableColumnType::FixedSizeList {
                element: scalar,
                length,
            },
            _ => {
                return Err(TableManifestError::InvalidColumnType { value: column.kind });
            }
        };
        TableColumn::new(column.name, column_type, column.nullable)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMetadata {
    key: String,
    value: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLocator {
    store_id: String,
    key: String,
    object_version: Option<String>,
}

/// Strict catalog decode, dependency, identity, and replica-merge failures.
#[derive(Debug, Error)]
pub enum ArtifactCatalogError {
    /// Input or encoded catalog exceeds the fixed byte bound.
    #[error("artifact catalog has {observed} bytes; maximum is {maximum}")]
    CatalogTooLarge {
        /// Observed size.
        observed: usize,
        /// Maximum size.
        maximum: usize,
    },
    /// Catalog has too many unique artifacts.
    #[error("artifact catalog has {observed} artifacts; maximum is {maximum}")]
    TooManyArtifacts {
        /// Observed count.
        observed: usize,
        /// Maximum count.
        maximum: usize,
    },
    /// JSON is malformed, contains an unknown/duplicate field, or cannot encode.
    #[error("invalid artifact catalog JSON: {reason}")]
    Json {
        /// Parser/encoder context.
        reason: String,
    },
    /// Top-level format identifier is not the C-03 catalog format.
    #[error("unsupported artifact catalog format {observed:?}")]
    InvalidCatalogFormat {
        /// Observed format.
        observed: String,
    },
    /// Catalog version is old, zero, or from the future.
    #[error("unsupported artifact catalog version {observed}")]
    UnsupportedCatalogVersion {
        /// Observed version.
        observed: u32,
    },
    /// Digest text is not canonical lowercase SHA-256 hex.
    #[error("invalid digest {value:?}: {source}")]
    InvalidDigest {
        /// Rejected digest text.
        value: String,
        /// Strict parser error.
        #[source]
        source: crate::ContentDigestParseError,
    },
    /// Underlying byte reference is invalid.
    #[error(transparent)]
    InvalidContent(Box<ProjectError>),
    /// Table declaration is invalid.
    #[error(transparent)]
    InvalidTable(#[from] TableManifestError),
    /// Artifact record/locator/metadata is invalid.
    #[error(transparent)]
    InvalidRecord(#[from] ArtifactRecordError),
    /// Two serialized or batch records claim the same artifact ID.
    #[error("duplicate artifact ID {artifact}")]
    DuplicateArtifact {
        /// Duplicated ID.
        artifact: ArtifactId,
    },
    /// Artifact names itself as a dependency.
    #[error("artifact {artifact} depends on itself")]
    SelfDependency {
        /// Self-dependent artifact.
        artifact: ArtifactId,
    },
    /// Artifact dependency is absent.
    #[error("artifact {artifact} depends on missing artifact {dependency}")]
    MissingDependency {
        /// Dependent artifact.
        artifact: ArtifactId,
        /// Missing dependency.
        dependency: ArtifactId,
    },
    /// Dependency graph contains a cycle.
    #[error("artifact dependency cycle involves {artifacts:?}")]
    Cycle {
        /// Deterministically sorted nodes remaining after iterative sorting.
        artifacts: Vec<ArtifactId>,
    },
    /// Claimed semantic digest differs from the recomputed declaration.
    #[error(
        "artifact {artifact} semantic digest mismatch: expected {expected}, observed {observed}"
    )]
    SemanticDigestMismatch {
        /// Affected artifact.
        artifact: ArtifactId,
        /// Claimed digest.
        expected: ContentDigest,
        /// Recomputed digest.
        observed: ContentDigest,
    },
    /// Claimed artifact ID differs from recomputed schema/content identity.
    #[error("artifact ID mismatch: expected {expected}, observed {observed}")]
    ArtifactIdMismatch {
        /// Claimed ID.
        expected: ArtifactId,
        /// Recomputed ID.
        observed: ArtifactId,
    },
    /// Same artifact ID arrived with distinct non-location semantics.
    #[error("artifact {artifact} conflicts with its existing semantic declaration")]
    ConflictingArtifact {
        /// Conflicting ID.
        artifact: ArtifactId,
    },
    /// Metadata wire array repeats a key.
    #[error("duplicate semantic metadata key {key:?}")]
    DuplicateMetadataKey {
        /// Duplicated key.
        key: String,
    },
    /// Decoded bytes are valid but not the canonical fixed-point encoding.
    #[error("artifact catalog JSON is not canonical")]
    NonCanonicalCatalog,
}
