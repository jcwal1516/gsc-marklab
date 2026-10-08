use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

use super::{
    types::{
        MultiscaleDirectPatchInputArtifacts, MultiscaleDirectPatchModelProvenance,
        MultiscaleEmbeddingExecutionProvenance,
    },
    MultiscaleEmbeddingProvenanceVariant, FORMAT, VERSION,
};
use crate::multiscale::{digest::LogicalDigest, error::MultiscaleEmbeddingError};

const DOMAIN: &[u8] = b"marklab-multiscale-embedding-provenance-logical-v1";

#[derive(Clone, Eq, PartialEq)]
pub(super) struct DirectPatchEvidence {
    pub(super) pooling: Box<str>,
    pub(super) model: MultiscaleDirectPatchModelProvenance,
    pub(super) execution: MultiscaleEmbeddingExecutionProvenance,
    pub(super) preprocessing_artifact_id: ArtifactId,
    pub(super) inputs: MultiscaleDirectPatchInputArtifacts,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct DerivedRegionEvidence {
    pub(super) execution: MultiscaleEmbeddingExecutionProvenance,
    pub(super) source_patch_table_artifact_id: ArtifactId,
    pub(super) patch_region_link_artifact_id: ArtifactId,
    pub(super) expected_regions_artifact_id: ArtifactId,
    pub(super) region_support_artifact_id: ArtifactId,
    pub(super) derivation_contract_artifact_id: ArtifactId,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct DerivedSlideEvidence {
    pub(super) execution: MultiscaleEmbeddingExecutionProvenance,
    pub(super) source_table_artifact_id: ArtifactId,
    pub(super) expected_slides_artifact_id: ArtifactId,
    pub(super) slide_support_artifact_id: ArtifactId,
    pub(super) derivation_contract_artifact_id: ArtifactId,
}

#[derive(Clone, Eq, PartialEq)]
// Keeping every fixed role inline avoids a hidden heap allocation before the caller's retained
// budget is checked. The largest closed variant is still below one KiB.
#[allow(clippy::large_enum_variant)]
pub(super) enum ProvenanceEvidence {
    DirectPatch(DirectPatchEvidence),
    DerivedRegion(DerivedRegionEvidence),
    DerivedSlideFromPatches(DerivedSlideEvidence),
    DerivedSlideFromRegions(DerivedSlideEvidence),
}

impl ProvenanceEvidence {
    pub(super) fn variant(&self) -> MultiscaleEmbeddingProvenanceVariant {
        match self {
            Self::DirectPatch(_) => MultiscaleEmbeddingProvenanceVariant::DirectPatch,
            Self::DerivedRegion(_) => MultiscaleEmbeddingProvenanceVariant::DerivedRegion,
            Self::DerivedSlideFromPatches(_) => {
                MultiscaleEmbeddingProvenanceVariant::DerivedSlideFromPatches
            }
            Self::DerivedSlideFromRegions(_) => {
                MultiscaleEmbeddingProvenanceVariant::DerivedSlideFromRegions
            }
        }
    }

    pub(super) fn pooling_or_aggregation(&self) -> &str {
        match self {
            Self::DirectPatch(value) => &value.pooling,
            Self::DerivedRegion(_) => "weighted_mean",
            Self::DerivedSlideFromPatches(_) | Self::DerivedSlideFromRegions(_) => {
                "arithmetic_mean"
            }
        }
    }

    pub(super) fn heap_text_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        match self {
            Self::DirectPatch(value) => {
                let model_and_pooling = value
                    .pooling
                    .len()
                    .checked_add(value.model.heap_text_bytes()?)
                    .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
                model_and_pooling
                    .checked_add(value.execution.heap_text_bytes()?)
                    .ok_or(MultiscaleEmbeddingError::SizeOverflow)
            }
            Self::DerivedRegion(value) => value.execution.heap_text_bytes(),
            Self::DerivedSlideFromPatches(value) | Self::DerivedSlideFromRegions(value) => {
                value.execution.heap_text_bytes()
            }
        }
    }

    pub(super) fn dependencies(&self) -> ([ArtifactId; 14], usize) {
        match self {
            Self::DirectPatch(value) => {
                let model = &value.model;
                let execution = &value.execution;
                let inputs = &value.inputs;
                (
                    [
                        model.checkpoint_artifact_id,
                        model.source_snapshot_artifact_id,
                        model.license_record_artifact_id,
                        inputs.input_normalization_artifact_id,
                        value.preprocessing_artifact_id,
                        execution.run_config_artifact_id,
                        execution.environment_artifact_id,
                        execution.converter_artifact_id,
                        inputs.source_entities_artifact_id,
                        inputs.source_vectors_artifact_id,
                        inputs.expected_patches_artifact_id,
                        inputs.identity_map_artifact_id,
                        inputs.source_row_link_artifact_id,
                        inputs.patch_support_artifact_id,
                    ],
                    14,
                )
            }
            Self::DerivedRegion(value) => {
                let execution = &value.execution;
                let first = execution.run_config_artifact_id;
                (
                    [
                        first,
                        execution.environment_artifact_id,
                        execution.converter_artifact_id,
                        value.source_patch_table_artifact_id,
                        value.patch_region_link_artifact_id,
                        value.expected_regions_artifact_id,
                        value.region_support_artifact_id,
                        value.derivation_contract_artifact_id,
                        first,
                        first,
                        first,
                        first,
                        first,
                        first,
                    ],
                    8,
                )
            }
            Self::DerivedSlideFromPatches(value) | Self::DerivedSlideFromRegions(value) => {
                let execution = &value.execution;
                let first = execution.run_config_artifact_id;
                (
                    [
                        first,
                        execution.environment_artifact_id,
                        execution.converter_artifact_id,
                        value.source_table_artifact_id,
                        value.expected_slides_artifact_id,
                        value.slide_support_artifact_id,
                        value.derivation_contract_artifact_id,
                        first,
                        first,
                        first,
                        first,
                        first,
                        first,
                        first,
                    ],
                    7,
                )
            }
        }
    }
}

pub(super) fn provenance_digest(
    owning_slide_id: &SlideId,
    output_dimension: u32,
    evidence: &ProvenanceEvidence,
) -> ContentDigest {
    let variant = evidence.variant();
    let mut digest = LogicalDigest::new(DOMAIN);
    digest.text(FORMAT);
    digest.u32(VERSION);
    digest.text(variant.wire_name());
    digest.text(variant.entity_kind().wire_name());
    digest.text(owning_slide_id.as_str());
    digest.u32(output_dimension);
    digest.text("f32");
    digest.text(evidence.pooling_or_aggregation());
    match evidence {
        ProvenanceEvidence::DirectPatch(value) => {
            let model = &value.model;
            let execution = &value.execution;
            let inputs = &value.inputs;
            digest.text(&model.model_family);
            digest.text(&model.model_version);
            digest.text(&model.encoder_architecture);
            digest.artifact_id(model.checkpoint_artifact_id);
            digest.content_digest(model.checkpoint_content_sha256);
            digest.artifact_id(model.source_snapshot_artifact_id);
            digest.artifact_id(model.license_record_artifact_id);
            digest.text(&model.license_spdx);
            digest.text(&model.citation);
            digest.text(&model.extraction_tensor);
            digest.u32(model.extraction_layer);
            digest.artifact_id(inputs.input_normalization_artifact_id);
            digest.artifact_id(value.preprocessing_artifact_id);
            digest.artifact_id(execution.run_config_artifact_id);
            digest.artifact_id(execution.environment_artifact_id);
            digest.artifact_id(execution.converter_artifact_id);
            digest.text(&execution.converter_name);
            digest.text(&execution.converter_version);
            digest.artifact_id(inputs.source_entities_artifact_id);
            digest.artifact_id(inputs.source_vectors_artifact_id);
            digest.artifact_id(inputs.expected_patches_artifact_id);
            digest.artifact_id(inputs.identity_map_artifact_id);
            digest.artifact_id(inputs.source_row_link_artifact_id);
            digest.artifact_id(inputs.patch_support_artifact_id);
        }
        ProvenanceEvidence::DerivedRegion(value) => {
            digest_execution(&mut digest, &value.execution);
            digest.artifact_id(value.source_patch_table_artifact_id);
            digest.artifact_id(value.patch_region_link_artifact_id);
            digest.artifact_id(value.expected_regions_artifact_id);
            digest.artifact_id(value.region_support_artifact_id);
            digest.artifact_id(value.derivation_contract_artifact_id);
        }
        ProvenanceEvidence::DerivedSlideFromPatches(value)
        | ProvenanceEvidence::DerivedSlideFromRegions(value) => {
            digest_execution(&mut digest, &value.execution);
            digest.artifact_id(value.source_table_artifact_id);
            digest.artifact_id(value.expected_slides_artifact_id);
            digest.artifact_id(value.slide_support_artifact_id);
            digest.artifact_id(value.derivation_contract_artifact_id);
        }
    }
    digest.finish()
}

fn digest_execution(
    digest: &mut LogicalDigest,
    execution: &MultiscaleEmbeddingExecutionProvenance,
) {
    digest.artifact_id(execution.run_config_artifact_id);
    digest.artifact_id(execution.environment_artifact_id);
    digest.artifact_id(execution.converter_artifact_id);
    digest.text(&execution.converter_name);
    digest.text(&execution.converter_version);
}
