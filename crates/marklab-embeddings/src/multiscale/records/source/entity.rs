use std::{fmt, io::Read, mem::size_of};

mod wire;
use wire::{parse_collect, parse_preflight, EntrySlice, WireRef};

use super::super::codec::{
    checked_slots, preflight_json_strings, require_decoded, require_retained, valid_source_key,
    valid_token, validate_source_row_count, MAX_RAW_SOURCE_JSON_STRING_BYTES,
    MAX_SOURCE_RECORD_BYTES,
};
use crate::multiscale::{
    digest::LogicalDigest,
    error::MultiscaleEmbeddingError,
    json::{
        canonical_json_len, compare_canonical_json_reader, encode_canonical_json,
        matches_canonical_json, CanonicalJsonReaderError,
    },
};
use marklab_project::ContentDigest;

const FORMAT: &str = "marklab.patch_source_entity_set";
const VERSION: u32 = 1;
const DOMAIN: &[u8] = b"marklab-patch-source-entity-set-logical-v1";

/// One private source-local patch key and its explicit source-entity row.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchSourceEntityEntry {
    source_patch_id: Box<str>,
    source_entity_row: u64,
}

impl fmt::Debug for PatchSourceEntityEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchSourceEntityEntry")
            .finish_non_exhaustive()
    }
}

impl PatchSourceEntityEntry {
    /// Validate one bounded, unpadded, control-free source-local key.
    pub fn new(
        source_patch_id: impl Into<String>,
        source_entity_row: u64,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let source_patch_id = source_patch_id.into();
        if !valid_source_key(&source_patch_id) {
            return Err(MultiscaleEmbeddingError::InvalidPatchSourceKey);
        }
        Ok(Self {
            source_patch_id: source_patch_id.into_boxed_str(),
            source_entity_row,
        })
    }

    /// Exact private source-local patch key.
    pub fn source_patch_id(&self) -> &str {
        &self.source_patch_id
    }

    /// Explicit zero-based source-entity row.
    pub fn source_entity_row(&self) -> u64 {
        self.source_entity_row
    }
}

/// Exact source-local patch-key universe for one declared source profile.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchSourceEntitySet {
    source_profile: Box<str>,
    entries: Box<[PatchSourceEntityEntry]>,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

impl fmt::Debug for PatchSourceEntitySet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchSourceEntitySet")
            .field("row_count", &self.entries.len())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchSourceEntitySet {
    /// Validate profile, bytewise source-key order, gap-free row permutation, and retained bytes.
    pub fn new(
        source_profile: impl Into<String>,
        mut entries: Vec<PatchSourceEntityEntry>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let source_profile = source_profile.into();
        if !valid_token(&source_profile) {
            return Err(MultiscaleEmbeddingError::InvalidPatchSourceProfile);
        }
        validate_source_row_count(entries.len())?;
        if entries
            .windows(2)
            .any(|pair| pair[0].source_patch_id >= pair[1].source_patch_id)
        {
            return Err(MultiscaleEmbeddingError::NonCanonicalPatchSourceEntities);
        }
        let source_bytes = source_text_bytes(&entries)?;
        let required = retained_bytes(source_profile.capacity(), entries.capacity(), source_bytes)?;
        require_retained(required, maximum_retained_bytes)?;
        entries.sort_unstable_by_key(|entry| entry.source_entity_row);
        if entries
            .iter()
            .enumerate()
            .any(|(expected, entry)| u64::try_from(expected).ok() != Some(entry.source_entity_row))
        {
            return Err(MultiscaleEmbeddingError::NonCanonicalPatchSourceEntities);
        }
        entries.sort_unstable_by(|left, right| left.source_patch_id.cmp(&right.source_patch_id));
        let logical_digest = source_digest(&source_profile, &entries)?;
        let wire = WireRef {
            format: FORMAT,
            version: VERSION,
            source_profile: &source_profile,
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
            source_profile: source_profile.into_boxed_str(),
            entries: entries.into_boxed_slice(),
            logical_digest,
            encoded_len,
        })
    }

    /// Decode with a count/string pass before exact-capacity semantic construction.
    pub fn from_canonical_json(
        bytes: &[u8],
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
        let source = Self::new(
            parsed.source_profile,
            parsed.entries,
            maximum_retained_bytes,
        )?;
        if !matches_canonical_json(&source.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(source)
    }

    /// Encode the exact version-one canonical JSON with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_SOURCE_RECORD_BYTES)
    }

    pub(in crate::multiscale) fn compare_canonical_json_reader<R: Read + ?Sized>(
        &self,
        reader: &mut R,
    ) -> Result<(), CanonicalJsonReaderError> {
        compare_canonical_json_reader(
            &self.wire(),
            self.encoded_len,
            MAX_SOURCE_RECORD_BYTES,
            reader,
        )
    }

    /// Explicit bounded source-profile token.
    pub fn source_profile(&self) -> &str {
        &self.source_profile
    }

    /// Canonical entries in strict source-key byte order.
    pub fn entries(&self) -> &[PatchSourceEntityEntry] {
        &self.entries
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    fn wire(&self) -> WireRef<'_> {
        WireRef {
            format: FORMAT,
            version: VERSION,
            source_profile: &self.source_profile,
            entries: EntrySlice(&self.entries),
        }
    }
}

fn source_text_bytes(
    entries: &[PatchSourceEntityEntry],
) -> Result<usize, MultiscaleEmbeddingError> {
    entries.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.source_patch_id.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    })
}

fn retained_bytes(
    profile_bytes: usize,
    entry_capacity: usize,
    source_bytes: usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<PatchSourceEntitySet>()
        .checked_add(profile_bytes)
        .and_then(|value| {
            value.checked_add(checked_slots::<PatchSourceEntityEntry>(entry_capacity).ok()?)
        })
        .and_then(|value| value.checked_add(source_bytes))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn source_digest(
    source_profile: &str,
    entries: &[PatchSourceEntityEntry],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(DOMAIN);
    digest.text(FORMAT);
    digest.u32(VERSION);
    digest.text(source_profile);
    digest.array_len(entries.len())?;
    for entry in entries {
        digest.text(&entry.source_patch_id);
        digest.u64(entry.source_entity_row);
    }
    Ok(digest.finish())
}
