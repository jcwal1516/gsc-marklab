use std::{fmt, io::Read, mem::size_of};

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

mod evidence;
mod types;
mod wire;

use evidence::{
    provenance_digest, DerivedRegionEvidence, DerivedSlideEvidence, DirectPatchEvidence,
    ProvenanceEvidence,
};
pub use types::{
    MultiscaleDirectPatchInputArtifacts, MultiscaleDirectPatchModelProvenance,
    MultiscaleEmbeddingExecutionProvenance,
};
use wire::{parse, ProvenanceWireRef};

use super::{
    codec::{
        preflight_json_strings_with_scratch, require_decoded, require_retained, valid_token,
        MAX_SMALL_RECORD_BYTES,
    },
    derivation::MultiscaleEmbeddingDerivationContract,
    support::{MultiscaleEmbeddingSupport, MultiscaleEmbeddingSupportVariant},
};
use crate::multiscale::{
    entity::EmbeddingEntityKind,
    error::MultiscaleEmbeddingError,
    json::{
        canonical_json_len, compare_canonical_json_reader, encode_canonical_json,
        matches_canonical_json, CanonicalJsonReaderError,
    },
};

const FORMAT: &str = "marklab.multiscale_embedding_provenance";
const VERSION: u32 = 1;
const MAX_DIMENSION: u32 = 65_536;
const MAX_RAW_PROVENANCE_JSON_STRING_BYTES: usize = 6 * 4_096;

/// Closed version-one provenance variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiscaleEmbeddingProvenanceVariant {
    /// Direct model extraction into expected patch rows.
    DirectPatch,
    /// Deterministic weighted region aggregation from a patch table.
    DerivedRegion,
    /// Deterministic arithmetic slide aggregation from a patch table.
    DerivedSlideFromPatches,
    /// Deterministic arithmetic slide aggregation from a region table.
    DerivedSlideFromRegions,
}

impl MultiscaleEmbeddingProvenanceVariant {
    fn wire_name(self) -> &'static str {
        match self {
            Self::DirectPatch => "direct_patch",
            Self::DerivedRegion => "derived_region",
            Self::DerivedSlideFromPatches => "derived_slide_from_patches",
            Self::DerivedSlideFromRegions => "derived_slide_from_regions",
        }
    }

    fn entity_kind(self) -> EmbeddingEntityKind {
        match self {
            Self::DirectPatch => EmbeddingEntityKind::Patch,
            Self::DerivedRegion => EmbeddingEntityKind::Region,
            Self::DerivedSlideFromPatches | Self::DerivedSlideFromRegions => {
                EmbeddingEntityKind::Slide
            }
        }
    }
}

/// Strict complete version-one multiscale embedding provenance value.
#[derive(Clone, Eq, PartialEq)]
pub struct MultiscaleEmbeddingProvenance {
    owning_slide_id: SlideId,
    output_dimension: u32,
    evidence: ProvenanceEvidence,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

pub(in crate::multiscale) struct DirectPatchArtifactRoles {
    pub(in crate::multiscale) checkpoint: ArtifactId,
    pub(in crate::multiscale) checkpoint_content_digest: ContentDigest,
    pub(in crate::multiscale) source_snapshot: ArtifactId,
    pub(in crate::multiscale) license_record: ArtifactId,
    pub(in crate::multiscale) input_normalization: ArtifactId,
    pub(in crate::multiscale) preprocessing: ArtifactId,
    pub(in crate::multiscale) run_config: ArtifactId,
    pub(in crate::multiscale) environment: ArtifactId,
    pub(in crate::multiscale) converter: ArtifactId,
    pub(in crate::multiscale) source_entities: ArtifactId,
    pub(in crate::multiscale) source_vectors: ArtifactId,
    pub(in crate::multiscale) expected_patches: ArtifactId,
    pub(in crate::multiscale) identity_map: ArtifactId,
    pub(in crate::multiscale) source_row_link: ArtifactId,
    pub(in crate::multiscale) patch_support: ArtifactId,
}

#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct DerivedRegionArtifactRoles {
    pub(in crate::multiscale) run_config: ArtifactId,
    pub(in crate::multiscale) environment: ArtifactId,
    pub(in crate::multiscale) converter: ArtifactId,
    pub(in crate::multiscale) source_patch_table: ArtifactId,
    pub(in crate::multiscale) patch_region_link: ArtifactId,
    pub(in crate::multiscale) expected_regions: ArtifactId,
    pub(in crate::multiscale) region_support: ArtifactId,
    pub(in crate::multiscale) derivation_contract: ArtifactId,
}

impl fmt::Debug for MultiscaleEmbeddingProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiscaleEmbeddingProvenance")
            .field("variant", &self.variant())
            .field("output_dimension", &self.output_dimension)
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl MultiscaleEmbeddingProvenance {
    /// Construct complete direct patch extraction provenance.
    #[allow(clippy::too_many_arguments)]
    pub fn direct_patch(
        owning_slide_id: SlideId,
        patch_support: &MultiscaleEmbeddingSupport,
        output_dimension: u32,
        pooling_or_aggregation: impl Into<String>,
        model: MultiscaleDirectPatchModelProvenance,
        execution: MultiscaleEmbeddingExecutionProvenance,
        preprocessing_artifact_id: ArtifactId,
        inputs: MultiscaleDirectPatchInputArtifacts,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_support(
            &owning_slide_id,
            patch_support,
            MultiscaleEmbeddingSupportVariant::Patch,
        )?;
        let pooling = pooling_or_aggregation.into();
        if !valid_token(&pooling) {
            return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingProvenance);
        }
        let model_and_pooling = pooling
            .capacity()
            .checked_add(model.heap_text_bytes()?)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let charged_text_bytes = model_and_pooling
            .checked_add(execution.heap_text_bytes()?)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        Self::finish(
            owning_slide_id,
            output_dimension,
            ProvenanceEvidence::DirectPatch(DirectPatchEvidence {
                pooling: pooling.into_boxed_str(),
                model,
                execution,
                preprocessing_artifact_id,
                inputs,
            }),
            charged_text_bytes,
            maximum_retained_bytes,
        )
    }

    /// Construct deterministic fraction-weighted region provenance.
    #[allow(clippy::too_many_arguments)]
    pub fn derived_region(
        owning_slide_id: SlideId,
        region_support: &MultiscaleEmbeddingSupport,
        derivation: &MultiscaleEmbeddingDerivationContract,
        output_dimension: u32,
        execution: MultiscaleEmbeddingExecutionProvenance,
        source_patch_table_artifact_id: ArtifactId,
        patch_region_link_artifact_id: ArtifactId,
        expected_regions_artifact_id: ArtifactId,
        region_support_artifact_id: ArtifactId,
        derivation_contract_artifact_id: ArtifactId,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_support(
            &owning_slide_id,
            region_support,
            MultiscaleEmbeddingSupportVariant::RegionFromPatches,
        )?;
        validate_derivation(derivation, "weighted_mean")?;
        let charged_text_bytes = execution.heap_text_bytes()?;
        Self::finish(
            owning_slide_id,
            output_dimension,
            ProvenanceEvidence::DerivedRegion(DerivedRegionEvidence {
                execution,
                source_patch_table_artifact_id,
                patch_region_link_artifact_id,
                expected_regions_artifact_id,
                region_support_artifact_id,
                derivation_contract_artifact_id,
            }),
            charged_text_bytes,
            maximum_retained_bytes,
        )
    }

    /// Construct deterministic arithmetic slide provenance from patches.
    #[allow(clippy::too_many_arguments)]
    pub fn derived_slide_from_patches(
        owning_slide_id: SlideId,
        slide_support: &MultiscaleEmbeddingSupport,
        derivation: &MultiscaleEmbeddingDerivationContract,
        output_dimension: u32,
        execution: MultiscaleEmbeddingExecutionProvenance,
        source_patch_table_artifact_id: ArtifactId,
        expected_slides_artifact_id: ArtifactId,
        slide_support_artifact_id: ArtifactId,
        derivation_contract_artifact_id: ArtifactId,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_support(
            &owning_slide_id,
            slide_support,
            MultiscaleEmbeddingSupportVariant::SlideFromPatches,
        )?;
        validate_derivation(derivation, "arithmetic_mean")?;
        let charged_text_bytes = execution.heap_text_bytes()?;
        Self::finish(
            owning_slide_id,
            output_dimension,
            ProvenanceEvidence::DerivedSlideFromPatches(DerivedSlideEvidence {
                execution,
                source_table_artifact_id: source_patch_table_artifact_id,
                expected_slides_artifact_id,
                slide_support_artifact_id,
                derivation_contract_artifact_id,
            }),
            charged_text_bytes,
            maximum_retained_bytes,
        )
    }

    /// Construct deterministic arithmetic slide provenance from regions.
    #[allow(clippy::too_many_arguments)]
    pub fn derived_slide_from_regions(
        owning_slide_id: SlideId,
        slide_support: &MultiscaleEmbeddingSupport,
        derivation: &MultiscaleEmbeddingDerivationContract,
        output_dimension: u32,
        execution: MultiscaleEmbeddingExecutionProvenance,
        source_region_table_artifact_id: ArtifactId,
        expected_slides_artifact_id: ArtifactId,
        slide_support_artifact_id: ArtifactId,
        derivation_contract_artifact_id: ArtifactId,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_support(
            &owning_slide_id,
            slide_support,
            MultiscaleEmbeddingSupportVariant::SlideFromRegions,
        )?;
        validate_derivation(derivation, "arithmetic_mean")?;
        let charged_text_bytes = execution.heap_text_bytes()?;
        Self::finish(
            owning_slide_id,
            output_dimension,
            ProvenanceEvidence::DerivedSlideFromRegions(DerivedSlideEvidence {
                execution,
                source_table_artifact_id: source_region_table_artifact_id,
                expected_slides_artifact_id,
                slide_support_artifact_id,
                derivation_contract_artifact_id,
            }),
            charged_text_bytes,
            maximum_retained_bytes,
        )
    }

    /// Decode and revalidate one exact canonical provenance fixed point under caller budgets.
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
        let parser_scratch_bytes =
            preflight_json_strings_with_scratch(bytes, MAX_RAW_PROVENANCE_JSON_STRING_BYTES)?;
        // One input-sized charge bounds all owned decoded strings in aggregate. Serde JSON keeps
        // the largest escaped string in a separate reusable scratch buffer until parsing ends.
        let decoded_required = size_of::<wire::Parsed>()
            .checked_add(bytes.len())
            .and_then(|required| required.checked_add(parser_scratch_bytes))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        require_decoded(decoded_required, maximum_decoded_bytes)?;
        let parsed = parse(bytes)?;
        validate_common(
            &parsed.variant,
            &parsed.entity_kind,
            &parsed.dtype,
            parsed.output_dimension,
            &parsed.pooling_or_aggregation,
        )?;
        let text_bytes = parsed.heap_text_bytes()?;
        require_retained(
            retained_bytes(parsed.owning_slide_id.len(), text_bytes)?,
            maximum_retained_bytes,
        )?;
        let owning_slide_id = SlideId::new(&parsed.owning_slide_id)
            .map_err(|_| MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingProvenance)?;
        let output_dimension = parsed.output_dimension;
        let evidence = parsed
            .evidence
            .into_evidence(parsed.pooling_or_aggregation)?;
        let provenance = Self::finish(
            owning_slide_id,
            output_dimension,
            evidence,
            text_bytes,
            maximum_retained_bytes,
        )?;
        if !matches_canonical_json(&provenance.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(provenance)
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

    /// Closed provenance variant.
    pub fn variant(&self) -> MultiscaleEmbeddingProvenanceVariant {
        self.evidence.variant()
    }

    /// Entity kind implied by the selected variant.
    pub fn entity_kind(&self) -> EmbeddingEntityKind {
        self.variant().entity_kind()
    }

    /// Owning slide.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.owning_slide_id
    }

    /// Positive output dimension, capped at 65,536.
    pub fn output_dimension(&self) -> u32 {
        self.output_dimension
    }

    /// Direct pooling token or the exact deterministic derived aggregation token.
    pub fn pooling_or_aggregation(&self) -> &str {
        self.evidence.pooling_or_aggregation()
    }

    /// Sorted distinct C-03 dependency IDs for the selected variant.
    pub fn direct_dependencies(&self) -> impl ExactSizeIterator<Item = ArtifactId> {
        let (mut dependencies, count) = self.evidence.dependencies();
        dependencies[..count].sort_unstable();
        dependencies.into_iter().take(count)
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    pub(in crate::multiscale) fn direct_patch_artifact_roles(
        &self,
    ) -> Option<DirectPatchArtifactRoles> {
        let ProvenanceEvidence::DirectPatch(evidence) = &self.evidence else {
            return None;
        };
        Some(DirectPatchArtifactRoles {
            checkpoint: evidence.model.checkpoint_artifact_id,
            checkpoint_content_digest: evidence.model.checkpoint_content_sha256,
            source_snapshot: evidence.model.source_snapshot_artifact_id,
            license_record: evidence.model.license_record_artifact_id,
            input_normalization: evidence.inputs.input_normalization_artifact_id,
            preprocessing: evidence.preprocessing_artifact_id,
            run_config: evidence.execution.run_config_artifact_id,
            environment: evidence.execution.environment_artifact_id,
            converter: evidence.execution.converter_artifact_id,
            source_entities: evidence.inputs.source_entities_artifact_id,
            source_vectors: evidence.inputs.source_vectors_artifact_id,
            expected_patches: evidence.inputs.expected_patches_artifact_id,
            identity_map: evidence.inputs.identity_map_artifact_id,
            source_row_link: evidence.inputs.source_row_link_artifact_id,
            patch_support: evidence.inputs.patch_support_artifact_id,
        })
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn derived_region_artifact_roles(
        &self,
    ) -> Option<DerivedRegionArtifactRoles> {
        let ProvenanceEvidence::DerivedRegion(evidence) = &self.evidence else {
            return None;
        };
        Some(DerivedRegionArtifactRoles {
            run_config: evidence.execution.run_config_artifact_id,
            environment: evidence.execution.environment_artifact_id,
            converter: evidence.execution.converter_artifact_id,
            source_patch_table: evidence.source_patch_table_artifact_id,
            patch_region_link: evidence.patch_region_link_artifact_id,
            expected_regions: evidence.expected_regions_artifact_id,
            region_support: evidence.region_support_artifact_id,
            derivation_contract: evidence.derivation_contract_artifact_id,
        })
    }

    fn finish(
        owning_slide_id: SlideId,
        output_dimension: u32,
        evidence: ProvenanceEvidence,
        charged_text_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_dimension(output_dimension)?;
        let actual_text_bytes = evidence.heap_text_bytes()?;
        if actual_text_bytes > charged_text_bytes {
            return Err(MultiscaleEmbeddingError::SizeOverflow);
        }
        require_retained(
            retained_bytes(owning_slide_id.as_str().len(), charged_text_bytes)?,
            maximum_retained_bytes,
        )?;
        validate_dependencies(&evidence)?;
        let logical_digest = provenance_digest(&owning_slide_id, output_dimension, &evidence);
        let mut provenance = Self {
            owning_slide_id,
            output_dimension,
            evidence,
            logical_digest,
            encoded_len: 0,
        };
        let encoded_len = canonical_json_len(&provenance.wire())?;
        if encoded_len > MAX_SMALL_RECORD_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_SMALL_RECORD_BYTES,
            });
        }
        provenance.encoded_len = encoded_len;
        Ok(provenance)
    }

    fn wire(&self) -> ProvenanceWireRef<'_> {
        ProvenanceWireRef::new(self)
    }
}

fn validate_common(
    variant: &str,
    entity_kind: &str,
    dtype: &str,
    output_dimension: u32,
    pooling: &str,
) -> Result<(), MultiscaleEmbeddingError> {
    let expected = match variant {
        "direct_patch" => ("patch", None),
        "derived_region" => ("region", Some("weighted_mean")),
        "derived_slide_from_patches" | "derived_slide_from_regions" => {
            ("slide", Some("arithmetic_mean"))
        }
        _ => return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingProvenance),
    };
    if entity_kind != expected.0
        || dtype != "f32"
        || expected.1.is_some_and(|value| pooling != value)
        || expected.1.is_none() && !valid_token(pooling)
    {
        return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingProvenance);
    }
    validate_dimension(output_dimension)
}

fn validate_dimension(output_dimension: u32) -> Result<(), MultiscaleEmbeddingError> {
    if output_dimension == 0 || output_dimension > MAX_DIMENSION {
        return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingProvenance);
    }
    Ok(())
}

fn validate_support(
    owning_slide_id: &SlideId,
    support: &MultiscaleEmbeddingSupport,
    expected_variant: MultiscaleEmbeddingSupportVariant,
) -> Result<(), MultiscaleEmbeddingError> {
    if support.variant() != expected_variant || support.owning_slide_id() != owning_slide_id {
        return Err(MultiscaleEmbeddingError::MultiscaleProvenanceSupportMismatch);
    }
    Ok(())
}

fn validate_derivation(
    derivation: &MultiscaleEmbeddingDerivationContract,
    expected_algorithm: &str,
) -> Result<(), MultiscaleEmbeddingError> {
    if derivation.algorithm() != expected_algorithm {
        return Err(MultiscaleEmbeddingError::MultiscaleProvenanceDerivationMismatch);
    }
    Ok(())
}

fn validate_dependencies(evidence: &ProvenanceEvidence) -> Result<(), MultiscaleEmbeddingError> {
    let (mut dependencies, count) = evidence.dependencies();
    let active = &mut dependencies[..count];
    active.sort_unstable();
    if active.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(MultiscaleEmbeddingError::DuplicateMultiscaleProvenanceArtifactDependency);
    }
    Ok(())
}

fn retained_bytes(
    slide_text_bytes: usize,
    heap_text_bytes: usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<MultiscaleEmbeddingProvenance>()
        .checked_add(slide_text_bytes)
        .and_then(|value| value.checked_add(heap_text_bytes))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}
