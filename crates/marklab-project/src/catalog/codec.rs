use super::{validation::validate_decoded_graph, *};

impl ArtifactCatalog {
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

pub(super) struct DecodedRecord {
    pub(super) claimed_id: ArtifactId,
    pub(super) claimed_semantic_digest: ContentDigest,
    pub(super) record: ArtifactRecord,
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
