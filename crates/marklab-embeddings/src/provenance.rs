use marklab_project::{ArtifactId, ContentDigest};

use crate::EmbeddingError;

mod validation;
mod wire;

pub use validation::{
    ArtifactAvailabilityFailure, CellEmbeddingArtifactRole, EmbeddingArtifactGraphError,
    VerifiedCellEmbeddingArtifactGraph,
};

const RAW_CELLVIT_DIMENSION: u32 = 1_280;

/// Canonical bounded fixed-point decimal represented as a JSON string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDecimal(String);

impl CanonicalDecimal {
    /// Validate the frozen fixed-point grammar without converting through floating point.
    pub fn new(value: impl Into<String>) -> Result<Self, EmbeddingError> {
        let value = value.into();
        if !is_canonical_decimal(&value) {
            return Err(EmbeddingError::InvalidCanonicalDecimal);
        }
        Ok(Self(value))
    }

    /// Borrow the exact canonical decimal text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_positive(&self) -> bool {
        !self.0.starts_with('-') && self.0 != "0"
    }
}

/// Model, checkpoint, license, and extraction-layer provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingModelProvenance {
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
}

impl CellEmbeddingModelProvenance {
    /// Validate all required version-one model and license declarations.
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
    ) -> Result<Self, EmbeddingError> {
        let model_family = model_family.into();
        let model_version = model_version.into();
        let encoder_architecture = encoder_architecture.into();
        let license_spdx = license_spdx.into();
        let citation = citation.into();
        let extraction_tensor = extraction_tensor.into();
        let values = [
            model_family.as_str(),
            model_version.as_str(),
            encoder_architecture.as_str(),
            license_spdx.as_str(),
            citation.as_str(),
            extraction_tensor.as_str(),
        ];
        if extraction_layer == 0 || values.contains(&"") {
            return Err(EmbeddingError::ProvenanceIncomplete);
        }
        if values.into_iter().any(|value| !valid_token(value)) {
            return Err(EmbeddingError::InvalidProvenance);
        }
        Ok(Self {
            model_family,
            model_version,
            encoder_architecture,
            checkpoint_artifact_id,
            checkpoint_content_sha256,
            source_snapshot_artifact_id,
            license_record_artifact_id,
            license_spdx,
            citation,
            extraction_tensor,
            extraction_layer,
        })
    }
}

/// Closed raw RGB H&E tensor and normalization contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingTensorContract {
    output_dimension: u32,
    input_channel_mean: [CanonicalDecimal; 3],
    input_channel_std: [CanonicalDecimal; 3],
}

impl CellEmbeddingTensorContract {
    /// Construct the version-one raw CellViT RGB H&E tensor profile.
    pub fn rgb_he_raw(
        output_dimension: u32,
        input_channel_mean: [CanonicalDecimal; 3],
        input_channel_std: [CanonicalDecimal; 3],
    ) -> Result<Self, EmbeddingError> {
        if output_dimension == 0 {
            return Err(EmbeddingError::ProvenanceIncomplete);
        }
        if output_dimension != RAW_CELLVIT_DIMENSION
            || input_channel_std.iter().any(|value| !value.is_positive())
        {
            return Err(EmbeddingError::InvalidProvenance);
        }
        Ok(Self {
            output_dimension,
            input_channel_mean,
            input_channel_std,
        })
    }

    /// Fixed output-vector dimension.
    pub fn output_dimension(&self) -> u32 {
        self.output_dimension
    }
}

/// Execution, converter, and importer provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingExecutionProvenance {
    preprocessing_artifact_id: ArtifactId,
    run_config_artifact_id: ArtifactId,
    environment_artifact_id: ArtifactId,
    converter_artifact_id: ArtifactId,
    converter_name: String,
    converter_version: String,
    importer_profile: String,
    importer_version: String,
}

impl CellEmbeddingExecutionProvenance {
    /// Validate exact execution dependencies and bounded converter/importer tokens.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        preprocessing_artifact_id: ArtifactId,
        run_config_artifact_id: ArtifactId,
        environment_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        converter_name: impl Into<String>,
        converter_version: impl Into<String>,
        importer_profile: impl Into<String>,
        importer_version: impl Into<String>,
    ) -> Result<Self, EmbeddingError> {
        let converter_name = converter_name.into();
        let converter_version = converter_version.into();
        let importer_profile = importer_profile.into();
        let importer_version = importer_version.into();
        let values = [
            converter_name.as_str(),
            converter_version.as_str(),
            importer_profile.as_str(),
            importer_version.as_str(),
        ];
        if values.contains(&"") {
            return Err(EmbeddingError::ProvenanceIncomplete);
        }
        if values.into_iter().any(|value| !valid_token(value)) {
            return Err(EmbeddingError::InvalidProvenance);
        }
        Ok(Self {
            preprocessing_artifact_id,
            run_config_artifact_id,
            environment_artifact_id,
            converter_artifact_id,
            converter_name,
            converter_version,
            importer_profile,
            importer_version,
        })
    }
}

/// Exact source, identity, context, and row-link artifact inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingInputArtifacts {
    source_cells_artifact_id: ArtifactId,
    source_vectors_artifact_id: ArtifactId,
    expected_cells_artifact_id: ArtifactId,
    identity_map_artifact_id: ArtifactId,
    spatial_context_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
}

impl CellEmbeddingInputArtifacts {
    /// Bind all six required embedding input artifacts by exact identity.
    pub fn new(
        source_cells_artifact_id: ArtifactId,
        source_vectors_artifact_id: ArtifactId,
        expected_cells_artifact_id: ArtifactId,
        identity_map_artifact_id: ArtifactId,
        spatial_context_artifact_id: ArtifactId,
        row_link_artifact_id: ArtifactId,
    ) -> Self {
        Self {
            source_cells_artifact_id,
            source_vectors_artifact_id,
            expected_cells_artifact_id,
            identity_map_artifact_id,
            spatial_context_artifact_id,
            row_link_artifact_id,
        }
    }
}

/// Strict complete version-one cell-embedding provenance artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingProvenance {
    model: CellEmbeddingModelProvenance,
    tensor: CellEmbeddingTensorContract,
    execution: CellEmbeddingExecutionProvenance,
    inputs: CellEmbeddingInputArtifacts,
}

impl CellEmbeddingProvenance {
    /// Validate a complete provenance value and its exact distinct dependency set.
    pub fn new(
        model: CellEmbeddingModelProvenance,
        tensor: CellEmbeddingTensorContract,
        execution: CellEmbeddingExecutionProvenance,
        inputs: CellEmbeddingInputArtifacts,
    ) -> Result<Self, EmbeddingError> {
        let provenance = Self {
            model,
            tensor,
            execution,
            inputs,
        };
        let dependencies = provenance.direct_dependencies();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(EmbeddingError::DuplicateArtifactDependency);
        }
        Ok(provenance)
    }

    /// Fixed output-vector dimension.
    pub fn output_dimension(&self) -> u32 {
        self.tensor.output_dimension
    }

    /// Sorted exact set of thirteen direct artifact dependencies.
    pub fn direct_dependencies(&self) -> [ArtifactId; 13] {
        let mut dependencies = [
            self.model.checkpoint_artifact_id,
            self.model.source_snapshot_artifact_id,
            self.model.license_record_artifact_id,
            self.execution.preprocessing_artifact_id,
            self.execution.run_config_artifact_id,
            self.execution.environment_artifact_id,
            self.execution.converter_artifact_id,
            self.inputs.source_cells_artifact_id,
            self.inputs.source_vectors_artifact_id,
            self.inputs.expected_cells_artifact_id,
            self.inputs.identity_map_artifact_id,
            self.inputs.spatial_context_artifact_id,
            self.inputs.row_link_artifact_id,
        ];
        dependencies.sort_unstable();
        dependencies
    }
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

fn is_canonical_decimal(value: &str) -> bool {
    if value.is_empty() || value.starts_with('+') {
        return false;
    }
    let (negative, unsigned) = match value.strip_prefix('-') {
        Some(unsigned) => (true, unsigned),
        None => (false, value),
    };
    if unsigned.is_empty() {
        return false;
    }
    let mut parts = unsigned.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
    {
        return false;
    }
    if let Some(fraction) = fraction {
        if fraction.is_empty()
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.ends_with('0')
        {
            return false;
        }
    }
    !(negative && unsigned == "0")
}
