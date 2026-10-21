use marklab_project::{ArtifactId, ContentDigest};

use crate::{canonical_token::valid_token, EmbeddingError};

mod validation;
mod wire;

pub(crate) use validation::artifact_availability_failure;
pub use validation::{
    ArtifactAvailabilityFailure, CellEmbeddingArtifactRole, EmbeddingArtifactGraphError,
    VerifiedCellEmbeddingArtifactGraph,
};

const RAW_CELLVIT_DIMENSION: u32 = 1_280;
const MAX_DIMENSION: u32 = 65_536;
const MAX_INPUT_CHANNELS: usize = 256;

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

/// Declared tensor semantics, with the original CellViT profile retained for compatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingTensorContract {
    version: u32,
    output_dimension: u32,
    pooling: String,
    output_normalization: String,
    input_channel_order: String,
    input_channel_mean: Vec<CanonicalDecimal>,
    input_channel_std: Vec<CanonicalDecimal>,
    modality: String,
    stain: String,
}

impl CellEmbeddingTensorContract {
    /// Declare a model-independent version-two tensor contract.
    ///
    /// Pooling and normalization describe the producer's output; Marklab does not apply
    /// them again. Dimensions are bounded at 65,536 and channel declarations at 256.
    /// Empty input normalization vectors require an explicit `none` channel order.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        output_dimension: u32,
        pooling: impl Into<String>,
        output_normalization: impl Into<String>,
        input_channel_order: impl Into<String>,
        input_channel_mean: Vec<CanonicalDecimal>,
        input_channel_std: Vec<CanonicalDecimal>,
        modality: impl Into<String>,
        stain: impl Into<String>,
    ) -> Result<Self, EmbeddingError> {
        let tensor = Self {
            version: 2,
            output_dimension,
            pooling: pooling.into(),
            output_normalization: output_normalization.into(),
            input_channel_order: input_channel_order.into(),
            input_channel_mean,
            input_channel_std,
            modality: modality.into(),
            stain: stain.into(),
        };
        let declarations = [
            tensor.pooling.as_str(),
            tensor.output_normalization.as_str(),
            tensor.input_channel_order.as_str(),
            tensor.modality.as_str(),
            tensor.stain.as_str(),
        ];
        if output_dimension == 0 || declarations.contains(&"") {
            return Err(EmbeddingError::ProvenanceIncomplete);
        }
        if output_dimension > MAX_DIMENSION
            || declarations.into_iter().any(|value| !valid_token(value))
            || tensor.input_channel_mean.len() != tensor.input_channel_std.len()
            || tensor.input_channel_mean.len() > MAX_INPUT_CHANNELS
            || (tensor.input_channel_mean.is_empty() != (tensor.input_channel_order == "none"))
            || tensor
                .input_channel_std
                .iter()
                .any(|value| !value.is_positive())
        {
            return Err(EmbeddingError::InvalidProvenance);
        }
        Ok(tensor)
    }

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
            version: 1,
            output_dimension,
            pooling: "nucleus_bbox_intersecting_token_mean".to_owned(),
            output_normalization: "none".to_owned(),
            input_channel_order: "rgb".to_owned(),
            input_channel_mean: input_channel_mean.into(),
            input_channel_std: input_channel_std.into(),
            modality: "brightfield".to_owned(),
            stain: "he".to_owned(),
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

/// Complete cell-embedding provenance, including the readable legacy version-one profile.
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

    /// Provenance schema version: one for legacy CellViT, two for declared model outputs.
    pub fn format_version(&self) -> u32 {
        self.tensor.version
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
