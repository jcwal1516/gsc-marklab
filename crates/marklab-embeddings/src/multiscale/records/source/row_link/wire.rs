use std::{fmt, mem::size_of};

use marklab_data::PatchId;
use marklab_project::{ArtifactId, ContentDigest};
use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize, Serializer,
};

use super::{retained_bytes, PatchEmbeddingSourceRowLinkEntry, FORMAT, VERSION};
use crate::{
    multiscale::{
        error::MultiscaleEmbeddingError,
        records::codec::{
            checked_slots, expect_header, expect_key, parse_artifact, parse_digest, require_hex,
            serialize_artifact, serialize_digest, try_vec_capacity, MAX_SOURCE_ROWS,
        },
    },
    EmbeddingStatus,
};

pub(super) struct Preflight {
    pub(super) entry_count: usize,
    pub(super) patch_bytes: usize,
}

impl Preflight {
    pub(super) fn decoded_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        size_of::<Parsed>()
            .checked_add(checked_slots::<PatchEmbeddingSourceRowLinkEntry>(
                self.entry_count,
            )?)
            .and_then(|value| value.checked_add(self.patch_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    }

    pub(super) fn retained_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        retained_bytes(self.entry_count, self.patch_bytes)
    }
}

pub(super) struct Parsed {
    pub(super) source_entities_artifact_id: ArtifactId,
    pub(super) source_entities_logical_digest: ContentDigest,
    pub(super) source_vectors_artifact_id: ArtifactId,
    pub(super) expected_patches_artifact_id: ArtifactId,
    pub(super) expected_patches_logical_digest: ContentDigest,
    pub(super) identity_map_artifact_id: ArtifactId,
    pub(super) identity_map_logical_digest: ContentDigest,
    pub(super) converter_artifact_id: ArtifactId,
    pub(super) entries: Vec<PatchEmbeddingSourceRowLinkEntry>,
}

pub(super) fn parse_preflight(bytes: &[u8]) -> Result<Preflight, MultiscaleEmbeddingError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let preflight = PreflightSeed
        .deserialize(&mut deserializer)
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    deserializer
        .end()
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    Ok(preflight)
}

struct PreflightSeed;

impl<'de> DeserializeSeed<'de> for PreflightSeed {
    type Value = Preflight;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(PreflightVisitor)
    }
}

struct PreflightVisitor;

impl<'de> Visitor<'de> for PreflightVisitor {
    type Value = Preflight;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an exact patch source-row link")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_header(&mut map, FORMAT, VERSION)?;
        for key in [
            "source_entities_artifact_id",
            "source_entities_logical_digest",
            "source_vectors_artifact_id",
            "expected_patches_artifact_id",
            "expected_patches_logical_digest",
            "identity_map_artifact_id",
            "identity_map_logical_digest",
            "converter_artifact_id",
        ] {
            expect_key(&mut map, key)?;
            require_hex::<A::Error>(map.next_value::<&str>()?)?;
        }
        expect_key(&mut map, "entries")?;
        let (entry_count, patch_bytes) = map.next_value_seed(EntryPreflightSeed)?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Preflight {
            entry_count,
            patch_bytes,
        })
    }
}

struct EntryPreflightSeed;

impl<'de> DeserializeSeed<'de> for EntryPreflightSeed {
    type Value = (usize, usize);

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(EntryPreflightVisitor)
    }
}

struct EntryPreflightVisitor;

impl<'de> Visitor<'de> for EntryPreflightVisitor {
    type Value = (usize, usize);

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded patch source-row entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut count = 0_usize;
        let mut patch_bytes = 0_usize;
        while let Some(entry) = sequence.next_element::<OwnedEntry>()? {
            if entry.patch_id.len() > 255 || parse_status(&entry.embedding_status).is_none() {
                return Err(A::Error::custom("invalid entry"));
            }
            count = count
                .checked_add(1)
                .ok_or_else(|| A::Error::custom("count overflow"))?;
            if count > MAX_SOURCE_ROWS {
                return Err(A::Error::custom("too many entries"));
            }
            patch_bytes = patch_bytes
                .checked_add(entry.patch_id.len())
                .ok_or_else(|| A::Error::custom("patch text overflow"))?;
        }
        Ok((count, patch_bytes))
    }
}

pub(super) fn parse_collect(
    bytes: &[u8],
    entry_count: usize,
) -> Result<Parsed, MultiscaleEmbeddingError> {
    let entries = try_vec_capacity::<PatchEmbeddingSourceRowLinkEntry>(entry_count)?;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let parsed = CollectSeed {
        entry_count,
        entries,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    deserializer
        .end()
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    Ok(parsed)
}

struct CollectSeed {
    pub(super) entry_count: usize,
    pub(super) entries: Vec<PatchEmbeddingSourceRowLinkEntry>,
}

impl<'de> DeserializeSeed<'de> for CollectSeed {
    type Value = Parsed;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(CollectVisitor {
            entry_count: self.entry_count,
            entries: self.entries,
        })
    }
}

struct CollectVisitor {
    pub(super) entry_count: usize,
    pub(super) entries: Vec<PatchEmbeddingSourceRowLinkEntry>,
}

impl<'de> Visitor<'de> for CollectVisitor {
    type Value = Parsed;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a preflighted patch source-row link")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let Self {
            entry_count,
            entries,
        } = self;
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "source_entities_artifact_id")?;
        let source_entities_artifact_id = parse_artifact::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "source_entities_logical_digest")?;
        let source_entities_logical_digest = parse_digest::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "source_vectors_artifact_id")?;
        let source_vectors_artifact_id = parse_artifact::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "expected_patches_artifact_id")?;
        let expected_patches_artifact_id = parse_artifact::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "expected_patches_logical_digest")?;
        let expected_patches_logical_digest = parse_digest::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "identity_map_artifact_id")?;
        let identity_map_artifact_id = parse_artifact::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "identity_map_logical_digest")?;
        let identity_map_logical_digest = parse_digest::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "converter_artifact_id")?;
        let converter_artifact_id = parse_artifact::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "entries")?;
        let entries = map.next_value_seed(EntryCollectSeed {
            expected_count: entry_count,
            entries,
        })?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Parsed {
            source_entities_artifact_id,
            source_entities_logical_digest,
            source_vectors_artifact_id,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            identity_map_artifact_id,
            identity_map_logical_digest,
            converter_artifact_id,
            entries,
        })
    }
}

struct EntryCollectSeed {
    pub(super) expected_count: usize,
    pub(super) entries: Vec<PatchEmbeddingSourceRowLinkEntry>,
}

impl<'de> DeserializeSeed<'de> for EntryCollectSeed {
    type Value = Vec<PatchEmbeddingSourceRowLinkEntry>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(EntryCollectVisitor {
            expected_count: self.expected_count,
            entries: self.entries,
        })
    }
}

struct EntryCollectVisitor {
    pub(super) expected_count: usize,
    pub(super) entries: Vec<PatchEmbeddingSourceRowLinkEntry>,
}

impl<'de> Visitor<'de> for EntryCollectVisitor {
    type Value = Vec<PatchEmbeddingSourceRowLinkEntry>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("preflighted patch source-row entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while let Some(entry) = sequence.next_element::<OwnedEntry>()? {
            let patch_id =
                PatchId::new(entry.patch_id).map_err(|_| A::Error::custom("invalid patch ID"))?;
            let status = parse_status(&entry.embedding_status)
                .ok_or_else(|| A::Error::custom("invalid status"))?;
            self.entries.push(
                PatchEmbeddingSourceRowLinkEntry::new(
                    patch_id,
                    status,
                    entry.source_entity_row,
                    entry.source_vector_row,
                )
                .map_err(|_| A::Error::custom("invalid row-link entry"))?,
            );
        }
        if self.entries.len() != self.expected_count {
            return Err(A::Error::custom("preflight count drift"));
        }
        Ok(self.entries)
    }
}

fn parse_status(value: &str) -> Option<EmbeddingStatus> {
    match value {
        "present" => Some(EmbeddingStatus::Present),
        "missing_vector" => Some(EmbeddingStatus::MissingVector),
        "extraction_failed" => Some(EmbeddingStatus::ExtractionFailed),
        "qc_rejected" => Some(EmbeddingStatus::QcRejected),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedEntry {
    pub(super) patch_id: String,
    pub(super) embedding_status: String,
    pub(super) source_entity_row: u64,
    pub(super) source_vector_row: Option<u64>,
}

#[derive(Serialize)]
pub(super) struct WireRef<'a> {
    pub(super) format: &'static str,
    pub(super) version: u32,
    #[serde(serialize_with = "serialize_artifact")]
    pub(super) source_entities_artifact_id: ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    pub(super) source_entities_logical_digest: ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    pub(super) source_vectors_artifact_id: ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    pub(super) expected_patches_artifact_id: ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    pub(super) expected_patches_logical_digest: ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    pub(super) identity_map_artifact_id: ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    pub(super) identity_map_logical_digest: ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    pub(super) converter_artifact_id: ArtifactId,
    pub(super) entries: EntrySlice<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct EntrySlice<'a>(pub(super) &'a [PatchEmbeddingSourceRowLinkEntry]);

impl Serialize for EntrySlice<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for entry in self.0 {
            sequence.serialize_element(&EntryRef {
                patch_id: entry.patch_id.as_str(),
                embedding_status: entry.status.wire_name(),
                source_entity_row: entry.source_entity_row,
                source_vector_row: entry.source_vector_row,
            })?;
        }
        sequence.end()
    }
}

#[derive(Serialize)]
struct EntryRef<'a> {
    pub(super) patch_id: &'a str,
    pub(super) embedding_status: &'static str,
    pub(super) source_entity_row: u64,
    pub(super) source_vector_row: Option<u64>,
}
