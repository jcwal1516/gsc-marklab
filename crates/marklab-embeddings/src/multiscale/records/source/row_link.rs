use std::{fmt, mem::size_of};

use super::{PatchIdentityMap, PatchSourceEntitySet};
use marklab_data::PatchId;
use marklab_project::{ArtifactId, ContentDigest};

mod wire;
use crate::{
    multiscale::{
        digest::LogicalDigest,
        error::MultiscaleEmbeddingError,
        expected::ExpectedPatchSet,
        json::{canonical_json_len, encode_canonical_json, matches_canonical_json},
        records::codec::{
            checked_slots, preflight_json_strings, require_decoded, require_retained,
            validate_source_row_count, MAX_RAW_SOURCE_JSON_STRING_BYTES, MAX_SOURCE_RECORD_BYTES,
        },
    },
    EmbeddingStatus,
};
use wire::{parse_collect, parse_preflight, EntrySlice, WireRef};

const FORMAT: &str = "marklab.patch_embedding_source_row_link";
const VERSION: u32 = 1;
const DOMAIN: &[u8] = b"marklab-patch-source-row-link-logical-v1";

/// One expected patch's exact source-entity and optional source-vector rows.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchEmbeddingSourceRowLinkEntry {
    patch_id: PatchId,
    status: EmbeddingStatus,
    source_entity_row: u64,
    source_vector_row: Option<u64>,
}

impl fmt::Debug for PatchEmbeddingSourceRowLinkEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchEmbeddingSourceRowLinkEntry")
            .field("status", &self.status)
            .finish()
    }
}

impl PatchEmbeddingSourceRowLinkEntry {
    /// Validate status-specific source-vector-row presence.
    pub fn new(
        patch_id: PatchId,
        status: EmbeddingStatus,
        source_entity_row: u64,
        source_vector_row: Option<u64>,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if matches!(
            status,
            EmbeddingStatus::Present | EmbeddingStatus::QcRejected
        ) != source_vector_row.is_some()
        {
            return Err(MultiscaleEmbeddingError::PatchSourceRowStatusMismatch);
        }
        Ok(Self {
            patch_id,
            status,
            source_entity_row,
            source_vector_row,
        })
    }

    /// Bind an accepted source vector.
    pub fn present(patch_id: PatchId, source_entity_row: u64, source_vector_row: u64) -> Self {
        Self {
            patch_id,
            status: EmbeddingStatus::Present,
            source_entity_row,
            source_vector_row: Some(source_vector_row),
        }
    }

    /// Retain an explicit missing-vector state.
    pub fn missing_vector(patch_id: PatchId, source_entity_row: u64) -> Self {
        Self {
            patch_id,
            status: EmbeddingStatus::MissingVector,
            source_entity_row,
            source_vector_row: None,
        }
    }

    /// Retain an explicit extraction-failed state.
    pub fn extraction_failed(patch_id: PatchId, source_entity_row: u64) -> Self {
        Self {
            patch_id,
            status: EmbeddingStatus::ExtractionFailed,
            source_entity_row,
            source_vector_row: None,
        }
    }

    /// Bind a source vector that declared QC rejected.
    pub fn qc_rejected(patch_id: PatchId, source_entity_row: u64, source_vector_row: u64) -> Self {
        Self {
            patch_id,
            status: EmbeddingStatus::QcRejected,
            source_entity_row,
            source_vector_row: Some(source_vector_row),
        }
    }

    /// Canonical expected patch identity.
    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    /// Closed embedding status.
    pub fn status(&self) -> EmbeddingStatus {
        self.status
    }

    /// Exact source-entity row associated through the identity map.
    pub fn source_entity_row(&self) -> u64 {
        self.source_entity_row
    }

    /// Gap-free source-vector row when a vector exists.
    pub fn source_vector_row(&self) -> Option<u64> {
        self.source_vector_row
    }
}

/// Exact expected-patch-order source row link for direct patch extraction.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchEmbeddingSourceRowLink {
    source_entities_artifact_id: ArtifactId,
    source_entities_logical_digest: ContentDigest,
    source_vectors_artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    identity_map_artifact_id: ArtifactId,
    identity_map_logical_digest: ContentDigest,
    converter_artifact_id: ArtifactId,
    entries: Box<[PatchEmbeddingSourceRowLinkEntry]>,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

impl fmt::Debug for PatchEmbeddingSourceRowLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchEmbeddingSourceRowLink")
            .field("row_count", &self.entries.len())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchEmbeddingSourceRowLink {
    /// Validate exact domains, status/nullability, row permutations, bindings, and retained bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_entities: &PatchSourceEntitySet,
        source_entities_artifact_id: ArtifactId,
        source_vectors_artifact_id: ArtifactId,
        expected_patches: &ExpectedPatchSet,
        expected_patches_artifact_id: ArtifactId,
        identity_map: &PatchIdentityMap,
        identity_map_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        mut entries: Vec<PatchEmbeddingSourceRowLinkEntry>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_source_row_count(entries.len())?;
        let dependencies = [
            source_entities_artifact_id,
            source_vectors_artifact_id,
            expected_patches_artifact_id,
            identity_map_artifact_id,
            converter_artifact_id,
        ];
        let mut sorted_dependencies = dependencies;
        sorted_dependencies.sort_unstable();
        if sorted_dependencies
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(MultiscaleEmbeddingError::DuplicatePatchSourceArtifactDependency);
        }
        if identity_map.source_entities_artifact_id() != source_entities_artifact_id
            || identity_map.source_entities_logical_digest() != source_entities.logical_digest()
            || identity_map.expected_patches_artifact_id() != expected_patches_artifact_id
            || identity_map.expected_patches_logical_digest() != expected_patches.logical_digest()
            || entries.len() != expected_patches.ids().len()
            || identity_map.source_rows_by_expected_patch().len() != entries.len()
        {
            return Err(MultiscaleEmbeddingError::PatchSourceRowLinkMismatch);
        }
        for (row, ((entry, expected), source_row)) in entries
            .iter()
            .zip(expected_patches.ids())
            .zip(identity_map.source_rows_by_expected_patch())
            .enumerate()
        {
            if &entry.patch_id != expected
                || entry.source_entity_row != *source_row
                || (matches!(
                    entry.status,
                    EmbeddingStatus::Present | EmbeddingStatus::QcRejected
                ) != entry.source_vector_row.is_some())
            {
                let _ = row;
                return Err(MultiscaleEmbeddingError::PatchSourceRowLinkMismatch);
            }
        }
        let patch_bytes = entries.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.patch_id.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        })?;
        let required = retained_bytes(entries.capacity(), patch_bytes)?;
        require_retained(required, maximum_retained_bytes)?;
        entries.sort_unstable_by_key(|entry| entry.source_vector_row);
        if entries
            .iter()
            .filter_map(|entry| entry.source_vector_row)
            .enumerate()
            .any(|(expected, observed)| u64::try_from(expected).ok() != Some(observed))
        {
            return Err(MultiscaleEmbeddingError::PatchSourceRowLinkMismatch);
        }
        entries.sort_unstable_by(|left, right| left.patch_id.cmp(&right.patch_id));

        let source_entities_logical_digest = source_entities.logical_digest();
        let expected_patches_logical_digest = expected_patches.logical_digest();
        let identity_map_logical_digest = identity_map.logical_digest();
        let logical_digest = row_link_digest(
            source_entities_artifact_id,
            source_entities_logical_digest,
            source_vectors_artifact_id,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            identity_map_artifact_id,
            identity_map_logical_digest,
            converter_artifact_id,
            &entries,
        )?;
        let wire = WireRef {
            format: FORMAT,
            version: VERSION,
            source_entities_artifact_id,
            source_entities_logical_digest,
            source_vectors_artifact_id,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            identity_map_artifact_id,
            identity_map_logical_digest,
            converter_artifact_id,
            entries: EntrySlice(&entries),
        };
        let encoded_len = canonical_json_len(&wire)?;
        if encoded_len > MAX_SOURCE_RECORD_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_SOURCE_RECORD_BYTES,
            });
        }
        Ok(Self {
            source_entities_artifact_id,
            source_entities_logical_digest,
            source_vectors_artifact_id,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            identity_map_artifact_id,
            identity_map_logical_digest,
            converter_artifact_id,
            entries: entries.into_boxed_slice(),
            logical_digest,
            encoded_len,
        })
    }

    /// Decode with count/string preflight and exact source-chain rebinding.
    #[allow(clippy::too_many_arguments)]
    pub fn from_canonical_json(
        bytes: &[u8],
        source_entities: &PatchSourceEntitySet,
        expected_patches: &ExpectedPatchSet,
        identity_map: &PatchIdentityMap,
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let effective_maximum = maximum_encoded_bytes.min(MAX_SOURCE_RECORD_BYTES);
        if bytes.len() > effective_maximum {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: effective_maximum,
            });
        }
        preflight_json_strings(bytes, MAX_RAW_SOURCE_JSON_STRING_BYTES)?;
        let preflight = parse_preflight(bytes)?;
        validate_source_row_count(preflight.entry_count)?;
        require_decoded(preflight.decoded_bytes()?, maximum_decoded_bytes)?;
        require_retained(preflight.retained_bytes()?, maximum_retained_bytes)?;
        let parsed = parse_collect(bytes, preflight.entry_count)?;
        if parsed.source_entities_logical_digest != source_entities.logical_digest()
            || parsed.expected_patches_logical_digest != expected_patches.logical_digest()
            || parsed.identity_map_logical_digest != identity_map.logical_digest()
        {
            return Err(MultiscaleEmbeddingError::PatchSourceRowLinkMismatch);
        }
        let link = Self::new(
            source_entities,
            parsed.source_entities_artifact_id,
            parsed.source_vectors_artifact_id,
            expected_patches,
            parsed.expected_patches_artifact_id,
            identity_map,
            parsed.identity_map_artifact_id,
            parsed.converter_artifact_id,
            parsed.entries,
            maximum_retained_bytes,
        )?;
        if !matches_canonical_json(&link.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(link)
    }

    /// Encode the exact version-one canonical JSON with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_SOURCE_RECORD_BYTES)
    }

    /// Source-entity artifact identity.
    pub fn source_entities_artifact_id(&self) -> ArtifactId {
        self.source_entities_artifact_id
    }

    /// Source-entity logical identity.
    pub fn source_entities_logical_digest(&self) -> ContentDigest {
        self.source_entities_logical_digest
    }

    /// Opaque source-vector artifact identity.
    pub fn source_vectors_artifact_id(&self) -> ArtifactId {
        self.source_vectors_artifact_id
    }

    /// Expected-patch artifact identity.
    pub fn expected_patches_artifact_id(&self) -> ArtifactId {
        self.expected_patches_artifact_id
    }

    /// Expected-patch logical identity.
    pub fn expected_patches_logical_digest(&self) -> ContentDigest {
        self.expected_patches_logical_digest
    }

    /// Identity-map artifact identity.
    pub fn identity_map_artifact_id(&self) -> ArtifactId {
        self.identity_map_artifact_id
    }

    /// Identity-map logical identity.
    pub fn identity_map_logical_digest(&self) -> ContentDigest {
        self.identity_map_logical_digest
    }

    /// Converter artifact identity.
    pub fn converter_artifact_id(&self) -> ArtifactId {
        self.converter_artifact_id
    }

    /// Canonical expected-patch-order entries.
    pub fn entries(&self) -> &[PatchEmbeddingSourceRowLinkEntry] {
        &self.entries
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    /// Sorted distinct record dependency set.
    pub fn direct_dependencies(&self) -> [ArtifactId; 5] {
        let mut dependencies = [
            self.source_entities_artifact_id,
            self.source_vectors_artifact_id,
            self.expected_patches_artifact_id,
            self.identity_map_artifact_id,
            self.converter_artifact_id,
        ];
        dependencies.sort_unstable();
        dependencies
    }

    fn wire(&self) -> WireRef<'_> {
        WireRef {
            format: FORMAT,
            version: VERSION,
            source_entities_artifact_id: self.source_entities_artifact_id,
            source_entities_logical_digest: self.source_entities_logical_digest,
            source_vectors_artifact_id: self.source_vectors_artifact_id,
            expected_patches_artifact_id: self.expected_patches_artifact_id,
            expected_patches_logical_digest: self.expected_patches_logical_digest,
            identity_map_artifact_id: self.identity_map_artifact_id,
            identity_map_logical_digest: self.identity_map_logical_digest,
            converter_artifact_id: self.converter_artifact_id,
            entries: EntrySlice(&self.entries),
        }
    }
}

fn retained_bytes(
    entry_capacity: usize,
    patch_bytes: usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<PatchEmbeddingSourceRowLink>()
        .checked_add(checked_slots::<PatchEmbeddingSourceRowLinkEntry>(
            entry_capacity,
        )?)
        .and_then(|value| value.checked_add(patch_bytes))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

#[allow(clippy::too_many_arguments)]
fn row_link_digest(
    source_entities_artifact_id: ArtifactId,
    source_entities_logical_digest: ContentDigest,
    source_vectors_artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    identity_map_artifact_id: ArtifactId,
    identity_map_logical_digest: ContentDigest,
    converter_artifact_id: ArtifactId,
    entries: &[PatchEmbeddingSourceRowLinkEntry],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(DOMAIN);
    digest.text(FORMAT);
    digest.u32(VERSION);
    digest.artifact_id(source_entities_artifact_id);
    digest.content_digest(source_entities_logical_digest);
    digest.artifact_id(source_vectors_artifact_id);
    digest.artifact_id(expected_patches_artifact_id);
    digest.content_digest(expected_patches_logical_digest);
    digest.artifact_id(identity_map_artifact_id);
    digest.content_digest(identity_map_logical_digest);
    digest.artifact_id(converter_artifact_id);
    digest.array_len(entries.len())?;
    for entry in entries {
        digest.text(entry.patch_id.as_str());
        digest.text(entry.status.wire_name());
        digest.u64(entry.source_entity_row);
        match entry.source_vector_row {
            Some(row) => {
                digest.u8(1);
                digest.u64(row);
            }
            None => digest.u8(0),
        }
    }
    Ok(digest.finish())
}
