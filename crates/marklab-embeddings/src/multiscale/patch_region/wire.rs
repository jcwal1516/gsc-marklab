use std::{fmt, mem::size_of};

#[cfg(feature = "parquet")]
use std::io::Read;

use marklab_project::{ArtifactId, ContentDigest};
use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, Visitor},
    Deserializer, Serialize,
};

use super::PatchRegionAssessment;
#[cfg(feature = "parquet")]
use crate::multiscale::json::{
    compare_canonical_json_reader as compare_reader, CanonicalJsonReaderError,
};
use crate::multiscale::{
    error::MultiscaleEmbeddingError,
    json::{canonical_json_len, encode_canonical_json, matches_canonical_json},
    records::codec::{
        expect_header, expect_key, parse_artifact, parse_digest, preflight_json_strings,
        require_decoded, require_hex, serialize_artifact, serialize_digest,
        MAX_RAW_SMALL_JSON_STRING_BYTES, MAX_SMALL_RECORD_BYTES,
    },
};

const FORMAT: &str = "marklab.patch_region_assessment";
const VERSION: u32 = 1;
const POLICY: &str = "expected_cartesian_exhaustive";

pub(super) fn to_canonical_json(
    assessment: &PatchRegionAssessment,
) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
    let wire = WireRef::new(assessment);
    let encoded_len = canonical_json_len(&wire)?;
    encode_canonical_json(&wire, encoded_len, MAX_SMALL_RECORD_BYTES)
}

pub(super) fn validate_canonical_json(
    assessment: &PatchRegionAssessment,
    bytes: &[u8],
    maximum_encoded_bytes: usize,
    maximum_decoded_bytes: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    let effective_maximum = maximum_encoded_bytes.min(MAX_SMALL_RECORD_BYTES);
    if bytes.len() > effective_maximum {
        return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
            observed: bytes.len(),
            maximum: effective_maximum,
        });
    }
    preflight_json_strings(bytes, MAX_RAW_SMALL_JSON_STRING_BYTES)?;
    let decoded_required = size_of::<Parsed<'_>>()
        .checked_add(bytes.len())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    require_decoded(decoded_required, maximum_decoded_bytes)?;
    let parsed = parse(bytes)?;
    let common = &assessment.common;
    let relation_count = u64::try_from(assessment.nonzero_relations.len())
        .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    if parsed.format != FORMAT
        || parsed.version != VERSION
        || parsed.assessment_policy != POLICY
        || parsed.owning_slide_id != common.owning_slide_id.as_str()
        || parsed.expected_patches_artifact_id != common.expected_patches_artifact_id
        || parsed.expected_patches_logical_digest != common.expected_patches_logical_digest
        || parsed.expected_regions_artifact_id != common.expected_regions_artifact_id
        || parsed.expected_regions_logical_digest != common.expected_regions_logical_digest
        || parsed.patch_context_artifact_id != common.patch_context_artifact_id
        || parsed.patch_context_logical_digest != common.patch_context_logical_digest
        || parsed.footprint_artifact_id != common.patch_footprints_artifact_id
        || parsed.footprint_logical_digest != common.patch_footprints_logical_digest
        || parsed.converter_artifact_id != common.converter_artifact_id
        || parsed.assessed_pair_count != common.assessed_pair_count
        || parsed.nonzero_relation_count != relation_count
        || parsed.nonzero_relations_digest != assessment.nonzero_relations_digest
    {
        return Err(MultiscaleEmbeddingError::PatchRegionAssessmentRecordMismatch);
    }
    if !matches_canonical_json(&WireRef::new(assessment), bytes) {
        return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
    }
    Ok(())
}

#[cfg(feature = "parquet")]
pub(super) fn compare_canonical_json_reader<R: Read + ?Sized>(
    assessment: &PatchRegionAssessment,
    reader: &mut R,
) -> Result<(), CanonicalJsonReaderError> {
    let wire = WireRef::new(assessment);
    let encoded_len = canonical_json_len(&wire).map_err(|_| CanonicalJsonReaderError::Mismatch)?;
    compare_reader(&wire, encoded_len, MAX_SMALL_RECORD_BYTES, reader)
}

struct Parsed<'a> {
    format: &'a str,
    version: u32,
    assessment_policy: &'a str,
    owning_slide_id: &'a str,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    expected_regions_artifact_id: ArtifactId,
    expected_regions_logical_digest: ContentDigest,
    patch_context_artifact_id: ArtifactId,
    patch_context_logical_digest: ContentDigest,
    footprint_artifact_id: ArtifactId,
    footprint_logical_digest: ContentDigest,
    converter_artifact_id: ArtifactId,
    assessed_pair_count: u64,
    nonzero_relation_count: u64,
    nonzero_relations_digest: ContentDigest,
}

fn parse(bytes: &[u8]) -> Result<Parsed<'_>, MultiscaleEmbeddingError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let parsed = ParsedSeed
        .deserialize(&mut deserializer)
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    deserializer
        .end()
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    Ok(parsed)
}

struct ParsedSeed;

impl<'de> DeserializeSeed<'de> for ParsedSeed {
    type Value = Parsed<'de>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(ParsedVisitor)
    }
}

struct ParsedVisitor;

impl<'de> Visitor<'de> for ParsedVisitor {
    type Value = Parsed<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an exact patch-region assessment record")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "assessment_policy")?;
        let assessment_policy = map.next_value()?;
        expect_key(&mut map, "owning_slide_id")?;
        let owning_slide_id = map.next_value()?;
        expect_key(&mut map, "expected_patches_artifact_id")?;
        let expected_patches_artifact_id = parse_id(map.next_value()?)?;
        expect_key(&mut map, "expected_patches_logical_digest")?;
        let expected_patches_logical_digest = parse_content_digest(map.next_value()?)?;
        expect_key(&mut map, "expected_regions_artifact_id")?;
        let expected_regions_artifact_id = parse_id(map.next_value()?)?;
        expect_key(&mut map, "expected_regions_logical_digest")?;
        let expected_regions_logical_digest = parse_content_digest(map.next_value()?)?;
        expect_key(&mut map, "patch_context_artifact_id")?;
        let patch_context_artifact_id = parse_id(map.next_value()?)?;
        expect_key(&mut map, "patch_context_logical_digest")?;
        let patch_context_logical_digest = parse_content_digest(map.next_value()?)?;
        expect_key(&mut map, "footprint_artifact_id")?;
        let footprint_artifact_id = parse_id(map.next_value()?)?;
        expect_key(&mut map, "footprint_logical_digest")?;
        let footprint_logical_digest = parse_content_digest(map.next_value()?)?;
        expect_key(&mut map, "converter_artifact_id")?;
        let converter_artifact_id = parse_id(map.next_value()?)?;
        expect_key(&mut map, "assessed_pair_count")?;
        let assessed_pair_count = map.next_value()?;
        expect_key(&mut map, "nonzero_relation_count")?;
        let nonzero_relation_count = map.next_value()?;
        expect_key(&mut map, "nonzero_relations_digest")?;
        let nonzero_relations_digest = parse_content_digest(map.next_value()?)?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Parsed {
            format: FORMAT,
            version: VERSION,
            assessment_policy,
            owning_slide_id,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            expected_regions_artifact_id,
            expected_regions_logical_digest,
            patch_context_artifact_id,
            patch_context_logical_digest,
            footprint_artifact_id,
            footprint_logical_digest,
            converter_artifact_id,
            assessed_pair_count,
            nonzero_relation_count,
            nonzero_relations_digest,
        })
    }
}

fn parse_id<E: serde::de::Error>(value: &str) -> Result<ArtifactId, E> {
    require_hex::<E>(value)?;
    parse_artifact::<E>(value)
}

fn parse_content_digest<E: serde::de::Error>(value: &str) -> Result<ContentDigest, E> {
    require_hex::<E>(value)?;
    parse_digest::<E>(value)
}

#[derive(Serialize)]
struct WireRef<'a> {
    format: &'static str,
    version: u32,
    assessment_policy: &'static str,
    owning_slide_id: &'a str,
    #[serde(serialize_with = "serialize_artifact")]
    expected_patches_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    expected_patches_logical_digest: &'a ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    expected_regions_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    expected_regions_logical_digest: &'a ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    patch_context_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    patch_context_logical_digest: &'a ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    footprint_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    footprint_logical_digest: &'a ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    converter_artifact_id: &'a ArtifactId,
    assessed_pair_count: u64,
    nonzero_relation_count: u64,
    #[serde(serialize_with = "serialize_digest")]
    nonzero_relations_digest: &'a ContentDigest,
}

impl<'a> WireRef<'a> {
    fn new(assessment: &'a PatchRegionAssessment) -> Self {
        let common = &assessment.common;
        Self {
            format: FORMAT,
            version: VERSION,
            assessment_policy: POLICY,
            owning_slide_id: common.owning_slide_id.as_str(),
            expected_patches_artifact_id: &common.expected_patches_artifact_id,
            expected_patches_logical_digest: &common.expected_patches_logical_digest,
            expected_regions_artifact_id: &common.expected_regions_artifact_id,
            expected_regions_logical_digest: &common.expected_regions_logical_digest,
            patch_context_artifact_id: &common.patch_context_artifact_id,
            patch_context_logical_digest: &common.patch_context_logical_digest,
            footprint_artifact_id: &common.patch_footprints_artifact_id,
            footprint_logical_digest: &common.patch_footprints_logical_digest,
            converter_artifact_id: &common.converter_artifact_id,
            assessed_pair_count: common.assessed_pair_count,
            nonzero_relation_count: assessment.nonzero_relations.len() as u64,
            nonzero_relations_digest: &assessment.nonzero_relations_digest,
        }
    }
}
