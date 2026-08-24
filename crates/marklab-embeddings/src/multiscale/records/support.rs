use std::{fmt, io::Read, mem::size_of};

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

mod wire;
use wire::{parse, Parsed, SupportWireRef};

use super::{
    binding::MultiscaleArtifactBinding,
    codec::{
        preflight_json_strings, require_decoded, require_retained, MAX_RAW_SMALL_JSON_STRING_BYTES,
        MAX_SMALL_RECORD_BYTES,
    },
};
use crate::multiscale::{
    digest::LogicalDigest,
    entity::EmbeddingEntityKind,
    error::MultiscaleEmbeddingError,
    json::{
        canonical_json_len, compare_canonical_json_reader, encode_canonical_json,
        matches_canonical_json, CanonicalJsonReaderError,
    },
};

const FORMAT: &str = "marklab.multiscale_embedding_support";
const VERSION: u32 = 1;
const DOMAIN: &[u8] = b"marklab-multiscale-embedding-support-logical-v1";

/// Closed lineage/support evidence variants admitted by C-05 version one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiscaleEmbeddingSupportVariant {
    /// Exact patch context, footprints, and overlap graph.
    Patch,
    /// Region support declared through patch support plus a patch-region link.
    RegionFromPatches,
    /// Slide support inherited from patch support and a source patch table.
    SlideFromPatches,
    /// Slide support inherited from region support and a source region table.
    SlideFromRegions,
}

impl MultiscaleEmbeddingSupportVariant {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::RegionFromPatches => "region_from_patches",
            Self::SlideFromPatches => "slide_from_patches",
            Self::SlideFromRegions => "slide_from_regions",
        }
    }

    fn entity_kind(self) -> EmbeddingEntityKind {
        match self {
            Self::Patch => EmbeddingEntityKind::Patch,
            Self::RegionFromPatches => EmbeddingEntityKind::Region,
            Self::SlideFromPatches | Self::SlideFromRegions => EmbeddingEntityKind::Slide,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
enum SupportEvidence {
    Patch([MultiscaleArtifactBinding; 3]),
    RegionFromPatches([MultiscaleArtifactBinding; 2]),
    SlideFromPatches([MultiscaleArtifactBinding; 2]),
    SlideFromRegions([MultiscaleArtifactBinding; 2]),
}

impl SupportEvidence {
    fn variant(&self) -> MultiscaleEmbeddingSupportVariant {
        match self {
            Self::Patch(_) => MultiscaleEmbeddingSupportVariant::Patch,
            Self::RegionFromPatches(_) => MultiscaleEmbeddingSupportVariant::RegionFromPatches,
            Self::SlideFromPatches(_) => MultiscaleEmbeddingSupportVariant::SlideFromPatches,
            Self::SlideFromRegions(_) => MultiscaleEmbeddingSupportVariant::SlideFromRegions,
        }
    }

    fn bindings(&self) -> &[MultiscaleArtifactBinding] {
        match self {
            Self::Patch(bindings) => bindings,
            Self::RegionFromPatches(bindings)
            | Self::SlideFromPatches(bindings)
            | Self::SlideFromRegions(bindings) => bindings,
        }
    }
}

/// Strict canonical lineage/support descriptor for one patch, region, or slide embedding table.
#[derive(Clone, Eq, PartialEq)]
pub struct MultiscaleEmbeddingSupport {
    owning_slide_id: SlideId,
    evidence: SupportEvidence,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

pub(in crate::multiscale) struct PatchSupportBindings {
    pub(in crate::multiscale) context: MultiscaleArtifactBinding,
    pub(in crate::multiscale) footprints: MultiscaleArtifactBinding,
    pub(in crate::multiscale) overlap: MultiscaleArtifactBinding,
}

#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct RegionFromPatchesSupportBindings {
    pub(in crate::multiscale) patch_support: MultiscaleArtifactBinding,
    pub(in crate::multiscale) patch_region_link: MultiscaleArtifactBinding,
}

#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct SlideFromPatchesSupportBindings {
    pub(in crate::multiscale) patch_support: MultiscaleArtifactBinding,
    pub(in crate::multiscale) source_patch_table: MultiscaleArtifactBinding,
}

#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct SlideFromRegionsSupportBindings {
    pub(in crate::multiscale) region_support: MultiscaleArtifactBinding,
    pub(in crate::multiscale) source_region_table: MultiscaleArtifactBinding,
}

impl fmt::Debug for MultiscaleEmbeddingSupport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiscaleEmbeddingSupport")
            .field("variant", &self.variant())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl MultiscaleEmbeddingSupport {
    /// Bind patch context, footprints, and positive-area overlap evidence.
    pub fn patch(
        owning_slide_id: SlideId,
        patch_context: MultiscaleArtifactBinding,
        patch_footprints: MultiscaleArtifactBinding,
        patch_overlap_graph: MultiscaleArtifactBinding,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        Self::new(
            owning_slide_id,
            SupportEvidence::Patch([patch_context, patch_footprints, patch_overlap_graph]),
            maximum_retained_bytes,
        )
    }

    /// Bind region support to exact patch support and one declared patch-region link.
    pub fn region_from_patches(
        owning_slide_id: SlideId,
        patch_support: MultiscaleArtifactBinding,
        patch_region_link: MultiscaleArtifactBinding,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        Self::new(
            owning_slide_id,
            SupportEvidence::RegionFromPatches([patch_support, patch_region_link]),
            maximum_retained_bytes,
        )
    }

    /// Bind slide support to exact patch support and its source patch table.
    pub fn slide_from_patches(
        owning_slide_id: SlideId,
        patch_support: MultiscaleArtifactBinding,
        source_patch_table: MultiscaleArtifactBinding,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        Self::new(
            owning_slide_id,
            SupportEvidence::SlideFromPatches([patch_support, source_patch_table]),
            maximum_retained_bytes,
        )
    }

    /// Bind slide support to exact region support and its source region table.
    pub fn slide_from_regions(
        owning_slide_id: SlideId,
        region_support: MultiscaleArtifactBinding,
        source_region_table: MultiscaleArtifactBinding,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        Self::new(
            owning_slide_id,
            SupportEvidence::SlideFromRegions([region_support, source_region_table]),
            maximum_retained_bytes,
        )
    }

    /// Decode and revalidate one exact canonical support fixed point under caller budgets.
    pub fn from_canonical_json(
        bytes: &[u8],
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
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
        if parsed.format != FORMAT || parsed.version != VERSION {
            return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingSupport);
        }
        let expected_kind = parsed.evidence.variant().entity_kind().wire_name();
        if parsed.variant != parsed.evidence.variant().wire_name()
            || parsed.entity_kind != expected_kind
        {
            return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingSupport);
        }
        require_retained(
            retained_bytes(parsed.owning_slide_id.len())?,
            maximum_retained_bytes,
        )?;
        let owning_slide_id = SlideId::new(parsed.owning_slide_id)
            .map_err(|_| MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingSupport)?;
        let support = Self::new(owning_slide_id, parsed.evidence, maximum_retained_bytes)?;
        if !matches_canonical_json(&support.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(support)
    }

    /// Encode the exact canonical JSON document with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_SMALL_RECORD_BYTES)
    }

    pub(in crate::multiscale) fn compare_canonical_json_reader<R: Read + ?Sized>(
        &self,
        reader: &mut R,
    ) -> Result<(), CanonicalJsonReaderError> {
        compare_canonical_json_reader(
            &self.wire(),
            self.encoded_len,
            MAX_SMALL_RECORD_BYTES,
            reader,
        )
    }

    /// Closed support variant.
    pub fn variant(&self) -> MultiscaleEmbeddingSupportVariant {
        self.evidence.variant()
    }

    /// Entity kind implied by the closed variant.
    pub fn entity_kind(&self) -> EmbeddingEntityKind {
        self.variant().entity_kind()
    }

    /// Owning slide shared by all bound support evidence.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.owning_slide_id
    }

    /// Sorted distinct artifact dependency IDs for the C-03 record boundary.
    pub fn direct_dependencies(&self) -> impl ExactSizeIterator<Item = ArtifactId> {
        let bindings = self.evidence.bindings();
        let mut dependencies = [bindings[0].artifact_id(); 3];
        for (target, binding) in dependencies.iter_mut().zip(bindings) {
            *target = binding.artifact_id();
        }
        dependencies[..bindings.len()].sort_unstable();
        dependencies.into_iter().take(bindings.len())
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    pub(in crate::multiscale) fn patch_bindings(&self) -> Option<PatchSupportBindings> {
        let SupportEvidence::Patch([context, footprints, overlap]) = self.evidence else {
            return None;
        };
        Some(PatchSupportBindings {
            context,
            footprints,
            overlap,
        })
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn region_from_patches_bindings(
        &self,
    ) -> Option<RegionFromPatchesSupportBindings> {
        let SupportEvidence::RegionFromPatches([patch_support, patch_region_link]) = self.evidence
        else {
            return None;
        };
        Some(RegionFromPatchesSupportBindings {
            patch_support,
            patch_region_link,
        })
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn slide_from_patches_bindings(
        &self,
    ) -> Option<SlideFromPatchesSupportBindings> {
        let SupportEvidence::SlideFromPatches([patch_support, source_patch_table]) = self.evidence
        else {
            return None;
        };
        Some(SlideFromPatchesSupportBindings {
            patch_support,
            source_patch_table,
        })
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn slide_from_regions_bindings(
        &self,
    ) -> Option<SlideFromRegionsSupportBindings> {
        let SupportEvidence::SlideFromRegions([region_support, source_region_table]) =
            self.evidence
        else {
            return None;
        };
        Some(SlideFromRegionsSupportBindings {
            region_support,
            source_region_table,
        })
    }

    fn new(
        owning_slide_id: SlideId,
        evidence: SupportEvidence,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_distinct(evidence.bindings())?;
        require_retained(
            retained_bytes(owning_slide_id.as_str().len())?,
            maximum_retained_bytes,
        )?;
        let logical_digest = support_digest(&owning_slide_id, &evidence);
        let mut support = Self {
            owning_slide_id,
            evidence,
            logical_digest,
            encoded_len: 0,
        };
        let encoded_len = canonical_json_len(&support.wire())?;
        if encoded_len > MAX_SMALL_RECORD_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_SMALL_RECORD_BYTES,
            });
        }
        support.encoded_len = encoded_len;
        Ok(support)
    }

    fn wire(&self) -> SupportWireRef<'_> {
        SupportWireRef::new(&self.owning_slide_id, &self.evidence)
    }
}

fn retained_bytes(slide_text_bytes: usize) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<MultiscaleEmbeddingSupport>()
        .checked_add(slide_text_bytes)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn validate_distinct(
    bindings: &[MultiscaleArtifactBinding],
) -> Result<(), MultiscaleEmbeddingError> {
    let mut dependencies = [bindings[0].artifact_id(); 3];
    for (target, binding) in dependencies.iter_mut().zip(bindings) {
        *target = binding.artifact_id();
    }
    let active = &mut dependencies[..bindings.len()];
    active.sort_unstable();
    if active.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(MultiscaleEmbeddingError::DuplicateMultiscaleSupportArtifactDependency);
    }
    Ok(())
}

fn support_digest(owning_slide_id: &SlideId, evidence: &SupportEvidence) -> ContentDigest {
    let variant = evidence.variant();
    let mut digest = LogicalDigest::new(DOMAIN);
    digest.text(FORMAT);
    digest.u32(VERSION);
    digest.text(variant.wire_name());
    digest.text(variant.entity_kind().wire_name());
    digest.text(owning_slide_id.as_str());
    for binding in evidence.bindings() {
        digest.artifact_id(binding.artifact_id());
        digest.content_digest(binding.logical_digest());
    }
    digest.finish()
}
