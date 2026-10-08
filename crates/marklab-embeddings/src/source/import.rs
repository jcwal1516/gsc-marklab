use std::fmt;

use marklab_data::CohortHierarchy;
use marklab_project::{ArtifactId, ArtifactRecord, ContentDigest};

use crate::{
    CellEmbeddingRowLink, CellEmbeddingTable, CellIdentityMap, ExpectedCellSet,
    VerifiedCellEmbeddingArtifactGraph,
};

use self::validation::{domain_error, import_error, require_record, require_schema};
use super::{
    CellVitCsvSummary, CellVitNpySummary, ImportFailure, SourceBundleBudgets, SourceBundleError,
};

mod orchestration;
mod validation;

#[cfg(test)]
mod tests;

pub use orchestration::{
    import_cellvit_he_bundle_bytes, import_cellvit_he_bundle_from_store,
    import_cellvit_he_bundle_readers,
};

const MAX_NPY_HEADER_RETAINED_BYTES: usize = 64 * 1024 + 12;

/// Exact artifact records bound to one CellViT H&E source import candidate.
#[derive(Clone, Eq, PartialEq)]
pub struct CellVitHeArtifactBindings {
    source_cells: ArtifactRecord,
    source_vectors: ArtifactRecord,
    expected_cells: ArtifactRecord,
    identity_map: ArtifactRecord,
    converter: ArtifactRecord,
}

impl CellVitHeArtifactBindings {
    /// Bind six exact source, identity, converter, and provenance records.
    pub fn from_records(
        source_cells: ArtifactRecord,
        source_vectors: ArtifactRecord,
        expected_cells: ArtifactRecord,
        identity_map: ArtifactRecord,
        converter: ArtifactRecord,
    ) -> Result<Self, SourceBundleError> {
        let mut identifiers = [
            source_cells.id(),
            source_vectors.id(),
            expected_cells.id(),
            identity_map.id(),
            converter.id(),
        ];
        identifiers.sort_unstable();
        if identifiers.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(import_error(ImportFailure::ArtifactBindingMismatch));
        }
        require_record(
            &source_cells,
            "marklab.cell_embedding_source_cells",
            Some("text/csv;profile=marklab-cellvit-he-bundle-v1"),
        )?;
        require_record(
            &source_vectors,
            "marklab.cell_embedding_source_npy",
            Some("application/x-npy;profile=marklab-cellvit-he-f4-v1"),
        )?;
        require_record(
            &expected_cells,
            "marklab.cell_embedding_expected_cells",
            Some("application/vnd.marklab.embedding-expected-cells.v1"),
        )?;
        require_record(
            &identity_map,
            "marklab.cell_embedding_identity_map",
            Some("application/vnd.marklab.embedding-identity-map.v1"),
        )?;
        require_schema(&converter, "marklab.converter_manifest")?;
        let mut expected_identity_dependencies = [source_cells.id(), expected_cells.id()];
        expected_identity_dependencies.sort_unstable();
        if identity_map.dependencies() != expected_identity_dependencies {
            return Err(import_error(ImportFailure::ArtifactBindingMismatch));
        }
        Ok(Self {
            source_cells,
            source_vectors,
            expected_cells,
            identity_map,
            converter,
        })
    }

    /// Source-cell CSV artifact.
    pub fn source_cells_artifact_id(&self) -> ArtifactId {
        self.source_cells.id()
    }

    /// Source-vector NPY artifact.
    pub fn source_vectors_artifact_id(&self) -> ArtifactId {
        self.source_vectors.id()
    }

    /// Canonical expected-cell-set artifact.
    pub fn expected_cells_artifact_id(&self) -> ArtifactId {
        self.expected_cells.id()
    }

    /// Explicit source-local identity-map artifact.
    pub fn identity_map_artifact_id(&self) -> ArtifactId {
        self.identity_map.id()
    }

    /// Reviewed converter-manifest artifact.
    pub fn converter_artifact_id(&self) -> ArtifactId {
        self.converter.id()
    }
}

impl fmt::Debug for CellVitHeArtifactBindings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellVitHeArtifactBindings")
            .field("source_cells_artifact_id", &self.source_cells.id())
            .field("source_vectors_artifact_id", &self.source_vectors.id())
            .field("expected_cells_artifact_id", &self.expected_cells.id())
            .field("identity_map_artifact_id", &self.identity_map.id())
            .field("converter_artifact_id", &self.converter.id())
            .finish()
    }
}

/// Borrowed domain inputs and explicit resource budgets for one promoted source import.
#[derive(Clone, Copy)]
pub struct CellVitHeImportRequest<'a> {
    expected: &'a ExpectedCellSet,
    identity_map: &'a CellIdentityMap,
    hierarchy: &'a CohortHierarchy,
    bindings: &'a CellVitHeArtifactBindings,
    budgets: SourceBundleBudgets,
}

impl<'a> CellVitHeImportRequest<'a> {
    /// Declare the exact expected set, identity map, hierarchy, artifact roles, and budgets.
    pub fn new(
        expected: &'a ExpectedCellSet,
        identity_map: &'a CellIdentityMap,
        hierarchy: &'a CohortHierarchy,
        bindings: &'a CellVitHeArtifactBindings,
        budgets: SourceBundleBudgets,
    ) -> Self {
        Self {
            expected,
            identity_map,
            hierarchy,
            bindings,
            budgets,
        }
    }
}

/// Canonical source values and row linkage awaiting row-link publication and graph validation.
pub struct CellVitHeImportCandidate {
    values: Vec<f32>,
    dimension: u32,
    row_link: CellEmbeddingRowLink,
    expected_cells_logical_digest: ContentDigest,
    maximum_retained_bytes: usize,
    npy_summary: CellVitNpySummary,
    csv_summary: CellVitCsvSummary,
}

impl fmt::Debug for CellVitHeImportCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellVitHeImportCandidate")
            .field("row_count", &self.row_count())
            .field("dimension", &self.dimension)
            .field("npy_summary", &self.npy_summary)
            .field("csv_summary", &self.csv_summary)
            .field("row_link_digest", &self.row_link.logical_digest())
            .finish()
    }
}

impl CellVitHeImportCandidate {
    /// Number of canonical present source rows.
    pub fn row_count(&self) -> usize {
        self.row_link.entries().len()
    }

    /// Fixed raw CellViT vector dimension.
    pub fn dimension(&self) -> u32 {
        self.dimension
    }

    /// One canonical cell-sorted vector before provenance-gated table finalization.
    pub fn canonical_vector(&self, row: usize) -> Option<&[f32]> {
        let dimension = self.dimension as usize;
        let start = row.checked_mul(dimension)?;
        self.values.get(start..start.checked_add(dimension)?)
    }

    /// All canonical cell-sorted contiguous source values.
    pub fn canonical_values(&self) -> &[f32] {
        &self.values
    }

    /// Candidate source-row correspondence to encode and publish before provenance.
    pub fn row_link(&self) -> &CellEmbeddingRowLink {
        &self.row_link
    }

    /// Aggregate-only exact NPY source facts.
    pub fn npy_summary(&self) -> CellVitNpySummary {
        self.npy_summary
    }

    /// Aggregate-only exact CSV source facts.
    pub fn csv_summary(&self) -> CellVitCsvSummary {
        self.csv_summary
    }

    /// Finalize a table only after the exact row-link/provenance graph is store-verified.
    pub fn finalize(
        self,
        expected: &ExpectedCellSet,
        graph: &VerifiedCellEmbeddingArtifactGraph,
    ) -> Result<ImportedCellVitHeBundle, SourceBundleError> {
        if expected.logical_digest() != self.expected_cells_logical_digest
            || graph.expected_cells_logical_digest != self.expected_cells_logical_digest
            || graph.row_link_logical_digest != self.row_link.logical_digest()
            || graph.source_cells_artifact_id != self.row_link.source_cells_artifact_id()
            || graph.source_vectors_artifact_id != self.row_link.source_vectors_artifact_id()
            || graph.expected_cells_artifact_id != self.row_link.expected_cells_artifact_id()
            || graph.identity_map_artifact_id != self.row_link.identity_map_artifact_id()
            || graph.converter_artifact_id != self.row_link.converter_artifact_id()
            || graph.output_dimension != self.dimension
        {
            return Err(import_error(ImportFailure::VerifiedGraphMismatch));
        }
        let table = CellEmbeddingTable::from_present_values(
            self.dimension,
            expected,
            self.row_link.expected_cells_artifact_id(),
            graph.provenance_artifact_id(),
            self.row_link.logical_digest(),
            self.values,
            self.maximum_retained_bytes,
        )
        .map_err(domain_error)?;
        Ok(ImportedCellVitHeBundle {
            table,
            row_link: self.row_link,
            npy_summary: self.npy_summary,
            csv_summary: self.csv_summary,
        })
    }
}

/// Provenance-gated canonical embedding table plus exact source-row correspondence.
pub struct ImportedCellVitHeBundle {
    table: CellEmbeddingTable,
    row_link: CellEmbeddingRowLink,
    npy_summary: CellVitNpySummary,
    csv_summary: CellVitCsvSummary,
}

impl fmt::Debug for ImportedCellVitHeBundle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportedCellVitHeBundle")
            .field("row_count", &self.table.row_count())
            .field("dimension", &self.table.dimension())
            .field("npy_summary", &self.npy_summary)
            .field("csv_summary", &self.csv_summary)
            .field("row_link_digest", &self.row_link.logical_digest())
            .finish()
    }
}

impl ImportedCellVitHeBundle {
    /// Canonical cell-sorted contiguous embedding table.
    pub fn table(&self) -> &CellEmbeddingTable {
        &self.table
    }

    /// Canonical cell-sorted source-row correspondence.
    pub fn row_link(&self) -> &CellEmbeddingRowLink {
        &self.row_link
    }

    /// Aggregate-only exact NPY source facts.
    pub fn npy_summary(&self) -> CellVitNpySummary {
        self.npy_summary
    }

    /// Aggregate-only exact CSV source facts.
    pub fn csv_summary(&self) -> CellVitCsvSummary {
        self.csv_summary
    }
}
