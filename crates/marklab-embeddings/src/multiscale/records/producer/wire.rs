use std::fmt;

use marklab_project::ArtifactId;
use serde::{
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, Visitor},
    Deserializer, Serialize,
};

use super::{CellPatchLinkProducer, FORMAT, VERSION};
use crate::multiscale::records::codec::{
    expect_header, expect_key, parse_artifact, require_hex, serialize_artifact,
};

pub(super) struct Parsed<'a> {
    pub(super) format: &'a str,
    pub(super) version: u32,
    pub(super) assignment_mode: &'a str,
    pub(super) algorithm: &'a str,
    pub(super) algorithm_version: &'a str,
    pub(super) role_artifact_ids: [ArtifactId; 4],
}

pub(super) fn parse(
    bytes: &[u8],
) -> Result<Parsed<'_>, crate::multiscale::error::MultiscaleEmbeddingError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let parsed = ParsedSeed
        .deserialize(&mut deserializer)
        .map_err(|_| crate::multiscale::error::MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    deserializer
        .end()
        .map_err(|_| crate::multiscale::error::MultiscaleEmbeddingError::InvalidCanonicalJson)?;
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
        formatter.write_str("an exact cell-patch producer record")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        expect_header(&mut map, FORMAT, VERSION)?;
        expect_key(&mut map, "assignment_mode")?;
        let assignment_mode = map.next_value()?;
        expect_key(&mut map, "algorithm")?;
        let algorithm = map.next_value()?;
        expect_key(&mut map, "algorithm_version")?;
        let algorithm_version = map.next_value()?;
        expect_key(&mut map, "source_coordinates_artifact_id")?;
        let source_coordinates = parse_id(map.next_value()?)?;
        expect_key(&mut map, "run_config_artifact_id")?;
        let run_config = parse_id(map.next_value()?)?;
        expect_key(&mut map, "environment_artifact_id")?;
        let environment = parse_id(map.next_value()?)?;
        expect_key(&mut map, "converter_artifact_id")?;
        let converter = parse_id(map.next_value()?)?;
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("extra field"));
        }
        Ok(Parsed {
            format: FORMAT,
            version: VERSION,
            assignment_mode,
            algorithm,
            algorithm_version,
            role_artifact_ids: [source_coordinates, run_config, environment, converter],
        })
    }
}

fn parse_id<E: serde::de::Error>(value: &str) -> Result<ArtifactId, E> {
    require_hex::<E>(value)?;
    parse_artifact::<E>(value)
}

#[derive(Serialize)]
pub(super) struct ProducerWireRef<'a> {
    format: &'static str,
    version: u32,
    assignment_mode: &'static str,
    algorithm: &'a str,
    algorithm_version: &'a str,
    #[serde(serialize_with = "serialize_artifact")]
    source_coordinates_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    run_config_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    environment_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    converter_artifact_id: &'a ArtifactId,
}

impl<'a> ProducerWireRef<'a> {
    pub(super) fn new(producer: &'a CellPatchLinkProducer) -> Self {
        let assignment_mode = match producer.assignment_mode {
            crate::multiscale::cell_patch::CellPatchAssignmentMode::ContainedShared => {
                "contained_shared"
            }
            crate::multiscale::cell_patch::CellPatchAssignmentMode::DeclaredWeightedInterpolation => {
                "declared_weighted_interpolation"
            }
        };
        Self {
            format: FORMAT,
            version: VERSION,
            assignment_mode,
            algorithm: producer.algorithm.as_str(),
            algorithm_version: &producer.algorithm_version,
            source_coordinates_artifact_id: &producer.role_artifact_ids[0],
            run_config_artifact_id: &producer.role_artifact_ids[1],
            environment_artifact_id: &producer.role_artifact_ids[2],
            converter_artifact_id: &producer.role_artifact_ids[3],
        }
    }
}
