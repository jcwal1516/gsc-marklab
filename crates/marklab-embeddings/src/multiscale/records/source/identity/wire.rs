use std::{fmt, mem::size_of};

use marklab_data::PatchId;
use marklab_project::{ArtifactId, ContentDigest};
use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize, Serializer,
};

use super::{retained_bytes, PatchIdentityMapEntry, FORMAT, VERSION};
use crate::multiscale::{
    error::MultiscaleEmbeddingError,
    records::codec::{
        checked_slots, expect_header, expect_key, parse_artifact, parse_digest, require_hex,
        serialize_artifact, serialize_digest, try_vec_capacity, valid_source_key, MAX_SOURCE_ROWS,
    },
};

pub(super) struct Preflight {
    pub(super) entry_count: usize,
    pub(super) source_bytes: usize,
    pub(super) patch_bytes: usize,
}

impl Preflight {
    pub(super) fn decoded_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        size_of::<Parsed>()
            .checked_add(checked_slots::<PatchIdentityMapEntry>(self.entry_count)?)
            .and_then(|value| value.checked_add(self.source_bytes))
            .and_then(|value| value.checked_add(self.patch_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    }

    pub(super) fn retained_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        retained_bytes(
            self.entry_count,
            self.source_bytes,
            self.patch_bytes,
            self.entry_count,
        )
    }
}

pub(super) struct Parsed {
    pub(super) source_entities_artifact_id: ArtifactId,
    pub(super) source_entities_logical_digest: ContentDigest,
    pub(super) expected_patches_artifact_id: ArtifactId,
    pub(super) expected_patches_logical_digest: ContentDigest,
    pub(super) entries: Vec<PatchIdentityMapEntry>,
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
        formatter.write_str("an exact patch identity map")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "source_entities_artifact_id")?;
        require_hex::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "source_entities_logical_digest")?;
        require_hex::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "expected_patches_artifact_id")?;
        require_hex::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "expected_patches_logical_digest")?;
        require_hex::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "entries")?;
        let (entry_count, source_bytes, patch_bytes) = map.next_value_seed(EntryPreflightSeed)?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Preflight {
            entry_count,
            source_bytes,
            patch_bytes,
        })
    }
}

struct EntryPreflightSeed;

impl<'de> DeserializeSeed<'de> for EntryPreflightSeed {
    type Value = (usize, usize, usize);

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(EntryPreflightVisitor)
    }
}

struct EntryPreflightVisitor;

impl<'de> Visitor<'de> for EntryPreflightVisitor {
    type Value = (usize, usize, usize);

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded patch identity entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut count = 0_usize;
        let mut source_bytes = 0_usize;
        let mut patch_bytes = 0_usize;
        while let Some(entry) = sequence.next_element::<OwnedEntry>()? {
            if !valid_source_key(&entry.source_patch_id) || entry.patch_id.len() > 255 {
                return Err(A::Error::custom("invalid entry text"));
            }
            count = count
                .checked_add(1)
                .ok_or_else(|| A::Error::custom("count overflow"))?;
            if count > MAX_SOURCE_ROWS {
                return Err(A::Error::custom("too many entries"));
            }
            source_bytes = source_bytes
                .checked_add(entry.source_patch_id.len())
                .ok_or_else(|| A::Error::custom("source text overflow"))?;
            patch_bytes = patch_bytes
                .checked_add(entry.patch_id.len())
                .ok_or_else(|| A::Error::custom("patch text overflow"))?;
        }
        Ok((count, source_bytes, patch_bytes))
    }
}

pub(super) fn parse_collect(
    bytes: &[u8],
    entry_count: usize,
) -> Result<Parsed, MultiscaleEmbeddingError> {
    let entries = try_vec_capacity::<PatchIdentityMapEntry>(entry_count)?;
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
    pub(super) entries: Vec<PatchIdentityMapEntry>,
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
    pub(super) entries: Vec<PatchIdentityMapEntry>,
}

impl<'de> Visitor<'de> for CollectVisitor {
    type Value = Parsed;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a preflighted patch identity map")
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
        expect_key(&mut map, "expected_patches_artifact_id")?;
        let expected_patches_artifact_id = parse_artifact::<A::Error>(map.next_value::<&str>()?)?;
        expect_key(&mut map, "expected_patches_logical_digest")?;
        let expected_patches_logical_digest = parse_digest::<A::Error>(map.next_value::<&str>()?)?;
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
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            entries,
        })
    }
}

struct EntryCollectSeed {
    pub(super) expected_count: usize,
    pub(super) entries: Vec<PatchIdentityMapEntry>,
}

impl<'de> DeserializeSeed<'de> for EntryCollectSeed {
    type Value = Vec<PatchIdentityMapEntry>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(EntryCollectVisitor {
            expected_count: self.expected_count,
            entries: self.entries,
        })
    }
}

struct EntryCollectVisitor {
    pub(super) expected_count: usize,
    pub(super) entries: Vec<PatchIdentityMapEntry>,
}

impl<'de> Visitor<'de> for EntryCollectVisitor {
    type Value = Vec<PatchIdentityMapEntry>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("preflighted patch identity entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while let Some(entry) = sequence.next_element::<OwnedEntry>()? {
            let patch_id =
                PatchId::new(entry.patch_id).map_err(|_| A::Error::custom("invalid patch ID"))?;
            self.entries.push(
                PatchIdentityMapEntry::new(entry.source_patch_id, patch_id)
                    .map_err(|_| A::Error::custom("invalid map entry"))?,
            );
        }
        if self.entries.len() != self.expected_count {
            return Err(A::Error::custom("preflight count drift"));
        }
        Ok(self.entries)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedEntry {
    pub(super) source_patch_id: String,
    pub(super) patch_id: String,
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
    pub(super) expected_patches_artifact_id: ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    pub(super) expected_patches_logical_digest: ContentDigest,
    pub(super) entries: EntrySlice<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct EntrySlice<'a>(pub(super) &'a [PatchIdentityMapEntry]);

impl Serialize for EntrySlice<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for entry in self.0 {
            sequence.serialize_element(&EntryRef {
                source_patch_id: &entry.source_patch_id,
                patch_id: entry.patch_id.as_str(),
            })?;
        }
        sequence.end()
    }
}

#[derive(Serialize)]
struct EntryRef<'a> {
    pub(super) source_patch_id: &'a str,
    pub(super) patch_id: &'a str,
}
