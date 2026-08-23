use std::fmt;

use marklab_data::SlideId;
use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, Visitor},
    Deserializer, Serialize,
};

use super::{SupportEvidence, FORMAT, VERSION};
use crate::multiscale::{
    error::MultiscaleEmbeddingError,
    records::{
        binding::{BindingWireRef, MultiscaleArtifactBinding},
        codec::{expect_header, expect_key},
    },
};

pub(super) struct Parsed<'a> {
    pub(super) format: &'a str,
    pub(super) version: u32,
    pub(super) variant: &'a str,
    pub(super) entity_kind: &'a str,
    pub(super) owning_slide_id: &'a str,
    pub(super) evidence: SupportEvidence,
}

pub(super) fn parse(bytes: &[u8]) -> Result<Parsed<'_>, MultiscaleEmbeddingError> {
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
        formatter.write_str("an exact multiscale support record")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "variant")?;
        let variant: &str = map.next_value()?;
        expect_key(&mut map, "entity_kind")?;
        let entity_kind = map.next_value()?;
        expect_key(&mut map, "owning_slide_id")?;
        let owning_slide_id = map.next_value()?;
        let evidence = match variant {
            "patch" => {
                expect_key(&mut map, "patch_context")?;
                let context = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                expect_key(&mut map, "patch_footprints")?;
                let footprints = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                expect_key(&mut map, "patch_overlap_graph")?;
                let overlap = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                SupportEvidence::Patch([context, footprints, overlap])
            }
            "region_from_patches" => {
                expect_key(&mut map, "patch_support")?;
                let support = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                expect_key(&mut map, "patch_region_link")?;
                let link = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                SupportEvidence::RegionFromPatches([support, link])
            }
            "slide_from_patches" => {
                expect_key(&mut map, "patch_support")?;
                let support = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                expect_key(&mut map, "source_patch_table")?;
                let table = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                SupportEvidence::SlideFromPatches([support, table])
            }
            "slide_from_regions" => {
                expect_key(&mut map, "region_support")?;
                let support = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                expect_key(&mut map, "source_region_table")?;
                let table = MultiscaleArtifactBinding::parse(map.next_value()?)?;
                SupportEvidence::SlideFromRegions([support, table])
            }
            _ => return Err(A::Error::custom("unsupported support variant")),
        };
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Parsed {
            format: FORMAT,
            version: VERSION,
            variant,
            entity_kind,
            owning_slide_id,
            evidence,
        })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum SupportWireRef<'a> {
    Patch(PatchWireRef<'a>),
    RegionFromPatches(RegionWireRef<'a>),
    SlideFromPatches(SlidePatchWireRef<'a>),
    SlideFromRegions(SlideRegionWireRef<'a>),
}

impl<'a> SupportWireRef<'a> {
    pub(super) fn new(slide: &'a SlideId, evidence: &'a SupportEvidence) -> Self {
        match evidence {
            SupportEvidence::Patch([context, footprints, overlap]) => Self::Patch(PatchWireRef {
                format: FORMAT,
                version: VERSION,
                variant: "patch",
                entity_kind: "patch",
                owning_slide_id: slide.as_str(),
                patch_context: context.wire(),
                patch_footprints: footprints.wire(),
                patch_overlap_graph: overlap.wire(),
            }),
            SupportEvidence::RegionFromPatches([support, link]) => {
                Self::RegionFromPatches(RegionWireRef {
                    format: FORMAT,
                    version: VERSION,
                    variant: "region_from_patches",
                    entity_kind: "region",
                    owning_slide_id: slide.as_str(),
                    patch_support: support.wire(),
                    patch_region_link: link.wire(),
                })
            }
            SupportEvidence::SlideFromPatches([support, table]) => {
                Self::SlideFromPatches(SlidePatchWireRef {
                    format: FORMAT,
                    version: VERSION,
                    variant: "slide_from_patches",
                    entity_kind: "slide",
                    owning_slide_id: slide.as_str(),
                    patch_support: support.wire(),
                    source_patch_table: table.wire(),
                })
            }
            SupportEvidence::SlideFromRegions([support, table]) => {
                Self::SlideFromRegions(SlideRegionWireRef {
                    format: FORMAT,
                    version: VERSION,
                    variant: "slide_from_regions",
                    entity_kind: "slide",
                    owning_slide_id: slide.as_str(),
                    region_support: support.wire(),
                    source_region_table: table.wire(),
                })
            }
        }
    }
}

#[derive(Serialize)]
pub(super) struct PatchWireRef<'a> {
    format: &'static str,
    version: u32,
    variant: &'static str,
    entity_kind: &'static str,
    owning_slide_id: &'a str,
    patch_context: BindingWireRef<'a>,
    patch_footprints: BindingWireRef<'a>,
    patch_overlap_graph: BindingWireRef<'a>,
}

#[derive(Serialize)]
pub(super) struct RegionWireRef<'a> {
    format: &'static str,
    version: u32,
    variant: &'static str,
    entity_kind: &'static str,
    owning_slide_id: &'a str,
    patch_support: BindingWireRef<'a>,
    patch_region_link: BindingWireRef<'a>,
}

#[derive(Serialize)]
pub(super) struct SlidePatchWireRef<'a> {
    format: &'static str,
    version: u32,
    variant: &'static str,
    entity_kind: &'static str,
    owning_slide_id: &'a str,
    patch_support: BindingWireRef<'a>,
    source_patch_table: BindingWireRef<'a>,
}

#[derive(Serialize)]
pub(super) struct SlideRegionWireRef<'a> {
    format: &'static str,
    version: u32,
    variant: &'static str,
    entity_kind: &'static str,
    owning_slide_id: &'a str,
    region_support: BindingWireRef<'a>,
    source_region_table: BindingWireRef<'a>,
}
