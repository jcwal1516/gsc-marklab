use marklab_project::{ArtifactId, ContentDigest};
use serde::{Serialize, Serializer};

use super::super::{
    DerivedRegionEvidence, DerivedSlideEvidence, DirectPatchEvidence,
    MultiscaleEmbeddingProvenance, ProvenanceEvidence, FORMAT, VERSION,
};
use crate::multiscale::records::codec::{serialize_artifact, serialize_digest};

#[derive(Serialize)]
#[serde(untagged)]
pub(in crate::multiscale::records::provenance) enum ProvenanceWireRef<'a> {
    Direct(DirectWireRef<'a>),
    Region(RegionWireRef<'a>),
    SlidePatches(SlideWireRef<'a>),
    SlideRegions(SlideWireRef<'a>),
}

impl<'a> ProvenanceWireRef<'a> {
    pub(in crate::multiscale::records::provenance) fn new(
        provenance: &'a MultiscaleEmbeddingProvenance,
    ) -> Self {
        match &provenance.evidence {
            ProvenanceEvidence::DirectPatch(value) => {
                Self::Direct(DirectWireRef::new(provenance, value))
            }
            ProvenanceEvidence::DerivedRegion(value) => {
                Self::Region(RegionWireRef::new(provenance, value))
            }
            ProvenanceEvidence::DerivedSlideFromPatches(value) => {
                Self::SlidePatches(SlideWireRef::new(provenance, value, true))
            }
            ProvenanceEvidence::DerivedSlideFromRegions(value) => {
                Self::SlideRegions(SlideWireRef::new(provenance, value, false))
            }
        }
    }
}

#[derive(Serialize)]
pub(in crate::multiscale::records::provenance) struct DirectWireRef<'a> {
    format: &'static str,
    version: u32,
    variant: &'static str,
    entity_kind: &'static str,
    owning_slide_id: &'a str,
    output_dimension: u32,
    dtype: &'static str,
    pooling_or_aggregation: &'a str,
    model_family: &'a str,
    model_version: &'a str,
    encoder_architecture: &'a str,
    #[serde(serialize_with = "serialize_artifact")]
    checkpoint_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_digest")]
    checkpoint_content_sha256: &'a ContentDigest,
    #[serde(serialize_with = "serialize_artifact")]
    source_snapshot_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    license_record_artifact_id: &'a ArtifactId,
    license_spdx: &'a str,
    citation: &'a str,
    extraction_tensor: &'a str,
    extraction_layer: u32,
    #[serde(serialize_with = "serialize_artifact")]
    input_normalization_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    preprocessing_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    run_config_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    environment_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    converter_artifact_id: &'a ArtifactId,
    converter_name: &'a str,
    converter_version: &'a str,
    #[serde(serialize_with = "serialize_artifact")]
    source_entities_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    source_vectors_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    expected_patches_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    identity_map_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    source_row_link_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    patch_support_artifact_id: &'a ArtifactId,
}

impl<'a> DirectWireRef<'a> {
    fn new(provenance: &'a MultiscaleEmbeddingProvenance, value: &'a DirectPatchEvidence) -> Self {
        let model = &value.model;
        let execution = &value.execution;
        let inputs = &value.inputs;
        Self {
            format: FORMAT,
            version: VERSION,
            variant: "direct_patch",
            entity_kind: "patch",
            owning_slide_id: provenance.owning_slide_id.as_str(),
            output_dimension: provenance.output_dimension,
            dtype: "f32",
            pooling_or_aggregation: &value.pooling,
            model_family: &model.model_family,
            model_version: &model.model_version,
            encoder_architecture: &model.encoder_architecture,
            checkpoint_artifact_id: &model.checkpoint_artifact_id,
            checkpoint_content_sha256: &model.checkpoint_content_sha256,
            source_snapshot_artifact_id: &model.source_snapshot_artifact_id,
            license_record_artifact_id: &model.license_record_artifact_id,
            license_spdx: &model.license_spdx,
            citation: &model.citation,
            extraction_tensor: &model.extraction_tensor,
            extraction_layer: model.extraction_layer,
            input_normalization_artifact_id: &inputs.input_normalization_artifact_id,
            preprocessing_artifact_id: &value.preprocessing_artifact_id,
            run_config_artifact_id: &execution.run_config_artifact_id,
            environment_artifact_id: &execution.environment_artifact_id,
            converter_artifact_id: &execution.converter_artifact_id,
            converter_name: &execution.converter_name,
            converter_version: &execution.converter_version,
            source_entities_artifact_id: &inputs.source_entities_artifact_id,
            source_vectors_artifact_id: &inputs.source_vectors_artifact_id,
            expected_patches_artifact_id: &inputs.expected_patches_artifact_id,
            identity_map_artifact_id: &inputs.identity_map_artifact_id,
            source_row_link_artifact_id: &inputs.source_row_link_artifact_id,
            patch_support_artifact_id: &inputs.patch_support_artifact_id,
        }
    }
}

#[derive(Serialize)]
pub(in crate::multiscale::records::provenance) struct RegionWireRef<'a> {
    format: &'static str,
    version: u32,
    variant: &'static str,
    entity_kind: &'static str,
    owning_slide_id: &'a str,
    output_dimension: u32,
    dtype: &'static str,
    pooling_or_aggregation: &'static str,
    #[serde(serialize_with = "serialize_artifact")]
    run_config_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    environment_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    converter_artifact_id: &'a ArtifactId,
    converter_name: &'a str,
    converter_version: &'a str,
    #[serde(serialize_with = "serialize_artifact")]
    source_patch_table_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    patch_region_link_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    expected_regions_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    region_support_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    derivation_contract_artifact_id: &'a ArtifactId,
}

impl<'a> RegionWireRef<'a> {
    fn new(
        provenance: &'a MultiscaleEmbeddingProvenance,
        value: &'a DerivedRegionEvidence,
    ) -> Self {
        Self {
            format: FORMAT,
            version: VERSION,
            variant: "derived_region",
            entity_kind: "region",
            owning_slide_id: provenance.owning_slide_id.as_str(),
            output_dimension: provenance.output_dimension,
            dtype: "f32",
            pooling_or_aggregation: "weighted_mean",
            run_config_artifact_id: &value.execution.run_config_artifact_id,
            environment_artifact_id: &value.execution.environment_artifact_id,
            converter_artifact_id: &value.execution.converter_artifact_id,
            converter_name: &value.execution.converter_name,
            converter_version: &value.execution.converter_version,
            source_patch_table_artifact_id: &value.source_patch_table_artifact_id,
            patch_region_link_artifact_id: &value.patch_region_link_artifact_id,
            expected_regions_artifact_id: &value.expected_regions_artifact_id,
            region_support_artifact_id: &value.region_support_artifact_id,
            derivation_contract_artifact_id: &value.derivation_contract_artifact_id,
        }
    }
}

#[derive(Serialize)]
pub(in crate::multiscale::records::provenance) struct SlideWireRef<'a> {
    format: &'static str,
    version: u32,
    variant: &'static str,
    entity_kind: &'static str,
    owning_slide_id: &'a str,
    output_dimension: u32,
    dtype: &'static str,
    pooling_or_aggregation: &'static str,
    #[serde(serialize_with = "serialize_artifact")]
    run_config_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    environment_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    converter_artifact_id: &'a ArtifactId,
    converter_name: &'a str,
    converter_version: &'a str,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_artifact"
    )]
    source_patch_table_artifact_id: Option<&'a ArtifactId>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_artifact"
    )]
    source_region_table_artifact_id: Option<&'a ArtifactId>,
    #[serde(serialize_with = "serialize_artifact")]
    expected_slides_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    slide_support_artifact_id: &'a ArtifactId,
    #[serde(serialize_with = "serialize_artifact")]
    derivation_contract_artifact_id: &'a ArtifactId,
}

impl<'a> SlideWireRef<'a> {
    fn new(
        provenance: &'a MultiscaleEmbeddingProvenance,
        value: &'a DerivedSlideEvidence,
        from_patches: bool,
    ) -> Self {
        Self {
            format: FORMAT,
            version: VERSION,
            variant: if from_patches {
                "derived_slide_from_patches"
            } else {
                "derived_slide_from_regions"
            },
            entity_kind: "slide",
            owning_slide_id: provenance.owning_slide_id.as_str(),
            output_dimension: provenance.output_dimension,
            dtype: "f32",
            pooling_or_aggregation: "arithmetic_mean",
            run_config_artifact_id: &value.execution.run_config_artifact_id,
            environment_artifact_id: &value.execution.environment_artifact_id,
            converter_artifact_id: &value.execution.converter_artifact_id,
            converter_name: &value.execution.converter_name,
            converter_version: &value.execution.converter_version,
            source_patch_table_artifact_id: from_patches.then_some(&value.source_table_artifact_id),
            source_region_table_artifact_id: (!from_patches)
                .then_some(&value.source_table_artifact_id),
            expected_slides_artifact_id: &value.expected_slides_artifact_id,
            slide_support_artifact_id: &value.slide_support_artifact_id,
            derivation_contract_artifact_id: &value.derivation_contract_artifact_id,
        }
    }
}

fn serialize_optional_artifact<S: Serializer>(
    value: &Option<&ArtifactId>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serialize_artifact(value, serializer),
        None => serializer.serialize_none(),
    }
}
