use std::fmt;

use marklab_project::{ArtifactId, ContentDigest};
use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, Visitor},
    Deserializer,
};

mod serialize;
pub(super) use serialize::ProvenanceWireRef;

use super::{
    types::{
        MultiscaleDirectPatchInputArtifacts, MultiscaleDirectPatchModelProvenance,
        MultiscaleEmbeddingExecutionProvenance,
    },
    DerivedRegionEvidence, DerivedSlideEvidence, DirectPatchEvidence, ProvenanceEvidence, FORMAT,
    VERSION,
};
use crate::multiscale::{
    error::MultiscaleEmbeddingError,
    records::codec::{expect_header, expect_key, parse_artifact, parse_digest, require_hex},
};

pub(super) struct Parsed {
    pub(super) variant: String,
    pub(super) entity_kind: String,
    pub(super) owning_slide_id: String,
    pub(super) output_dimension: u32,
    pub(super) dtype: String,
    pub(super) pooling_or_aggregation: String,
    pub(super) evidence: ParsedEvidence,
}

impl Parsed {
    pub(super) fn heap_text_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        let evidence = self.evidence.heap_text_bytes()?;
        match self.evidence {
            ParsedEvidence::DirectPatch(_) => self
                .pooling_or_aggregation
                .len()
                .checked_add(evidence)
                .ok_or(MultiscaleEmbeddingError::SizeOverflow),
            _ => Ok(evidence),
        }
    }
}

// Keeping the fixed roles inline avoids a hidden allocation beyond the decoded strings before the
// exact decoded/retained accounting has completed.
#[allow(clippy::large_enum_variant)]
pub(super) enum ParsedEvidence {
    DirectPatch(ParsedDirectPatch),
    DerivedRegion(ParsedDerivedRegion),
    DerivedSlideFromPatches(ParsedDerivedSlide),
    DerivedSlideFromRegions(ParsedDerivedSlide),
}

impl ParsedEvidence {
    fn heap_text_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        match self {
            Self::DirectPatch(value) => sum_text_bytes(&[
                &value.model_family,
                &value.model_version,
                &value.encoder_architecture,
                &value.license_spdx,
                &value.citation,
                &value.extraction_tensor,
                &value.execution.converter_name,
                &value.execution.converter_version,
            ]),
            Self::DerivedRegion(value) => sum_text_bytes(&[
                &value.execution.converter_name,
                &value.execution.converter_version,
            ]),
            Self::DerivedSlideFromPatches(value) | Self::DerivedSlideFromRegions(value) => {
                sum_text_bytes(&[
                    &value.execution.converter_name,
                    &value.execution.converter_version,
                ])
            }
        }
    }

    pub(super) fn into_evidence(
        self,
        pooling: String,
    ) -> Result<ProvenanceEvidence, MultiscaleEmbeddingError> {
        match self {
            Self::DirectPatch(value) => {
                let model = MultiscaleDirectPatchModelProvenance::from_owned(
                    value.model_family,
                    value.model_version,
                    value.encoder_architecture,
                    value.checkpoint_artifact_id,
                    value.checkpoint_content_sha256,
                    value.source_snapshot_artifact_id,
                    value.license_record_artifact_id,
                    value.license_spdx,
                    value.citation,
                    value.extraction_tensor,
                    value.extraction_layer,
                )?;
                Ok(ProvenanceEvidence::DirectPatch(DirectPatchEvidence {
                    pooling: pooling.into_boxed_str(),
                    model,
                    execution: value.execution.into_execution()?,
                    preprocessing_artifact_id: value.preprocessing_artifact_id,
                    inputs: MultiscaleDirectPatchInputArtifacts::new(
                        value.input_normalization_artifact_id,
                        value.source_entities_artifact_id,
                        value.source_vectors_artifact_id,
                        value.expected_patches_artifact_id,
                        value.identity_map_artifact_id,
                        value.source_row_link_artifact_id,
                        value.patch_support_artifact_id,
                    ),
                }))
            }
            Self::DerivedRegion(value) => {
                Ok(ProvenanceEvidence::DerivedRegion(DerivedRegionEvidence {
                    execution: value.execution.into_execution()?,
                    source_patch_table_artifact_id: value.source_patch_table_artifact_id,
                    patch_region_link_artifact_id: value.patch_region_link_artifact_id,
                    expected_regions_artifact_id: value.expected_regions_artifact_id,
                    region_support_artifact_id: value.region_support_artifact_id,
                    derivation_contract_artifact_id: value.derivation_contract_artifact_id,
                }))
            }
            Self::DerivedSlideFromPatches(value) => Ok(
                ProvenanceEvidence::DerivedSlideFromPatches(value.into_evidence()?),
            ),
            Self::DerivedSlideFromRegions(value) => Ok(
                ProvenanceEvidence::DerivedSlideFromRegions(value.into_evidence()?),
            ),
        }
    }
}

fn sum_text_bytes(values: &[&str]) -> Result<usize, MultiscaleEmbeddingError> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    })
}

pub(super) struct ParsedExecution {
    run_config_artifact_id: ArtifactId,
    environment_artifact_id: ArtifactId,
    converter_artifact_id: ArtifactId,
    converter_name: String,
    converter_version: String,
}

impl ParsedExecution {
    fn into_execution(
        self,
    ) -> Result<MultiscaleEmbeddingExecutionProvenance, MultiscaleEmbeddingError> {
        MultiscaleEmbeddingExecutionProvenance::from_owned(
            self.run_config_artifact_id,
            self.environment_artifact_id,
            self.converter_artifact_id,
            self.converter_name,
            self.converter_version,
        )
    }
}

pub(super) struct ParsedDirectPatch {
    model_family: String,
    model_version: String,
    encoder_architecture: String,
    checkpoint_artifact_id: ArtifactId,
    checkpoint_content_sha256: ContentDigest,
    source_snapshot_artifact_id: ArtifactId,
    license_record_artifact_id: ArtifactId,
    license_spdx: String,
    citation: String,
    extraction_tensor: String,
    extraction_layer: u32,
    input_normalization_artifact_id: ArtifactId,
    preprocessing_artifact_id: ArtifactId,
    execution: ParsedExecution,
    source_entities_artifact_id: ArtifactId,
    source_vectors_artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    identity_map_artifact_id: ArtifactId,
    source_row_link_artifact_id: ArtifactId,
    patch_support_artifact_id: ArtifactId,
}

pub(super) struct ParsedDerivedRegion {
    execution: ParsedExecution,
    source_patch_table_artifact_id: ArtifactId,
    patch_region_link_artifact_id: ArtifactId,
    expected_regions_artifact_id: ArtifactId,
    region_support_artifact_id: ArtifactId,
    derivation_contract_artifact_id: ArtifactId,
}

pub(super) struct ParsedDerivedSlide {
    execution: ParsedExecution,
    source_table_artifact_id: ArtifactId,
    expected_slides_artifact_id: ArtifactId,
    slide_support_artifact_id: ArtifactId,
    derivation_contract_artifact_id: ArtifactId,
}

impl ParsedDerivedSlide {
    fn into_evidence(self) -> Result<DerivedSlideEvidence, MultiscaleEmbeddingError> {
        Ok(DerivedSlideEvidence {
            execution: self.execution.into_execution()?,
            source_table_artifact_id: self.source_table_artifact_id,
            expected_slides_artifact_id: self.expected_slides_artifact_id,
            slide_support_artifact_id: self.slide_support_artifact_id,
            derivation_contract_artifact_id: self.derivation_contract_artifact_id,
        })
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Parsed, MultiscaleEmbeddingError> {
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
    type Value = Parsed;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(ParsedVisitor)
    }
}

struct ParsedVisitor;

impl<'de> Visitor<'de> for ParsedVisitor {
    type Value = Parsed;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an exact multiscale embedding provenance record")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "variant")?;
        let variant: String = map.next_value()?;
        expect_key(&mut map, "entity_kind")?;
        let entity_kind = map.next_value()?;
        expect_key(&mut map, "owning_slide_id")?;
        let owning_slide_id = map.next_value()?;
        expect_key(&mut map, "output_dimension")?;
        let output_dimension = map.next_value()?;
        expect_key(&mut map, "dtype")?;
        let dtype = map.next_value()?;
        expect_key(&mut map, "pooling_or_aggregation")?;
        let pooling_or_aggregation = map.next_value()?;
        let evidence = match variant.as_str() {
            "direct_patch" => ParsedEvidence::DirectPatch(parse_direct(&mut map)?),
            "derived_region" => ParsedEvidence::DerivedRegion(parse_region(&mut map)?),
            "derived_slide_from_patches" => {
                ParsedEvidence::DerivedSlideFromPatches(parse_slide(&mut map, true)?)
            }
            "derived_slide_from_regions" => {
                ParsedEvidence::DerivedSlideFromRegions(parse_slide(&mut map, false)?)
            }
            _ => return Err(A::Error::custom("unsupported provenance variant")),
        };
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Parsed {
            variant,
            entity_kind,
            owning_slide_id,
            output_dimension,
            dtype,
            pooling_or_aggregation,
            evidence,
        })
    }
}

fn parse_direct<'de, A: MapAccess<'de>>(map: &mut A) -> Result<ParsedDirectPatch, A::Error> {
    expect_key(map, "model_family")?;
    let model_family = map.next_value()?;
    expect_key(map, "model_version")?;
    let model_version = map.next_value()?;
    expect_key(map, "encoder_architecture")?;
    let encoder_architecture = map.next_value()?;
    expect_key(map, "checkpoint_artifact_id")?;
    let checkpoint_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "checkpoint_content_sha256")?;
    let checkpoint_content_sha256 = parse_content_digest(map.next_value()?)?;
    expect_key(map, "source_snapshot_artifact_id")?;
    let source_snapshot_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "license_record_artifact_id")?;
    let license_record_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "license_spdx")?;
    let license_spdx = map.next_value()?;
    expect_key(map, "citation")?;
    let citation = map.next_value()?;
    expect_key(map, "extraction_tensor")?;
    let extraction_tensor = map.next_value()?;
    expect_key(map, "extraction_layer")?;
    let extraction_layer = map.next_value()?;
    expect_key(map, "input_normalization_artifact_id")?;
    let input_normalization_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "preprocessing_artifact_id")?;
    let preprocessing_artifact_id = parse_id(map.next_value()?)?;
    let execution = parse_execution(map)?;
    expect_key(map, "source_entities_artifact_id")?;
    let source_entities_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "source_vectors_artifact_id")?;
    let source_vectors_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "expected_patches_artifact_id")?;
    let expected_patches_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "identity_map_artifact_id")?;
    let identity_map_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "source_row_link_artifact_id")?;
    let source_row_link_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "patch_support_artifact_id")?;
    let patch_support_artifact_id = parse_id(map.next_value()?)?;
    Ok(ParsedDirectPatch {
        model_family,
        model_version,
        encoder_architecture,
        checkpoint_artifact_id,
        checkpoint_content_sha256,
        source_snapshot_artifact_id,
        license_record_artifact_id,
        license_spdx,
        citation,
        extraction_tensor,
        extraction_layer,
        input_normalization_artifact_id,
        preprocessing_artifact_id,
        execution,
        source_entities_artifact_id,
        source_vectors_artifact_id,
        expected_patches_artifact_id,
        identity_map_artifact_id,
        source_row_link_artifact_id,
        patch_support_artifact_id,
    })
}

fn parse_execution<'de, A: MapAccess<'de>>(map: &mut A) -> Result<ParsedExecution, A::Error> {
    expect_key(map, "run_config_artifact_id")?;
    let run_config_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "environment_artifact_id")?;
    let environment_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "converter_artifact_id")?;
    let converter_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "converter_name")?;
    let converter_name = map.next_value()?;
    expect_key(map, "converter_version")?;
    let converter_version = map.next_value()?;
    Ok(ParsedExecution {
        run_config_artifact_id,
        environment_artifact_id,
        converter_artifact_id,
        converter_name,
        converter_version,
    })
}

fn parse_region<'de, A: MapAccess<'de>>(map: &mut A) -> Result<ParsedDerivedRegion, A::Error> {
    let execution = parse_execution(map)?;
    expect_key(map, "source_patch_table_artifact_id")?;
    let source_patch_table_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "patch_region_link_artifact_id")?;
    let patch_region_link_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "expected_regions_artifact_id")?;
    let expected_regions_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "region_support_artifact_id")?;
    let region_support_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "derivation_contract_artifact_id")?;
    let derivation_contract_artifact_id = parse_id(map.next_value()?)?;
    Ok(ParsedDerivedRegion {
        execution,
        source_patch_table_artifact_id,
        patch_region_link_artifact_id,
        expected_regions_artifact_id,
        region_support_artifact_id,
        derivation_contract_artifact_id,
    })
}

fn parse_slide<'de, A: MapAccess<'de>>(
    map: &mut A,
    from_patches: bool,
) -> Result<ParsedDerivedSlide, A::Error> {
    let execution = parse_execution(map)?;
    expect_key(
        map,
        if from_patches {
            "source_patch_table_artifact_id"
        } else {
            "source_region_table_artifact_id"
        },
    )?;
    let source_table_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "expected_slides_artifact_id")?;
    let expected_slides_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "slide_support_artifact_id")?;
    let slide_support_artifact_id = parse_id(map.next_value()?)?;
    expect_key(map, "derivation_contract_artifact_id")?;
    let derivation_contract_artifact_id = parse_id(map.next_value()?)?;
    Ok(ParsedDerivedSlide {
        execution,
        source_table_artifact_id,
        expected_slides_artifact_id,
        slide_support_artifact_id,
        derivation_contract_artifact_id,
    })
}

fn parse_id<E: serde::de::Error>(value: &str) -> Result<ArtifactId, E> {
    require_hex::<E>(value)?;
    parse_artifact::<E>(value)
}

fn parse_content_digest<E: serde::de::Error>(value: &str) -> Result<ContentDigest, E> {
    require_hex::<E>(value)?;
    parse_digest::<E>(value)
}
