use std::fmt;

use marklab_project::{ArtifactId, ContentDigest};
use serde::{Deserialize, Serialize};

use super::codec::{
    parse_artifact, parse_digest, require_hex, serialize_artifact, serialize_digest,
};

/// Exact artifact identity plus the format-independent logical identity it is expected to carry.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct MultiscaleArtifactBinding {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
}

impl fmt::Debug for MultiscaleArtifactBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiscaleArtifactBinding")
            .finish_non_exhaustive()
    }
}

impl MultiscaleArtifactBinding {
    /// Bind one artifact identity to its exact logical digest.
    pub fn new(artifact_id: ArtifactId, logical_digest: ContentDigest) -> Self {
        Self {
            artifact_id,
            logical_digest,
        }
    }

    /// Artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Expected format-independent logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    pub(super) fn wire(&self) -> BindingWireRef<'_> {
        BindingWireRef {
            artifact_id: &self.artifact_id,
            logical_digest: &self.logical_digest,
        }
    }

    pub(super) fn parse<E: serde::de::Error>(wire: BindingWireBorrowed<'_>) -> Result<Self, E> {
        require_hex::<E>(wire.artifact_id)?;
        require_hex::<E>(wire.logical_digest)?;
        Ok(Self::new(
            parse_artifact::<E>(wire.artifact_id)?,
            parse_digest::<E>(wire.logical_digest)?,
        ))
    }
}

#[derive(Serialize)]
pub(super) struct BindingWireRef<'a> {
    #[serde(serialize_with = "serialize_artifact")]
    artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    logical_digest: &'a ContentDigest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BindingWireBorrowed<'a> {
    pub(super) artifact_id: &'a str,
    pub(super) logical_digest: &'a str,
}
