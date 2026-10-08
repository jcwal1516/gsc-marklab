use std::{collections::BTreeMap, fmt, str::FromStr};

use super::{
    ArtifactId, ArtifactRecordError, ArtifactSchema, TableColumnType, TableManifest,
    MAX_IDENTIFIER_BYTES, MAX_METADATA_ENTRIES, MAX_METADATA_VALUE_BYTES,
};
use crate::{ArtifactRef, ContentDigest, ContentDigestParseError};

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

pub(super) fn prepare_artifact_identity(
    content: &ArtifactRef,
    schema: &ArtifactSchema,
    table: Option<&TableManifest>,
    mut dependencies: Vec<ArtifactId>,
    semantic_metadata: &BTreeMap<String, String>,
) -> Result<(Vec<ArtifactId>, ContentDigest, ArtifactId), ArtifactRecordError> {
    dependencies.sort_unstable();
    if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ArtifactRecordError::DuplicateDependency);
    }
    validate_metadata(semantic_metadata)?;
    let semantic_digest =
        compute_semantic_digest(content, schema, table, &dependencies, semantic_metadata);
    let id = compute_artifact_id(content, schema, semantic_digest);
    Ok((dependencies, semantic_digest, id))
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

pub(super) fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}
