use std::{fmt, mem::size_of};

use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize, Serializer,
};

use super::{retained_bytes, PatchSourceEntityEntry, FORMAT, VERSION};
use crate::multiscale::{
    error::MultiscaleEmbeddingError,
    records::codec::{
        checked_slots, expect_header, expect_key, try_vec_capacity, valid_source_key,
        MAX_SOURCE_ROWS,
    },
};

pub(super) struct Preflight {
    pub(super) profile_bytes: usize,
    pub(super) entry_count: usize,
    pub(super) source_bytes: usize,
}

impl Preflight {
    pub(super) fn decoded_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        size_of::<Parsed>()
            .checked_add(self.profile_bytes)
            .and_then(|value| {
                value.checked_add(checked_slots::<PatchSourceEntityEntry>(self.entry_count).ok()?)
            })
            .and_then(|value| value.checked_add(self.source_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    }

    pub(super) fn retained_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        retained_bytes(self.profile_bytes, self.entry_count, self.source_bytes)
    }
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
        formatter.write_str("an exact patch source-entity set")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "source_profile")?;
        let profile: &str = map.next_value()?;
        expect_key(&mut map, "entries")?;
        let (entry_count, source_bytes) = map.next_value_seed(EntryPreflightSeed)?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Preflight {
            profile_bytes: profile.len(),
            entry_count,
            source_bytes,
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
        formatter.write_str("bounded patch source entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut count = 0_usize;
        let mut source_bytes = 0_usize;
        while let Some(entry) = sequence.next_element::<OwnedEntry>()? {
            if !valid_source_key(&entry.source_patch_id) {
                return Err(A::Error::custom("invalid source key"));
            }
            count = count
                .checked_add(1)
                .ok_or_else(|| A::Error::custom("count overflow"))?;
            if count > MAX_SOURCE_ROWS {
                return Err(A::Error::custom("too many entries"));
            }
            source_bytes = source_bytes
                .checked_add(entry.source_patch_id.len())
                .ok_or_else(|| A::Error::custom("text overflow"))?;
        }
        Ok((count, source_bytes))
    }
}

pub(super) struct Parsed {
    pub(super) source_profile: String,
    pub(super) entries: Vec<PatchSourceEntityEntry>,
}

pub(super) fn parse_collect(
    bytes: &[u8],
    entry_count: usize,
) -> Result<Parsed, MultiscaleEmbeddingError> {
    let entries = try_vec_capacity::<PatchSourceEntityEntry>(entry_count)?;
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
    pub(super) entries: Vec<PatchSourceEntityEntry>,
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
    pub(super) entries: Vec<PatchSourceEntityEntry>,
}

impl<'de> Visitor<'de> for CollectVisitor {
    type Value = Parsed;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a preflighted patch source-entity set")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let Self {
            entry_count,
            entries,
        } = self;
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "source_profile")?;
        let source_profile = map.next_value()?;
        expect_key(&mut map, "entries")?;
        let entries = map.next_value_seed(EntryCollectSeed {
            expected_count: entry_count,
            entries,
        })?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Parsed {
            source_profile,
            entries,
        })
    }
}

struct EntryCollectSeed {
    pub(super) expected_count: usize,
    pub(super) entries: Vec<PatchSourceEntityEntry>,
}

impl<'de> DeserializeSeed<'de> for EntryCollectSeed {
    type Value = Vec<PatchSourceEntityEntry>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(EntryCollectVisitor {
            expected_count: self.expected_count,
            entries: self.entries,
        })
    }
}

struct EntryCollectVisitor {
    pub(super) expected_count: usize,
    pub(super) entries: Vec<PatchSourceEntityEntry>,
}

impl<'de> Visitor<'de> for EntryCollectVisitor {
    type Value = Vec<PatchSourceEntityEntry>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("preflighted patch source entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while let Some(entry) = sequence.next_element::<OwnedEntry>()? {
            self.entries.push(
                PatchSourceEntityEntry::new(entry.source_patch_id, entry.source_entity_row)
                    .map_err(|_| A::Error::custom("invalid entry"))?,
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
    pub(super) source_entity_row: u64,
}

#[derive(Serialize)]
pub(super) struct WireRef<'a> {
    pub(super) format: &'static str,
    pub(super) version: u32,
    pub(super) source_profile: &'a str,
    pub(super) entries: EntrySlice<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct EntrySlice<'a>(pub(super) &'a [PatchSourceEntityEntry]);

impl Serialize for EntrySlice<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for entry in self.0 {
            sequence.serialize_element(&EntryRef {
                source_patch_id: &entry.source_patch_id,
                source_entity_row: entry.source_entity_row,
            })?;
        }
        sequence.end()
    }
}

#[derive(Serialize)]
struct EntryRef<'a> {
    pub(super) source_patch_id: &'a str,
    pub(super) source_entity_row: u64,
}
