use std::{fmt, mem::size_of};

use marklab_project::{ArtifactId, ContentDigest};

use crate::multiscale::{
    error::MultiscaleEmbeddingError,
    records::codec::{require_retained, valid_token},
};

/// Model, checkpoint, reviewed-source, license, and extraction declarations for direct patches.
#[derive(Clone, Eq, PartialEq)]
pub struct MultiscaleDirectPatchModelProvenance {
    pub(super) model_family: Box<str>,
    pub(super) model_version: Box<str>,
    pub(super) encoder_architecture: Box<str>,
    pub(super) checkpoint_artifact_id: ArtifactId,
    pub(super) checkpoint_content_sha256: ContentDigest,
    pub(super) source_snapshot_artifact_id: ArtifactId,
    pub(super) license_record_artifact_id: ArtifactId,
    pub(super) license_spdx: Box<str>,
    pub(super) citation: Box<str>,
    pub(super) extraction_tensor: Box<str>,
    pub(super) extraction_layer: u32,
}

impl fmt::Debug for MultiscaleDirectPatchModelProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiscaleDirectPatchModelProvenance")
            .finish_non_exhaustive()
    }
}

impl MultiscaleDirectPatchModelProvenance {
    /// Validate all version-one direct model, license, citation, and extraction declarations.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model_family: impl Into<String>,
        model_version: impl Into<String>,
        encoder_architecture: impl Into<String>,
        checkpoint_artifact_id: ArtifactId,
        checkpoint_content_sha256: ContentDigest,
        source_snapshot_artifact_id: ArtifactId,
        license_record_artifact_id: ArtifactId,
        license_spdx: impl Into<String>,
        citation: impl Into<String>,
        extraction_tensor: impl Into<String>,
        extraction_layer: u32,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let model_family = model_family.into();
        let model_version = model_version.into();
        let encoder_architecture = encoder_architecture.into();
        let license_spdx = license_spdx.into();
        let citation = citation.into();
        let extraction_tensor = extraction_tensor.into();
        validate_model_text(
            &model_family,
            &model_version,
            &encoder_architecture,
            &license_spdx,
            &citation,
            &extraction_tensor,
            extraction_layer,
        )?;
        let text_capacity = [
            model_family.capacity(),
            model_version.capacity(),
            encoder_architecture.capacity(),
            license_spdx.capacity(),
            citation.capacity(),
            extraction_tensor.capacity(),
        ]
        .into_iter()
        .try_fold(0_usize, checked_add)?;
        require_retained(
            size_of::<Self>()
                .checked_add(text_capacity)
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
            maximum_retained_bytes,
        )?;
        Ok(Self {
            model_family: model_family.into_boxed_str(),
            model_version: model_version.into_boxed_str(),
            encoder_architecture: encoder_architecture.into_boxed_str(),
            checkpoint_artifact_id,
            checkpoint_content_sha256,
            source_snapshot_artifact_id,
            license_record_artifact_id,
            license_spdx: license_spdx.into_boxed_str(),
            citation: citation.into_boxed_str(),
            extraction_tensor: extraction_tensor.into_boxed_str(),
            extraction_layer,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_owned(
        model_family: String,
        model_version: String,
        encoder_architecture: String,
        checkpoint_artifact_id: ArtifactId,
        checkpoint_content_sha256: ContentDigest,
        source_snapshot_artifact_id: ArtifactId,
        license_record_artifact_id: ArtifactId,
        license_spdx: String,
        citation: String,
        extraction_tensor: String,
        extraction_layer: u32,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_model_text(
            &model_family,
            &model_version,
            &encoder_architecture,
            &license_spdx,
            &citation,
            &extraction_tensor,
            extraction_layer,
        )?;
        Ok(Self {
            model_family: model_family.into_boxed_str(),
            model_version: model_version.into_boxed_str(),
            encoder_architecture: encoder_architecture.into_boxed_str(),
            checkpoint_artifact_id,
            checkpoint_content_sha256,
            source_snapshot_artifact_id,
            license_record_artifact_id,
            license_spdx: license_spdx.into_boxed_str(),
            citation: citation.into_boxed_str(),
            extraction_tensor: extraction_tensor.into_boxed_str(),
            extraction_layer,
        })
    }

    pub(super) fn heap_text_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        [
            self.model_family.len(),
            self.model_version.len(),
            self.encoder_architecture.len(),
            self.license_spdx.len(),
            self.citation.len(),
            self.extraction_tensor.len(),
        ]
        .into_iter()
        .try_fold(0_usize, checked_add)
    }
}

/// Shared run/environment/converter provenance for every C-05 provenance variant.
#[derive(Clone, Eq, PartialEq)]
pub struct MultiscaleEmbeddingExecutionProvenance {
    pub(super) run_config_artifact_id: ArtifactId,
    pub(super) environment_artifact_id: ArtifactId,
    pub(super) converter_artifact_id: ArtifactId,
    pub(super) converter_name: Box<str>,
    pub(super) converter_version: Box<str>,
}

impl fmt::Debug for MultiscaleEmbeddingExecutionProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiscaleEmbeddingExecutionProvenance")
            .finish_non_exhaustive()
    }
}

impl MultiscaleEmbeddingExecutionProvenance {
    /// Validate exact execution roles and bounded converter identity tokens.
    pub fn new(
        run_config_artifact_id: ArtifactId,
        environment_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        converter_name: impl Into<String>,
        converter_version: impl Into<String>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let converter_name = converter_name.into();
        let converter_version = converter_version.into();
        validate_execution_text(&converter_name, &converter_version)?;
        let required = size_of::<Self>()
            .checked_add(converter_name.capacity())
            .and_then(|value| value.checked_add(converter_version.capacity()))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        require_retained(required, maximum_retained_bytes)?;
        Ok(Self {
            run_config_artifact_id,
            environment_artifact_id,
            converter_artifact_id,
            converter_name: converter_name.into_boxed_str(),
            converter_version: converter_version.into_boxed_str(),
        })
    }

    pub(super) fn from_owned(
        run_config_artifact_id: ArtifactId,
        environment_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        converter_name: String,
        converter_version: String,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_execution_text(&converter_name, &converter_version)?;
        Ok(Self {
            run_config_artifact_id,
            environment_artifact_id,
            converter_artifact_id,
            converter_name: converter_name.into_boxed_str(),
            converter_version: converter_version.into_boxed_str(),
        })
    }

    pub(super) fn heap_text_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        self.converter_name
            .len()
            .checked_add(self.converter_version.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    }
}

/// Exact seven artifact roles used only by direct-patch provenance.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct MultiscaleDirectPatchInputArtifacts {
    pub(super) input_normalization_artifact_id: ArtifactId,
    pub(super) source_entities_artifact_id: ArtifactId,
    pub(super) source_vectors_artifact_id: ArtifactId,
    pub(super) expected_patches_artifact_id: ArtifactId,
    pub(super) identity_map_artifact_id: ArtifactId,
    pub(super) source_row_link_artifact_id: ArtifactId,
    pub(super) patch_support_artifact_id: ArtifactId,
}

impl fmt::Debug for MultiscaleDirectPatchInputArtifacts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiscaleDirectPatchInputArtifacts")
            .finish_non_exhaustive()
    }
}

impl MultiscaleDirectPatchInputArtifacts {
    /// Bind the normalization, source, identity, row-link, expected-set, and support roles.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input_normalization_artifact_id: ArtifactId,
        source_entities_artifact_id: ArtifactId,
        source_vectors_artifact_id: ArtifactId,
        expected_patches_artifact_id: ArtifactId,
        identity_map_artifact_id: ArtifactId,
        source_row_link_artifact_id: ArtifactId,
        patch_support_artifact_id: ArtifactId,
    ) -> Self {
        Self {
            input_normalization_artifact_id,
            source_entities_artifact_id,
            source_vectors_artifact_id,
            expected_patches_artifact_id,
            identity_map_artifact_id,
            source_row_link_artifact_id,
            patch_support_artifact_id,
        }
    }
}

fn validate_model_text(
    model_family: &str,
    model_version: &str,
    encoder_architecture: &str,
    license_spdx: &str,
    citation: &str,
    extraction_tensor: &str,
    extraction_layer: u32,
) -> Result<(), MultiscaleEmbeddingError> {
    if extraction_layer == 0
        || [
            model_family,
            model_version,
            encoder_architecture,
            license_spdx,
            extraction_tensor,
        ]
        .into_iter()
        .any(|value| !valid_token(value))
        || citation.is_empty()
        || citation.len() > 4_096
        || citation.trim() != citation
        || citation.chars().any(char::is_control)
    {
        return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingProvenance);
    }
    Ok(())
}

fn validate_execution_text(
    converter_name: &str,
    converter_version: &str,
) -> Result<(), MultiscaleEmbeddingError> {
    if !valid_token(converter_name) || !valid_token(converter_version) {
        return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingProvenance);
    }
    Ok(())
}

fn checked_add(total: usize, value: usize) -> Result<usize, MultiscaleEmbeddingError> {
    total
        .checked_add(value)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}
