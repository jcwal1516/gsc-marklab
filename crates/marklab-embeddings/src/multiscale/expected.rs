use std::{fmt, mem::size_of};

use super::{
    digest::LogicalDigest,
    entity::{EmbeddingEntityKind, EntitySpec},
    error::MultiscaleEmbeddingError,
    json::{canonical_json_len, encode_canonical_json, matches_canonical_json},
};
use marklab_data::{CohortHierarchy, HierarchyKind, PatchId, RegionId, SlideId};
use marklab_project::ContentDigest;

mod wire;
use wire::{
    parse_expected, parse_preflight, preflight_json_string_lengths, ExpectedWireRef, IdSlice,
};

const EXPECTED_VERSION: u32 = 1;
const MAX_EXPECTED_BYTES: usize = 1024 * 1024 * 1024;
const MAX_EXPECTED_ROWS: usize = 100_000_000;
const MAX_RAW_JSON_STRING_BYTES: usize = 6 * 255;

#[derive(Clone, Eq, PartialEq)]
pub(super) struct ExpectedEntitySet<I> {
    owning_slide_id: SlideId,
    selection_rule: Box<str>,
    ids: Box<[I]>,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

impl<I> fmt::Debug for ExpectedEntitySet<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExpectedEntitySet")
            .field("row_count", &self.ids.len())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl<I: EntitySpec> ExpectedEntitySet<I> {
    fn new(
        hierarchy: &CohortHierarchy,
        owning_slide_id: SlideId,
        selection_rule: String,
        ids: Vec<I>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if !valid_token(&selection_rule) {
            return Err(MultiscaleEmbeddingError::InvalidSelectionRule);
        }
        validate_count(ids.len())?;
        if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(MultiscaleEmbeddingError::NonCanonicalExpectedOrder);
        }
        validate_cardinality::<I>(&owning_slide_id, &ids)?;
        validate_hierarchy(hierarchy, &owning_slide_id, &ids)?;
        let wire = ExpectedWireRef::<I> {
            format: I::EXPECTED_FORMAT,
            version: EXPECTED_VERSION,
            owning_slide_id: owning_slide_id.as_str(),
            selection_rule: &selection_rule,
            ids: IdSlice(&ids),
        };
        let encoded_len = canonical_json_len(&wire)?;
        if encoded_len > MAX_EXPECTED_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_EXPECTED_BYTES,
            });
        }
        let required = retained_bytes::<I>(
            &owning_slide_id,
            &selection_rule,
            ids.capacity(),
            ids.iter().map(EntitySpec::as_str),
        )?;
        require_retained_budget(required, maximum_retained_bytes)?;
        let logical_digest = expected_digest::<I>(&owning_slide_id, &selection_rule, &ids)?;
        Ok(Self {
            owning_slide_id,
            selection_rule: selection_rule.into_boxed_str(),
            ids: ids.into_boxed_slice(),
            logical_digest,
            encoded_len,
        })
    }

    fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_EXPECTED_BYTES)
    }

    fn from_canonical_json(
        bytes: &[u8],
        hierarchy: &CohortHierarchy,
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let effective_maximum = maximum_encoded_bytes.min(MAX_EXPECTED_BYTES);
        if bytes.len() > effective_maximum {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: effective_maximum,
            });
        }
        preflight_json_string_lengths(bytes)?;
        let preflight = parse_preflight::<I>(bytes)?;
        validate_count(preflight.id_count)?;
        let decoded_required = preflight.decoded_bytes::<I>()?;
        if decoded_required > maximum_decoded_bytes {
            return Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded {
                required: decoded_required,
                maximum: maximum_decoded_bytes,
            });
        }
        let required = preflight.retained_bytes::<I>()?;
        require_retained_budget(required, maximum_retained_bytes)?;
        let parsed = parse_expected::<I>(bytes, preflight.id_count)?;
        let expected = Self::new(
            hierarchy,
            SlideId::new(parsed.owning_slide_id)
                .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?,
            parsed.selection_rule,
            parsed.ids,
            maximum_retained_bytes,
        )?;
        if !matches_canonical_json(&expected.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(expected)
    }

    fn wire(&self) -> ExpectedWireRef<'_, I> {
        ExpectedWireRef::<I> {
            format: I::EXPECTED_FORMAT,
            version: EXPECTED_VERSION,
            owning_slide_id: self.owning_slide_id.as_str(),
            selection_rule: &self.selection_rule,
            ids: IdSlice(&self.ids),
        }
    }
}

fn validate_count(count: usize) -> Result<(), MultiscaleEmbeddingError> {
    if count > MAX_EXPECTED_ROWS {
        return Err(MultiscaleEmbeddingError::RowCountExceeded {
            observed: count,
            maximum: MAX_EXPECTED_ROWS,
        });
    }
    Ok(())
}

fn retained_bytes<'a, I: EntitySpec + 'a>(
    owning_slide_id: &SlideId,
    selection_rule: &str,
    id_capacity: usize,
    mut id_texts: impl Iterator<Item = &'a str>,
) -> Result<usize, MultiscaleEmbeddingError> {
    let id_slots = id_capacity
        .checked_mul(size_of::<I>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let id_text = id_texts.try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    })?;
    size_of::<ExpectedEntitySet<I>>()
        .checked_add(owning_slide_id.as_str().len())
        .and_then(|value| value.checked_add(selection_rule.len()))
        .and_then(|value| value.checked_add(id_slots))
        .and_then(|value| value.checked_add(id_text))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn require_retained_budget(
    required: usize,
    maximum: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum });
    }
    Ok(())
}

fn validate_cardinality<I: EntitySpec>(
    owning_slide_id: &SlideId,
    ids: &[I],
) -> Result<(), MultiscaleEmbeddingError> {
    if I::KIND == EmbeddingEntityKind::Slide
        && (ids.len() != 1 || ids[0].as_str() != owning_slide_id.as_str())
    {
        return Err(MultiscaleEmbeddingError::InvalidExpectedSetCardinality);
    }
    Ok(())
}

fn validate_hierarchy<I: EntitySpec>(
    hierarchy: &CohortHierarchy,
    owning_slide_id: &SlideId,
    ids: &[I],
) -> Result<(), MultiscaleEmbeddingError> {
    let owning = marklab_data::HierarchyId::from(owning_slide_id.clone());
    if !hierarchy.contains(&owning) {
        return Err(MultiscaleEmbeddingError::HierarchyOwnershipMismatch);
    }
    for id in ids {
        let mut current = id.hierarchy_id();
        if !hierarchy.contains(&current) {
            return Err(MultiscaleEmbeddingError::HierarchyOwnershipMismatch);
        }
        loop {
            if current.kind() == HierarchyKind::Slide {
                if current != owning {
                    return Err(MultiscaleEmbeddingError::HierarchyOwnershipMismatch);
                }
                break;
            }
            current = hierarchy
                .parent(&current)
                .cloned()
                .ok_or(MultiscaleEmbeddingError::HierarchyOwnershipMismatch)?;
        }
    }
    Ok(())
}

fn expected_digest<I: EntitySpec>(
    owning_slide_id: &SlideId,
    selection_rule: &str,
    ids: &[I],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(I::EXPECTED_DOMAIN);
    digest.text(I::EXPECTED_FORMAT);
    digest.u32(EXPECTED_VERSION);
    digest.text(owning_slide_id.as_str());
    digest.text(selection_rule);
    digest.array_len(ids.len())?;
    for id in ids {
        digest.text(id.as_str());
    }
    Ok(digest.finish())
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b':' | b'+' | b'-' => index != 0,
            _ => false,
        })
}

macro_rules! define_expected_set {
    ($name:ident, $id:ty, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(ExpectedEntitySet<$id>);

        impl $name {
            /// Validate one owning slide, bounded selection rule, canonical IDs, hierarchy, and budget.
            pub fn new(
                hierarchy: &CohortHierarchy,
                owning_slide_id: SlideId,
                selection_rule: impl Into<String>,
                ids: Vec<$id>,
                maximum_retained_bytes: usize,
            ) -> Result<Self, MultiscaleEmbeddingError> {
                ExpectedEntitySet::new(
                    hierarchy,
                    owning_slide_id,
                    selection_rule.into(),
                    ids,
                    maximum_retained_bytes,
                )
                .map(Self)
            }

            /// Decode the exact canonical JSON after encoded and retained preflight.
            pub fn from_canonical_json(
                bytes: &[u8],
                hierarchy: &CohortHierarchy,
                maximum_encoded_bytes: usize,
                maximum_decoded_bytes: usize,
                maximum_retained_bytes: usize,
            ) -> Result<Self, MultiscaleEmbeddingError> {
                ExpectedEntitySet::from_canonical_json(
                    bytes,
                    hierarchy,
                    maximum_encoded_bytes,
                    maximum_decoded_bytes,
                    maximum_retained_bytes,
                )
                .map(Self)
            }

            /// Encode the exact version-one canonical JSON with one final newline.
            pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
                self.0.to_canonical_json()
            }

            /// Owning slide for every expected entity.
            pub fn owning_slide_id(&self) -> &SlideId {
                &self.0.owning_slide_id
            }

            /// Explicit bounded selection-rule token.
            pub fn selection_rule(&self) -> &str {
                &self.0.selection_rule
            }

            /// Strictly increasing typed expected IDs.
            pub fn ids(&self) -> &[$id] {
                &self.0.ids
            }

            /// Format-independent domain-separated logical identity.
            pub fn logical_digest(&self) -> ContentDigest {
                self.0.logical_digest
            }
        }
    };
}

define_expected_set!(
    ExpectedPatchSet,
    PatchId,
    "Canonical expected patch identities for one owning slide."
);
define_expected_set!(
    ExpectedRegionSet,
    RegionId,
    "Canonical expected region identities for one owning slide."
);
define_expected_set!(
    ExpectedSlideSet,
    SlideId,
    "Canonical singleton expected-slide identity."
);

#[cfg(test)]
mod tests {
    use super::preflight_json_string_lengths;

    #[test]
    fn raw_string_preflight_handles_escapes_and_unterminated_values() {
        assert!(preflight_json_string_lengths(br#"{"a":"b\\\"c"}"#).is_ok());
        assert!(preflight_json_string_lengths(br#"{"a":"unterminated}"#).is_err());
    }
}
