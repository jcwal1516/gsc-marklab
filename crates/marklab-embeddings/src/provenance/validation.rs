use std::convert::Infallible;

use marklab_project::{
    ArtifactCatalog, ArtifactId, ArtifactRecord, LocalArtifactStore, TableColumnType, TableFormat,
    TableScalarType, VerifiedReaderError,
};

use super::CellEmbeddingProvenance;
use crate::{CellEmbeddingRowLink, CellIdentityMap, EmbeddingSpatialContext, ExpectedCellSet};

mod error;
#[cfg(test)]
mod tests;

pub(crate) use error::artifact_availability_failure;
pub use error::{
    ArtifactAvailabilityFailure, CellEmbeddingArtifactRole, EmbeddingArtifactGraphError,
    VerifiedCellEmbeddingArtifactGraph,
};

impl CellEmbeddingProvenance {
    /// Validate exact catalog semantics, canonical payload bindings, and managed availability.
    ///
    /// This establishes the artifact graph boundary. Source and columnar payload parsers must
    /// separately validate their physical bytes before an embedding table can be promoted.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_artifact_graph(
        &self,
        provenance_artifact_id: ArtifactId,
        expected: &ExpectedCellSet,
        identity_map: &CellIdentityMap,
        spatial_context: &EmbeddingSpatialContext,
        row_link: &CellEmbeddingRowLink,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedCellEmbeddingArtifactGraph, EmbeddingArtifactGraphError> {
        self.validate_linkage(expected, identity_map, row_link)?;
        let roles = self.roles(provenance_artifact_id);
        for (role, id) in roles {
            require_schema(required_record(catalog, role, id)?, role)?;
        }
        self.validate_c04_records(
            provenance_artifact_id,
            expected,
            identity_map,
            spatial_context,
            row_link,
            catalog,
        )?;
        for (role, id) in roles {
            let record = required_record(catalog, role, id)?;
            store
                .with_verified_reader(record, |_reader| Ok::<(), Infallible>(()))
                .map_err(|error| match error {
                    VerifiedReaderError::Store(error) => EmbeddingArtifactGraphError::Unavailable {
                        role,
                        reason: artifact_availability_failure(&error),
                    },
                    VerifiedReaderError::Callback(error) => match error {},
                })?;
        }
        Ok(VerifiedCellEmbeddingArtifactGraph {
            provenance_artifact_id,
            dependency_count: 13,
            source_cells_artifact_id: self.inputs.source_cells_artifact_id,
            source_vectors_artifact_id: self.inputs.source_vectors_artifact_id,
            expected_cells_artifact_id: self.inputs.expected_cells_artifact_id,
            identity_map_artifact_id: self.inputs.identity_map_artifact_id,
            converter_artifact_id: self.execution.converter_artifact_id,
            row_link_artifact_id: self.inputs.row_link_artifact_id,
            expected_cells_logical_digest: expected.logical_digest(),
            row_link_logical_digest: row_link.logical_digest(),
            output_dimension: self.tensor.output_dimension(),
        })
    }

    fn validate_linkage(
        &self,
        expected: &ExpectedCellSet,
        identity_map: &CellIdentityMap,
        row_link: &CellEmbeddingRowLink,
    ) -> Result<(), EmbeddingArtifactGraphError> {
        if identity_map.source_cells_artifact_id() != self.inputs.source_cells_artifact_id
            || identity_map.expected_cells_artifact_id() != self.inputs.expected_cells_artifact_id
            || identity_map.expected_cells_logical_digest() != expected.logical_digest()
            || row_link.source_cells_artifact_id() != self.inputs.source_cells_artifact_id
            || row_link.source_vectors_artifact_id() != self.inputs.source_vectors_artifact_id
            || row_link.expected_cells_artifact_id() != self.inputs.expected_cells_artifact_id
            || row_link.identity_map_artifact_id() != self.inputs.identity_map_artifact_id
            || row_link.converter_artifact_id() != self.execution.converter_artifact_id
            || row_link.expected_cells_logical_digest() != expected.logical_digest()
            || row_link.entries().len() != expected.cells().len()
            || row_link
                .entries()
                .iter()
                .zip(expected.cells())
                .any(|(entry, cell)| entry.cell_id() != cell)
        {
            return Err(EmbeddingArtifactGraphError::LinkageMismatch);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_c04_records(
        &self,
        provenance_artifact_id: ArtifactId,
        expected: &ExpectedCellSet,
        identity_map: &CellIdentityMap,
        spatial_context: &EmbeddingSpatialContext,
        row_link: &CellEmbeddingRowLink,
        catalog: &ArtifactCatalog,
    ) -> Result<(), EmbeddingArtifactGraphError> {
        let provenance_record = required_record(
            catalog,
            CellEmbeddingArtifactRole::Provenance,
            provenance_artifact_id,
        )?;
        let checkpoint_record = required_record(
            catalog,
            CellEmbeddingArtifactRole::Checkpoint,
            self.model.checkpoint_artifact_id,
        )?;
        if checkpoint_record.content().digest() != self.model.checkpoint_content_sha256 {
            return Err(EmbeddingArtifactGraphError::CheckpointDigestMismatch);
        }

        let c04_non_tables = [
            (
                CellEmbeddingArtifactRole::Provenance,
                provenance_artifact_id,
                "application/vnd.marklab.embedding-provenance.v1+json",
            ),
            (
                CellEmbeddingArtifactRole::SourceCells,
                self.inputs.source_cells_artifact_id,
                "text/csv;profile=marklab-cellvit-he-bundle-v1",
            ),
            (
                CellEmbeddingArtifactRole::SourceVectors,
                self.inputs.source_vectors_artifact_id,
                "application/x-npy;profile=marklab-cellvit-he-f4-v1",
            ),
            (
                CellEmbeddingArtifactRole::ExpectedCells,
                self.inputs.expected_cells_artifact_id,
                "application/vnd.marklab.embedding-expected-cells.v1",
            ),
            (
                CellEmbeddingArtifactRole::IdentityMap,
                self.inputs.identity_map_artifact_id,
                "application/vnd.marklab.embedding-identity-map.v1",
            ),
            (
                CellEmbeddingArtifactRole::SpatialContext,
                self.inputs.spatial_context_artifact_id,
                "application/vnd.marklab.embedding-spatial-context.v1+json",
            ),
        ];
        for (role, id, kind) in c04_non_tables {
            let record = required_record(catalog, role, id)?;
            require_c04_common(record, role)?;
            if record.content().kind() != kind {
                return Err(EmbeddingArtifactGraphError::ContentKindMismatch { role });
            }
            if record.table().is_some() {
                return Err(EmbeddingArtifactGraphError::UnexpectedTableManifest { role });
            }
        }
        let row_link_record = required_record(
            catalog,
            CellEmbeddingArtifactRole::RowLink,
            self.inputs.row_link_artifact_id,
        )?;
        require_c04_common(row_link_record, CellEmbeddingArtifactRole::RowLink)?;
        require_row_link_manifest(row_link_record, row_link.row_count())?;

        require_dependencies(
            provenance_record,
            CellEmbeddingArtifactRole::Provenance,
            &self.direct_dependencies(),
        )?;
        let mut identity_dependencies = [
            self.inputs.source_cells_artifact_id,
            self.inputs.expected_cells_artifact_id,
        ];
        identity_dependencies.sort_unstable();
        let identity_record = required_record(
            catalog,
            CellEmbeddingArtifactRole::IdentityMap,
            self.inputs.identity_map_artifact_id,
        )?;
        require_dependencies(
            identity_record,
            CellEmbeddingArtifactRole::IdentityMap,
            &identity_dependencies,
        )?;
        require_dependencies(
            row_link_record,
            CellEmbeddingArtifactRole::RowLink,
            &row_link.direct_dependencies(),
        )?;

        let provenance_bytes = self.to_canonical_json().map_err(|_| {
            EmbeddingArtifactGraphError::PayloadIdentityMismatch {
                role: CellEmbeddingArtifactRole::Provenance,
            }
        })?;
        require_payload_identity(
            provenance_record,
            CellEmbeddingArtifactRole::Provenance,
            &provenance_bytes,
        )?;
        let expected_record = required_record(
            catalog,
            CellEmbeddingArtifactRole::ExpectedCells,
            self.inputs.expected_cells_artifact_id,
        )?;
        let expected_bytes = expected.to_bytes().map_err(|_| {
            EmbeddingArtifactGraphError::PayloadIdentityMismatch {
                role: CellEmbeddingArtifactRole::ExpectedCells,
            }
        })?;
        require_payload_identity(
            expected_record,
            CellEmbeddingArtifactRole::ExpectedCells,
            &expected_bytes,
        )?;
        let identity_bytes = identity_map.to_bytes().map_err(|_| {
            EmbeddingArtifactGraphError::PayloadIdentityMismatch {
                role: CellEmbeddingArtifactRole::IdentityMap,
            }
        })?;
        require_payload_identity(
            identity_record,
            CellEmbeddingArtifactRole::IdentityMap,
            &identity_bytes,
        )?;
        let context_record = required_record(
            catalog,
            CellEmbeddingArtifactRole::SpatialContext,
            self.inputs.spatial_context_artifact_id,
        )?;
        let context_bytes = spatial_context.to_canonical_json().map_err(|_| {
            EmbeddingArtifactGraphError::PayloadIdentityMismatch {
                role: CellEmbeddingArtifactRole::SpatialContext,
            }
        })?;
        require_payload_identity(
            context_record,
            CellEmbeddingArtifactRole::SpatialContext,
            &context_bytes,
        )
    }

    fn roles(
        &self,
        provenance_artifact_id: ArtifactId,
    ) -> [(CellEmbeddingArtifactRole, ArtifactId); 14] {
        [
            (
                CellEmbeddingArtifactRole::Provenance,
                provenance_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::Checkpoint,
                self.model.checkpoint_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::SourceSnapshot,
                self.model.source_snapshot_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::LicenseRecord,
                self.model.license_record_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::Preprocessing,
                self.execution.preprocessing_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::RunConfig,
                self.execution.run_config_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::Environment,
                self.execution.environment_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::Converter,
                self.execution.converter_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::SourceCells,
                self.inputs.source_cells_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::SourceVectors,
                self.inputs.source_vectors_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::ExpectedCells,
                self.inputs.expected_cells_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::IdentityMap,
                self.inputs.identity_map_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::SpatialContext,
                self.inputs.spatial_context_artifact_id,
            ),
            (
                CellEmbeddingArtifactRole::RowLink,
                self.inputs.row_link_artifact_id,
            ),
        ]
    }
}

fn required_record(
    catalog: &ArtifactCatalog,
    role: CellEmbeddingArtifactRole,
    id: ArtifactId,
) -> Result<&ArtifactRecord, EmbeddingArtifactGraphError> {
    catalog
        .get(id)
        .ok_or(EmbeddingArtifactGraphError::MissingRecord { role })
}

fn require_schema(
    record: &ArtifactRecord,
    role: CellEmbeddingArtifactRole,
) -> Result<(), EmbeddingArtifactGraphError> {
    if record.schema().id() != schema_id(role) || record.schema().version() != 1 {
        return Err(EmbeddingArtifactGraphError::SchemaMismatch { role });
    }
    Ok(())
}

fn schema_id(role: CellEmbeddingArtifactRole) -> &'static str {
    match role {
        CellEmbeddingArtifactRole::Provenance => "marklab.cell_embedding_provenance",
        CellEmbeddingArtifactRole::Checkpoint => "marklab.model_checkpoint",
        CellEmbeddingArtifactRole::SourceSnapshot => "marklab.source_snapshot",
        CellEmbeddingArtifactRole::LicenseRecord => "marklab.license_record",
        CellEmbeddingArtifactRole::Preprocessing => "marklab.embedding_preprocessing",
        CellEmbeddingArtifactRole::RunConfig => "marklab.embedding_run_config",
        CellEmbeddingArtifactRole::Environment => "marklab.execution_environment",
        CellEmbeddingArtifactRole::Converter => "marklab.converter_manifest",
        CellEmbeddingArtifactRole::SourceCells => "marklab.cell_embedding_source_cells",
        CellEmbeddingArtifactRole::SourceVectors => "marklab.cell_embedding_source_npy",
        CellEmbeddingArtifactRole::ExpectedCells => "marklab.cell_embedding_expected_cells",
        CellEmbeddingArtifactRole::IdentityMap => "marklab.cell_embedding_identity_map",
        CellEmbeddingArtifactRole::SpatialContext => "marklab.cell_embedding_spatial_context",
        CellEmbeddingArtifactRole::RowLink => "marklab.cell_embedding_row_link",
    }
}

fn require_c04_common(
    record: &ArtifactRecord,
    role: CellEmbeddingArtifactRole,
) -> Result<(), EmbeddingArtifactGraphError> {
    if !record.semantic_metadata().is_empty() {
        return Err(EmbeddingArtifactGraphError::SemanticMetadataMismatch { role });
    }
    Ok(())
}

fn require_dependencies(
    record: &ArtifactRecord,
    role: CellEmbeddingArtifactRole,
    expected: &[ArtifactId],
) -> Result<(), EmbeddingArtifactGraphError> {
    if record.dependencies() != expected {
        return Err(EmbeddingArtifactGraphError::DependencyMismatch { role });
    }
    Ok(())
}

fn require_payload_identity(
    record: &ArtifactRecord,
    role: CellEmbeddingArtifactRole,
    bytes: &[u8],
) -> Result<(), EmbeddingArtifactGraphError> {
    let byte_len = u64::try_from(bytes.len())
        .map_err(|_| EmbeddingArtifactGraphError::PayloadIdentityMismatch { role })?;
    if record.content().digest() != marklab_project::ContentDigest::from_bytes(bytes)
        || record.content().byte_len() != byte_len
    {
        return Err(EmbeddingArtifactGraphError::PayloadIdentityMismatch { role });
    }
    Ok(())
}

fn require_row_link_manifest(
    record: &ArtifactRecord,
    row_count: u64,
) -> Result<(), EmbeddingArtifactGraphError> {
    let (format, encoding) = match record.content().kind() {
        "application/vnd.marklab.embedding-row-link.v1+arrow" => (
            TableFormat::ArrowIpcFile,
            "marklab.arrow-ipc.embedding-row-link.v1",
        ),
        "application/vnd.marklab.embedding-row-link.v1+parquet" => (
            TableFormat::ParquetFile,
            "marklab.parquet.embedding-row-link.v1",
        ),
        _ => {
            return Err(EmbeddingArtifactGraphError::ContentKindMismatch {
                role: CellEmbeddingArtifactRole::RowLink,
            });
        }
    };
    let Some(table) = record.table() else {
        return Err(EmbeddingArtifactGraphError::RowLinkManifestMismatch);
    };
    let columns = table.columns();
    let valid_columns = columns.len() == 3
        && columns[0].name() == "cell_id"
        && columns[0].column_type() == &TableColumnType::Scalar(TableScalarType::Utf8)
        && !columns[0].nullable()
        && columns[1].name() == "source_cell_row"
        && columns[1].column_type() == &TableColumnType::Scalar(TableScalarType::U64)
        && !columns[1].nullable()
        && columns[2].name() == "source_embedding_row"
        && columns[2].column_type() == &TableColumnType::Scalar(TableScalarType::U64)
        && columns[2].nullable();
    if table.format() != format
        || table.encoding_version() != encoding
        || table.row_count() != row_count
        || !valid_columns
        || table.primary_key().len() != 1
        || table.primary_key()[0] != "cell_id"
    {
        return Err(EmbeddingArtifactGraphError::RowLinkManifestMismatch);
    }
    Ok(())
}
