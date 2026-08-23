use std::{fmt, mem::size_of};

use super::entity::PatchSourceEntitySet;
use marklab_data::PatchId;
use marklab_project::{ArtifactId, ContentDigest};

mod wire;
use crate::multiscale::{
    digest::LogicalDigest,
    error::MultiscaleEmbeddingError,
    expected::ExpectedPatchSet,
    json::{canonical_json_len, encode_canonical_json, matches_canonical_json},
    records::codec::{
        checked_slots, preflight_json_strings, require_decoded, require_retained, try_vec_capacity,
        valid_source_key, validate_source_row_count, MAX_RAW_SOURCE_JSON_STRING_BYTES,
        MAX_SOURCE_RECORD_BYTES,
    },
};
use wire::{parse_collect, parse_preflight, EntrySlice, WireRef};

const FORMAT: &str = "marklab.patch_identity_map";
const VERSION: u32 = 1;
const DOMAIN: &[u8] = b"marklab-patch-identity-map-logical-v1";

/// One explicit private source-local patch key to canonical `PatchId` mapping.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchIdentityMapEntry {
    source_patch_id: Box<str>,
    patch_id: PatchId,
}

impl fmt::Debug for PatchIdentityMapEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchIdentityMapEntry")
            .finish_non_exhaustive()
    }
}

impl PatchIdentityMapEntry {
    /// Validate one bounded source key and retain its explicit typed target.
    pub fn new(
        source_patch_id: impl Into<String>,
        patch_id: PatchId,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let source_patch_id = source_patch_id.into();
        if !valid_source_key(&source_patch_id) {
            return Err(MultiscaleEmbeddingError::InvalidPatchSourceKey);
        }
        Ok(Self {
            source_patch_id: source_patch_id.into_boxed_str(),
            patch_id,
        })
    }

    /// Exact private source-local patch key.
    pub fn source_patch_id(&self) -> &str {
        &self.source_patch_id
    }

    /// Explicit canonical patch target.
    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }
}

/// Exact one-to-one mapping from a source entity set to expected patches.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchIdentityMap {
    source_entities_artifact_id: ArtifactId,
    source_entities_logical_digest: ContentDigest,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    entries: Box<[PatchIdentityMapEntry]>,
    source_rows_by_expected_patch: Box<[u64]>,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

impl fmt::Debug for PatchIdentityMap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchIdentityMap")
            .field("row_count", &self.entries.len())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchIdentityMap {
    /// Validate exact source domain, expected typed range, bindings, and retained bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_entities: &PatchSourceEntitySet,
        source_entities_artifact_id: ArtifactId,
        expected_patches: &ExpectedPatchSet,
        expected_patches_artifact_id: ArtifactId,
        mut entries: Vec<PatchIdentityMapEntry>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_source_row_count(entries.len())?;
        if source_entities_artifact_id == expected_patches_artifact_id {
            return Err(MultiscaleEmbeddingError::DuplicatePatchSourceArtifactDependency);
        }
        if entries.len() != source_entities.entries().len()
            || entries.len() != expected_patches.ids().len()
            || entries
                .iter()
                .zip(source_entities.entries())
                .any(|(entry, source)| entry.source_patch_id() != source.source_patch_id())
        {
            return Err(MultiscaleEmbeddingError::PatchIdentityMapMismatch);
        }
        let source_bytes = entries.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.source_patch_id.len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        })?;
        let patch_bytes = entries.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.patch_id.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        })?;
        let required =
            retained_bytes(entries.capacity(), source_bytes, patch_bytes, entries.len())?;
        require_retained(required, maximum_retained_bytes)?;

        entries.sort_unstable_by(|left, right| left.patch_id.cmp(&right.patch_id));
        if entries
            .iter()
            .zip(expected_patches.ids())
            .any(|(entry, expected)| &entry.patch_id != expected)
        {
            return Err(MultiscaleEmbeddingError::PatchIdentityMapMismatch);
        }
        let mut source_rows_by_expected_patch = try_vec_capacity::<u64>(entries.len())?;
        for entry in &entries {
            let source_index = source_entities
                .entries()
                .binary_search_by(|source| source.source_patch_id().cmp(entry.source_patch_id()))
                .map_err(|_| MultiscaleEmbeddingError::PatchIdentityMapMismatch)?;
            source_rows_by_expected_patch
                .push(source_entities.entries()[source_index].source_entity_row());
        }
        entries.sort_unstable_by(|left, right| left.source_patch_id.cmp(&right.source_patch_id));

        let source_entities_logical_digest = source_entities.logical_digest();
        let expected_patches_logical_digest = expected_patches.logical_digest();
        let logical_digest = identity_digest(
            source_entities_artifact_id,
            source_entities_logical_digest,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            &entries,
        )?;
        let wire = WireRef {
            format: FORMAT,
            version: VERSION,
            source_entities_artifact_id,
            source_entities_logical_digest,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
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
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            entries: entries.into_boxed_slice(),
            source_rows_by_expected_patch: source_rows_by_expected_patch.into_boxed_slice(),
            logical_digest,
            encoded_len,
        })
    }

    /// Decode with count/string preflight and rebind exact source and expected values.
    pub fn from_canonical_json(
        bytes: &[u8],
        source_entities: &PatchSourceEntitySet,
        expected_patches: &ExpectedPatchSet,
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
        {
            return Err(MultiscaleEmbeddingError::PatchIdentityMapMismatch);
        }
        let map = Self::new(
            source_entities,
            parsed.source_entities_artifact_id,
            expected_patches,
            parsed.expected_patches_artifact_id,
            parsed.entries,
            maximum_retained_bytes,
        )?;
        if !matches_canonical_json(&map.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(map)
    }

    /// Encode the exact version-one canonical JSON with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_SOURCE_RECORD_BYTES)
    }

    /// Bound source-entity artifact identity.
    pub fn source_entities_artifact_id(&self) -> ArtifactId {
        self.source_entities_artifact_id
    }

    /// Bound source-entity logical identity.
    pub fn source_entities_logical_digest(&self) -> ContentDigest {
        self.source_entities_logical_digest
    }

    /// Bound expected-patch artifact identity.
    pub fn expected_patches_artifact_id(&self) -> ArtifactId {
        self.expected_patches_artifact_id
    }

    /// Bound expected-patch logical identity.
    pub fn expected_patches_logical_digest(&self) -> ContentDigest {
        self.expected_patches_logical_digest
    }

    /// Canonical source-key-ordered entries.
    pub fn entries(&self) -> &[PatchIdentityMapEntry] {
        &self.entries
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    /// Sorted distinct record dependency set.
    pub fn direct_dependencies(&self) -> [ArtifactId; 2] {
        let mut dependencies = [
            self.source_entities_artifact_id,
            self.expected_patches_artifact_id,
        ];
        dependencies.sort_unstable();
        dependencies
    }

    pub(super) fn source_rows_by_expected_patch(&self) -> &[u64] {
        &self.source_rows_by_expected_patch
    }

    fn wire(&self) -> WireRef<'_> {
        WireRef {
            format: FORMAT,
            version: VERSION,
            source_entities_artifact_id: self.source_entities_artifact_id,
            source_entities_logical_digest: self.source_entities_logical_digest,
            expected_patches_artifact_id: self.expected_patches_artifact_id,
            expected_patches_logical_digest: self.expected_patches_logical_digest,
            entries: EntrySlice(&self.entries),
        }
    }
}

fn retained_bytes(
    entry_capacity: usize,
    source_bytes: usize,
    patch_bytes: usize,
    row_count: usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<PatchIdentityMap>()
        .checked_add(checked_slots::<PatchIdentityMapEntry>(entry_capacity)?)
        .and_then(|value| value.checked_add(source_bytes))
        .and_then(|value| value.checked_add(patch_bytes))
        .and_then(|value| value.checked_add(checked_slots::<u64>(row_count).ok()?))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn identity_digest(
    source_entities_artifact_id: ArtifactId,
    source_entities_logical_digest: ContentDigest,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    entries: &[PatchIdentityMapEntry],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(DOMAIN);
    digest.text(FORMAT);
    digest.u32(VERSION);
    digest.artifact_id(source_entities_artifact_id);
    digest.content_digest(source_entities_logical_digest);
    digest.artifact_id(expected_patches_artifact_id);
    digest.content_digest(expected_patches_logical_digest);
    digest.array_len(entries.len())?;
    for entry in entries {
        digest.text(&entry.source_patch_id);
        digest.text(entry.patch_id.as_str());
    }
    Ok(digest.finish())
}
