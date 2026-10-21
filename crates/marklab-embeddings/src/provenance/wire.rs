use std::str::FromStr;

use marklab_project::{ArtifactId, ContentDigest};
use serde::{Deserialize, Serialize};

use super::{
    CanonicalDecimal, CellEmbeddingExecutionProvenance, CellEmbeddingInputArtifacts,
    CellEmbeddingModelProvenance, CellEmbeddingProvenance, CellEmbeddingTensorContract,
};
use crate::EmbeddingError;

const PROVENANCE_FORMAT: &str = "marklab.cell_embedding_provenance";
const MAX_PROVENANCE_BYTES: usize = 64 * 1024;

impl CellEmbeddingProvenance {
    /// Encode deterministic strict JSON with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, EmbeddingError> {
        let mut bytes = serde_json::to_vec(&WireProvenance::from(self))
            .map_err(|_| EmbeddingError::InvalidProvenanceJson)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_PROVENANCE_BYTES {
            return Err(EmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: MAX_PROVENANCE_BYTES,
            });
        }
        Ok(bytes)
    }

    /// Decode strict provenance JSON and require byte-for-byte canonical equality.
    pub fn from_canonical_json(bytes: &[u8]) -> Result<Self, EmbeddingError> {
        if bytes.len() > MAX_PROVENANCE_BYTES {
            return Err(EmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: MAX_PROVENANCE_BYTES,
            });
        }
        let wire: WireProvenance =
            serde_json::from_slice(bytes).map_err(|_| EmbeddingError::InvalidProvenanceJson)?;
        let provenance = wire.into_provenance()?;
        if provenance.to_canonical_json()?.as_slice() != bytes {
            return Err(EmbeddingError::InvalidProvenanceJson);
        }
        Ok(provenance)
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WireProvenance {
    format: String,
    version: u32,
    model_family: String,
    model_version: String,
    encoder_architecture: String,
    checkpoint_artifact_id: String,
    checkpoint_content_sha256: String,
    source_snapshot_artifact_id: String,
    license_record_artifact_id: String,
    license_spdx: String,
    citation: String,
    extraction_tensor: String,
    extraction_layer: u32,
    pooling: String,
    output_dimension: u32,
    output_dtype: String,
    output_normalization: WireNone,
    input_channel_order: String,
    input_channel_mean: Vec<String>,
    input_channel_std: Vec<String>,
    modality: String,
    stain: String,
    preprocessing_artifact_id: String,
    run_config_artifact_id: String,
    environment_artifact_id: String,
    converter_artifact_id: String,
    converter_name: String,
    converter_version: String,
    importer_profile: String,
    importer_version: String,
    source_cells_artifact_id: String,
    source_vectors_artifact_id: String,
    expected_cells_artifact_id: String,
    identity_map_artifact_id: String,
    spatial_context_artifact_id: String,
    row_link_artifact_id: String,
    missing_vector_policy: String,
}

impl From<&CellEmbeddingProvenance> for WireProvenance {
    fn from(provenance: &CellEmbeddingProvenance) -> Self {
        let model = &provenance.model;
        let tensor = &provenance.tensor;
        let execution = &provenance.execution;
        let inputs = &provenance.inputs;
        Self {
            format: PROVENANCE_FORMAT.to_owned(),
            version: provenance.format_version(),
            model_family: model.model_family.clone(),
            model_version: model.model_version.clone(),
            encoder_architecture: model.encoder_architecture.clone(),
            checkpoint_artifact_id: model.checkpoint_artifact_id.to_string(),
            checkpoint_content_sha256: model.checkpoint_content_sha256.to_string(),
            source_snapshot_artifact_id: model.source_snapshot_artifact_id.to_string(),
            license_record_artifact_id: model.license_record_artifact_id.to_string(),
            license_spdx: model.license_spdx.clone(),
            citation: model.citation.clone(),
            extraction_tensor: model.extraction_tensor.clone(),
            extraction_layer: model.extraction_layer,
            pooling: tensor.pooling.clone(),
            output_dimension: tensor.output_dimension,
            output_dtype: "f32".to_owned(),
            output_normalization: WireNone {
                kind: tensor.output_normalization.clone(),
            },
            input_channel_order: tensor.input_channel_order.clone(),
            input_channel_mean: tensor
                .input_channel_mean
                .iter()
                .map(|value| value.0.clone())
                .collect(),
            input_channel_std: tensor
                .input_channel_std
                .iter()
                .map(|value| value.0.clone())
                .collect(),
            modality: tensor.modality.clone(),
            stain: tensor.stain.clone(),
            preprocessing_artifact_id: execution.preprocessing_artifact_id.to_string(),
            run_config_artifact_id: execution.run_config_artifact_id.to_string(),
            environment_artifact_id: execution.environment_artifact_id.to_string(),
            converter_artifact_id: execution.converter_artifact_id.to_string(),
            converter_name: execution.converter_name.clone(),
            converter_version: execution.converter_version.clone(),
            importer_profile: execution.importer_profile.clone(),
            importer_version: execution.importer_version.clone(),
            source_cells_artifact_id: inputs.source_cells_artifact_id.to_string(),
            source_vectors_artifact_id: inputs.source_vectors_artifact_id.to_string(),
            expected_cells_artifact_id: inputs.expected_cells_artifact_id.to_string(),
            identity_map_artifact_id: inputs.identity_map_artifact_id.to_string(),
            spatial_context_artifact_id: inputs.spatial_context_artifact_id.to_string(),
            row_link_artifact_id: inputs.row_link_artifact_id.to_string(),
            missing_vector_policy: "explicit_status_no_imputation".to_owned(),
        }
    }
}

impl WireProvenance {
    fn into_provenance(self) -> Result<CellEmbeddingProvenance, EmbeddingError> {
        if !self.is_complete() {
            return Err(EmbeddingError::ProvenanceIncomplete);
        }
        if self.format != PROVENANCE_FORMAT
            || !matches!(self.version, 1 | 2)
            || self.output_dtype != "f32"
            || self.missing_vector_policy != "explicit_status_no_imputation"
            || (self.version == 1
                && (self.pooling != "nucleus_bbox_intersecting_token_mean"
                    || self.output_normalization.kind != "none"
                    || self.input_channel_order != "rgb"
                    || self.modality != "brightfield"
                    || self.stain != "he"))
        {
            return Err(EmbeddingError::InvalidProvenanceJson);
        }
        let model = CellEmbeddingModelProvenance::new(
            self.model_family,
            self.model_version,
            self.encoder_architecture,
            parse_artifact_id(&self.checkpoint_artifact_id)?,
            parse_digest(&self.checkpoint_content_sha256)?,
            parse_artifact_id(&self.source_snapshot_artifact_id)?,
            parse_artifact_id(&self.license_record_artifact_id)?,
            self.license_spdx,
            self.citation,
            self.extraction_tensor,
            self.extraction_layer,
        )?;
        let mean = self
            .input_channel_mean
            .into_iter()
            .map(CanonicalDecimal::new)
            .collect::<Result<Vec<_>, _>>()?;
        let std = self
            .input_channel_std
            .into_iter()
            .map(CanonicalDecimal::new)
            .collect::<Result<Vec<_>, _>>()?;
        let tensor = if self.version == 1 {
            CellEmbeddingTensorContract::rgb_he_raw(
                self.output_dimension,
                mean.try_into()
                    .map_err(|_| EmbeddingError::InvalidProvenanceJson)?,
                std.try_into()
                    .map_err(|_| EmbeddingError::InvalidProvenanceJson)?,
            )?
        } else {
            CellEmbeddingTensorContract::new(
                self.output_dimension,
                self.pooling,
                self.output_normalization.kind,
                self.input_channel_order,
                mean,
                std,
                self.modality,
                self.stain,
            )?
        };
        let execution = CellEmbeddingExecutionProvenance::new(
            parse_artifact_id(&self.preprocessing_artifact_id)?,
            parse_artifact_id(&self.run_config_artifact_id)?,
            parse_artifact_id(&self.environment_artifact_id)?,
            parse_artifact_id(&self.converter_artifact_id)?,
            self.converter_name,
            self.converter_version,
            self.importer_profile,
            self.importer_version,
        )?;
        let inputs = CellEmbeddingInputArtifacts::new(
            parse_artifact_id(&self.source_cells_artifact_id)?,
            parse_artifact_id(&self.source_vectors_artifact_id)?,
            parse_artifact_id(&self.expected_cells_artifact_id)?,
            parse_artifact_id(&self.identity_map_artifact_id)?,
            parse_artifact_id(&self.spatial_context_artifact_id)?,
            parse_artifact_id(&self.row_link_artifact_id)?,
        );
        CellEmbeddingProvenance::new(model, tensor, execution, inputs)
    }

    fn is_complete(&self) -> bool {
        self.version != 0
            && self.extraction_layer != 0
            && self.output_dimension != 0
            && !self.output_normalization.kind.is_empty()
            && ![
                self.format.as_str(),
                self.model_family.as_str(),
                self.model_version.as_str(),
                self.encoder_architecture.as_str(),
                self.checkpoint_artifact_id.as_str(),
                self.checkpoint_content_sha256.as_str(),
                self.source_snapshot_artifact_id.as_str(),
                self.license_record_artifact_id.as_str(),
                self.license_spdx.as_str(),
                self.citation.as_str(),
                self.extraction_tensor.as_str(),
                self.pooling.as_str(),
                self.output_dtype.as_str(),
                self.input_channel_order.as_str(),
                self.modality.as_str(),
                self.stain.as_str(),
                self.preprocessing_artifact_id.as_str(),
                self.run_config_artifact_id.as_str(),
                self.environment_artifact_id.as_str(),
                self.converter_artifact_id.as_str(),
                self.converter_name.as_str(),
                self.converter_version.as_str(),
                self.importer_profile.as_str(),
                self.importer_version.as_str(),
                self.source_cells_artifact_id.as_str(),
                self.source_vectors_artifact_id.as_str(),
                self.expected_cells_artifact_id.as_str(),
                self.identity_map_artifact_id.as_str(),
                self.spatial_context_artifact_id.as_str(),
                self.row_link_artifact_id.as_str(),
                self.missing_vector_policy.as_str(),
            ]
            .contains(&"")
            && !self.input_channel_mean.iter().any(String::is_empty)
            && !self.input_channel_std.iter().any(String::is_empty)
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireNone {
    kind: String,
}

fn parse_artifact_id(value: &str) -> Result<ArtifactId, EmbeddingError> {
    if !canonical_digest_text(value) {
        return Err(EmbeddingError::InvalidProvenanceJson);
    }
    ArtifactId::from_str(value).map_err(|_| EmbeddingError::InvalidProvenanceJson)
}

fn parse_digest(value: &str) -> Result<ContentDigest, EmbeddingError> {
    if !canonical_digest_text(value) {
        return Err(EmbeddingError::InvalidProvenanceJson);
    }
    ContentDigest::from_str(value).map_err(|_| EmbeddingError::InvalidProvenanceJson)
}

fn canonical_digest_text(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
