use std::{fmt, marker::PhantomData, mem::size_of};

use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserializer, Serialize, Serializer,
};

use super::{ExpectedEntitySet, EXPECTED_VERSION, MAX_EXPECTED_ROWS, MAX_RAW_JSON_STRING_BYTES};
use crate::multiscale::{entity::EntitySpec, error::MultiscaleEmbeddingError};

pub(super) fn preflight_json_string_lengths(bytes: &[u8]) -> Result<(), MultiscaleEmbeddingError> {
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        index += 1;
        let start = index;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if !escaped && byte == b'"' {
                break;
            }
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            }
            index += 1;
            if index.saturating_sub(start) > MAX_RAW_JSON_STRING_BYTES {
                return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
            }
        }
        if index == bytes.len() {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        index += 1;
    }
    Ok(())
}

#[derive(Default)]
pub(super) struct ExpectedPreflight {
    owning_slide_bytes: usize,
    selection_rule_bytes: usize,
    pub(super) id_count: usize,
    id_bytes: usize,
}

impl ExpectedPreflight {
    pub(super) fn retained_bytes<I>(&self) -> Result<usize, MultiscaleEmbeddingError> {
        let id_slots = self
            .id_count
            .checked_mul(size_of::<I>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        size_of::<ExpectedEntitySet<I>>()
            .checked_add(self.owning_slide_bytes)
            .and_then(|value| value.checked_add(self.selection_rule_bytes))
            .and_then(|value| value.checked_add(id_slots))
            .and_then(|value| value.checked_add(self.id_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    }

    pub(super) fn decoded_bytes<I>(&self) -> Result<usize, MultiscaleEmbeddingError> {
        let id_slots = self
            .id_count
            .checked_mul(size_of::<I>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        size_of::<ParsedExpected<I>>()
            .checked_add(self.owning_slide_bytes)
            .and_then(|value| value.checked_add(self.selection_rule_bytes))
            .and_then(|value| value.checked_add(id_slots))
            .and_then(|value| value.checked_add(self.id_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    }
}

pub(super) fn parse_preflight<I: EntitySpec>(
    bytes: &[u8],
) -> Result<ExpectedPreflight, MultiscaleEmbeddingError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let preflight = ExpectedPreflightSeed::<I>(PhantomData)
        .deserialize(&mut deserializer)
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    deserializer
        .end()
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    Ok(preflight)
}

struct ExpectedPreflightSeed<I>(PhantomData<I>);

impl<'de, I: EntitySpec> DeserializeSeed<'de> for ExpectedPreflightSeed<I> {
    type Value = ExpectedPreflight;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(ExpectedPreflightVisitor::<I>(PhantomData))
    }
}

struct ExpectedPreflightVisitor<I>(PhantomData<I>);

impl<'de, I: EntitySpec> Visitor<'de> for ExpectedPreflightVisitor<I> {
    type Value = ExpectedPreflight;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an exact Marklab expected-set object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_key(&mut map, "format")?;
        let format: String = map.next_value()?;
        if format != I::EXPECTED_FORMAT {
            return Err(A::Error::custom("wrong format"));
        }
        expect_key(&mut map, "version")?;
        let version: u32 = map.next_value()?;
        if version != EXPECTED_VERSION {
            return Err(A::Error::custom("wrong version"));
        }
        expect_key(&mut map, "owning_slide_id")?;
        let owning_slide_id: String = map.next_value()?;
        expect_key(&mut map, "selection_rule")?;
        let selection_rule: String = map.next_value()?;
        expect_key(&mut map, "ids")?;
        let (id_count, id_bytes) = map.next_value_seed(IdPreflightSeed)?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(ExpectedPreflight {
            owning_slide_bytes: owning_slide_id.len(),
            selection_rule_bytes: selection_rule.len(),
            id_count,
            id_bytes,
        })
    }
}

struct IdPreflightSeed;

impl<'de> DeserializeSeed<'de> for IdPreflightSeed {
    type Value = (usize, usize);

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(IdPreflightVisitor)
    }
}

struct IdPreflightVisitor;

impl<'de> Visitor<'de> for IdPreflightVisitor {
    type Value = (usize, usize);

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded expected-ID array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut count = 0_usize;
        let mut bytes = 0_usize;
        while let Some(value) = sequence.next_element::<String>()? {
            if value.len() > 255 {
                return Err(A::Error::custom("ID too long"));
            }
            count = count
                .checked_add(1)
                .ok_or_else(|| A::Error::custom("count overflow"))?;
            if count > MAX_EXPECTED_ROWS {
                return Err(A::Error::custom("too many IDs"));
            }
            bytes = bytes
                .checked_add(value.len())
                .ok_or_else(|| A::Error::custom("byte overflow"))?;
        }
        Ok((count, bytes))
    }
}

pub(super) struct ParsedExpected<I> {
    pub(super) owning_slide_id: String,
    pub(super) selection_rule: String,
    pub(super) ids: Vec<I>,
}

pub(super) fn parse_expected<I: EntitySpec>(
    bytes: &[u8],
    expected_count: usize,
) -> Result<ParsedExpected<I>, MultiscaleEmbeddingError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let parsed = ExpectedCollectSeed::<I> {
        expected_count,
        marker: PhantomData,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    deserializer
        .end()
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    Ok(parsed)
}

struct ExpectedCollectSeed<I> {
    expected_count: usize,
    marker: PhantomData<I>,
}

impl<'de, I: EntitySpec> DeserializeSeed<'de> for ExpectedCollectSeed<I> {
    type Value = ParsedExpected<I>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(ExpectedCollectVisitor::<I> {
            expected_count: self.expected_count,
            marker: PhantomData,
        })
    }
}

struct ExpectedCollectVisitor<I> {
    expected_count: usize,
    marker: PhantomData<I>,
}

impl<'de, I: EntitySpec> Visitor<'de> for ExpectedCollectVisitor<I> {
    type Value = ParsedExpected<I>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an exact Marklab expected-set object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_key(&mut map, "format")?;
        let format: String = map.next_value()?;
        if format != I::EXPECTED_FORMAT {
            return Err(A::Error::custom("wrong format"));
        }
        expect_key(&mut map, "version")?;
        let version: u32 = map.next_value()?;
        if version != EXPECTED_VERSION {
            return Err(A::Error::custom("wrong version"));
        }
        expect_key(&mut map, "owning_slide_id")?;
        let owning_slide_id = map.next_value()?;
        expect_key(&mut map, "selection_rule")?;
        let selection_rule = map.next_value()?;
        expect_key(&mut map, "ids")?;
        let ids = map.next_value_seed(IdCollectSeed::<I> {
            expected_count: self.expected_count,
            marker: PhantomData,
        })?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(ParsedExpected {
            owning_slide_id,
            selection_rule,
            ids,
        })
    }
}

struct IdCollectSeed<I> {
    expected_count: usize,
    marker: PhantomData<I>,
}

impl<'de, I: EntitySpec> DeserializeSeed<'de> for IdCollectSeed<I> {
    type Value = Vec<I>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(IdCollectVisitor::<I> {
            expected_count: self.expected_count,
            marker: PhantomData,
        })
    }
}

struct IdCollectVisitor<I> {
    expected_count: usize,
    marker: PhantomData<I>,
}

impl<'de, I: EntitySpec> Visitor<'de> for IdCollectVisitor<I> {
    type Value = Vec<I>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a preflighted expected-ID array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut ids = Vec::new();
        ids.try_reserve_exact(self.expected_count)
            .map_err(|_| A::Error::custom("allocation failed"))?;
        while let Some(value) = sequence.next_element::<String>()? {
            ids.push(I::parse(value).map_err(|_| A::Error::custom("invalid typed ID"))?);
        }
        if ids.len() != self.expected_count {
            return Err(A::Error::custom("preflight count drift"));
        }
        Ok(ids)
    }
}

fn expect_key<'de, A: MapAccess<'de>>(map: &mut A, expected: &str) -> Result<(), A::Error> {
    let observed = map
        .next_key::<String>()?
        .ok_or_else(|| A::Error::custom("missing field"))?;
    if observed != expected {
        return Err(A::Error::custom("field order mismatch"));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(bound(serialize = ""))]
pub(super) struct ExpectedWireRef<'a, I: EntitySpec> {
    pub(super) format: &'static str,
    pub(super) version: u32,
    pub(super) owning_slide_id: &'a str,
    pub(super) selection_rule: &'a str,
    pub(super) ids: IdSlice<'a, I>,
}

pub(super) struct IdSlice<'a, I>(pub(super) &'a [I]);

impl<I> Copy for IdSlice<'_, I> {}

impl<I> Clone for IdSlice<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: EntitySpec> Serialize for IdSlice<'_, I> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for id in self.0 {
            sequence.serialize_element(id.as_str())?;
        }
        sequence.end()
    }
}
