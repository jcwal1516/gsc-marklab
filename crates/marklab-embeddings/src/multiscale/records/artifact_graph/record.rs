use marklab_project::{ArtifactCatalog, ArtifactId, ArtifactRecord};

use crate::multiscale::physical::{record_matches, SpatialArtifactRole};

use super::{MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole};

pub(super) fn required_record(
    catalog: &ArtifactCatalog,
    role: MultiscaleEmbeddingArtifactRole,
    id: ArtifactId,
) -> Result<&ArtifactRecord, MultiscaleEmbeddingArtifactGraphError> {
    catalog
        .get(id)
        .ok_or(MultiscaleEmbeddingArtifactGraphError::MissingRecord { role })
}

pub(super) fn require_record_profile(
    record: &ArtifactRecord,
    role: MultiscaleEmbeddingArtifactRole,
    row_count: Option<u64>,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    if record.schema().id() != schema_id(role) || record.schema().version() != 1 {
        return Err(MultiscaleEmbeddingArtifactGraphError::SchemaMismatch { role });
    }
    if !record.semantic_metadata().is_empty() {
        return Err(MultiscaleEmbeddingArtifactGraphError::SemanticMetadataMismatch { role });
    }
    match role {
        MultiscaleEmbeddingArtifactRole::PatchFootprints => {
            let rows = row_count
                .ok_or(MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch { role })?;
            require_footprint_manifest(record, rows)
        }
        MultiscaleEmbeddingArtifactRole::PatchOverlapGraph => {
            let rows = row_count
                .ok_or(MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch { role })?;
            require_overlap_manifest(record, rows)
        }
        _ => {
            if record.content().kind() != content_kind(role) {
                return Err(MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch { role });
            }
            if record.table().is_some() {
                return Err(
                    MultiscaleEmbeddingArtifactGraphError::UnexpectedTableManifest { role },
                );
            }
            Ok(())
        }
    }
}

pub(super) fn require_dependencies(
    record: &ArtifactRecord,
    role: MultiscaleEmbeddingArtifactRole,
    expected: &[ArtifactId],
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    if record.dependencies() != expected {
        return Err(MultiscaleEmbeddingArtifactGraphError::DependencyMismatch { role });
    }
    Ok(())
}

fn schema_id(role: MultiscaleEmbeddingArtifactRole) -> &'static str {
    match role {
        MultiscaleEmbeddingArtifactRole::Provenance => "marklab.multiscale_embedding_provenance",
        MultiscaleEmbeddingArtifactRole::Checkpoint => "marklab.model_checkpoint",
        MultiscaleEmbeddingArtifactRole::SourceSnapshot => "marklab.source_snapshot",
        MultiscaleEmbeddingArtifactRole::LicenseRecord => "marklab.license_record",
        MultiscaleEmbeddingArtifactRole::InputNormalization => {
            "marklab.patch_embedding_input_normalization"
        }
        MultiscaleEmbeddingArtifactRole::Preprocessing => "marklab.embedding_preprocessing",
        MultiscaleEmbeddingArtifactRole::RunConfig => "marklab.embedding_run_config",
        MultiscaleEmbeddingArtifactRole::Environment => "marklab.execution_environment",
        MultiscaleEmbeddingArtifactRole::Converter => "marklab.converter_manifest",
        MultiscaleEmbeddingArtifactRole::SourceEntities => "marklab.patch_source_entity_set",
        MultiscaleEmbeddingArtifactRole::SourceVectors => "marklab.patch_embedding_source_vectors",
        MultiscaleEmbeddingArtifactRole::ExpectedPatches => "marklab.expected_patch_set",
        MultiscaleEmbeddingArtifactRole::IdentityMap => "marklab.patch_identity_map",
        MultiscaleEmbeddingArtifactRole::SourceRowLink => "marklab.patch_embedding_source_row_link",
        MultiscaleEmbeddingArtifactRole::PatchContext => "marklab.patch_embedding_context",
        MultiscaleEmbeddingArtifactRole::PatchFootprints => "marklab.patch_footprint_table",
        MultiscaleEmbeddingArtifactRole::PatchOverlapGraph => "marklab.patch_overlap_edge_table",
        MultiscaleEmbeddingArtifactRole::PatchSupport => "marklab.multiscale_embedding_support",
        MultiscaleEmbeddingArtifactRole::SourcePatchTable => "marklab.patch_embedding_table",
        MultiscaleEmbeddingArtifactRole::PatchRegionLink => "marklab.patch_region_link",
        MultiscaleEmbeddingArtifactRole::ExpectedRegions => "marklab.expected_region_set",
        MultiscaleEmbeddingArtifactRole::RegionSupport => "marklab.multiscale_embedding_support",
        MultiscaleEmbeddingArtifactRole::Derivation => "marklab.multiscale_embedding_derivation",
    }
}

fn content_kind(role: MultiscaleEmbeddingArtifactRole) -> &'static str {
    match role {
        MultiscaleEmbeddingArtifactRole::Provenance => {
            "application/vnd.marklab.multiscale-embedding-provenance.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::Checkpoint
        | MultiscaleEmbeddingArtifactRole::SourceSnapshot => "application/octet-stream",
        MultiscaleEmbeddingArtifactRole::LicenseRecord => "text/plain",
        MultiscaleEmbeddingArtifactRole::InputNormalization => {
            "application/vnd.marklab.patch-embedding-input-normalization.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::Preprocessing
        | MultiscaleEmbeddingArtifactRole::RunConfig
        | MultiscaleEmbeddingArtifactRole::Environment
        | MultiscaleEmbeddingArtifactRole::Converter => "application/json",
        MultiscaleEmbeddingArtifactRole::SourceEntities => {
            "application/vnd.marklab.patch-source-entity-set.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::SourceVectors => {
            "application/vnd.marklab.patch-source-vectors.v1+binary"
        }
        MultiscaleEmbeddingArtifactRole::ExpectedPatches => {
            "application/vnd.marklab.expected-patch-set.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::IdentityMap => {
            "application/vnd.marklab.patch-identity-map.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::SourceRowLink => {
            "application/vnd.marklab.patch-embedding-source-row-link.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::PatchContext => {
            "application/vnd.marklab.patch-embedding-context.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::PatchSupport => {
            "application/vnd.marklab.multiscale-embedding-support.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::ExpectedRegions => {
            "application/vnd.marklab.expected-region-set.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::RegionSupport => {
            "application/vnd.marklab.multiscale-embedding-support.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::Derivation => {
            "application/vnd.marklab.multiscale-embedding-derivation.v1+json"
        }
        MultiscaleEmbeddingArtifactRole::PatchFootprints
        | MultiscaleEmbeddingArtifactRole::PatchOverlapGraph
        | MultiscaleEmbeddingArtifactRole::SourcePatchTable
        | MultiscaleEmbeddingArtifactRole::PatchRegionLink => "",
    }
}

fn require_footprint_manifest(
    record: &ArtifactRecord,
    row_count: u64,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    let role = MultiscaleEmbeddingArtifactRole::PatchFootprints;
    if !matches!(
        record.content().kind(),
        "application/vnd.marklab.patch-footprint-table.v1+arrow"
            | "application/vnd.marklab.patch-footprint-table.v1+parquet"
    ) {
        return Err(MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch { role });
    }
    if !record_matches(record, SpatialArtifactRole::Footprint, row_count) {
        return Err(MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch { role });
    }
    Ok(())
}

fn require_overlap_manifest(
    record: &ArtifactRecord,
    row_count: u64,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    let role = MultiscaleEmbeddingArtifactRole::PatchOverlapGraph;
    if !matches!(
        record.content().kind(),
        "application/vnd.marklab.patch-overlap-edge-table.v1+arrow"
            | "application/vnd.marklab.patch-overlap-edge-table.v1+parquet"
    ) {
        return Err(MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch { role });
    }
    if !record_matches(record, SpatialArtifactRole::Overlap, row_count) {
        return Err(MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch { role });
    }
    Ok(())
}
