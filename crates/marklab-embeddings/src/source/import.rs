use std::{fmt, io::Cursor};

use marklab_data::{CellId, CohortHierarchy, HierarchyId};
use marklab_project::{
    ArtifactId, ArtifactReadSeek, ArtifactRecord, ContentDigest, LocalArtifactStore,
    VerifiedReaderError,
};

use crate::{
    digest::canonical_positive_zero, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry,
    CellEmbeddingTable, CellIdentityMap, EmbeddingError, EmbeddingStatus, ExpectedCellSet,
    VerifiedCellEmbeddingArtifactGraph,
};

use super::{
    csv::{parse_reader as parse_csv_reader, ParsedCsv},
    npy::{
        reader_shape as npy_reader_shape, visit_reader as visit_npy_reader, NPY_STREAM_BUFFER_BYTES,
    },
    CellVitCsvSummary, CellVitNpySummary, ImportFailure, SourceBundleBudgets, SourceBundleError,
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

/// Import complete borrowed NPY/CSV bytes through the same bounded seekable-reader path.
pub fn import_cellvit_he_bundle_bytes(
    npy_bytes: &[u8],
    csv_bytes: &[u8],
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    let mut npy_reader = Cursor::new(npy_bytes);
    let mut csv_reader = Cursor::new(csv_bytes);
    import_cellvit_he_bundle_readers(&mut npy_reader, &mut csv_reader, request)
}

/// Import bounded seekable NPY/CSV readers into one canonical allocation.
pub fn import_cellvit_he_bundle_readers(
    npy_reader: &mut dyn ArtifactReadSeek,
    csv_reader: &mut dyn ArtifactReadSeek,
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    validate_request(request)?;
    let mut parsed = parse_csv_reader(csv_reader, request.budgets)?;
    require_source_identity(
        &request.bindings.source_cells,
        parsed.summary.content_digest(),
        parsed.summary.encoded_byte_len(),
    )?;
    bind_identity_rows(&mut parsed, request)?;
    finish_import(npy_reader, parsed, request)
}

/// Import bound managed source records through post-verified store readers.
pub fn import_cellvit_he_bundle_from_store(
    store: &LocalArtifactStore,
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    validate_request(request)?;
    let mut parsed = store
        .with_verified_reader(&request.bindings.source_cells, |csv_reader| {
            let parsed = parse_csv_reader(csv_reader, request.budgets)?;
            require_source_identity(
                &request.bindings.source_cells,
                parsed.summary.content_digest(),
                parsed.summary.encoded_byte_len(),
            )?;
            Ok(parsed)
        })
        .map_err(verified_reader_error)?;
    bind_identity_rows(&mut parsed, request)?;
    store
        .with_verified_reader(&request.bindings.source_vectors, |npy_reader| {
            finish_import(npy_reader, parsed, request)
        })
        .map_err(verified_reader_error)
}

fn finish_import(
    npy_reader: &mut dyn ArtifactReadSeek,
    mut parsed: ParsedCsv,
    request: CellVitHeImportRequest<'_>,
) -> Result<CellVitHeImportCandidate, SourceBundleError> {
    let shape_peak = parsed
        .retained_bytes
        .checked_add(MAX_NPY_HEADER_RETAINED_BYTES)
        .ok_or(SourceBundleError::SizeOverflow)?;
    enforce_retained(shape_peak, request.budgets)?;
    let (npy_rows, dimension) = npy_reader_shape(npy_reader, request.budgets)?;
    if npy_rows != parsed.summary.row_count() {
        return Err(SourceBundleError::RowCountMismatch {
            npy: npy_rows,
            csv: parsed.summary.row_count(),
        });
    }

    let component_count = component_count(npy_rows, dimension)?;
    let required = peak_retained_bytes(&parsed, request.expected, component_count)?;
    enforce_retained(required, request.budgets)?;
    let value_bytes = component_count
        .checked_mul(size_of::<f32>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(component_count)
        .map_err(|_| SourceBundleError::AllocationFailed {
            requested: value_bytes,
        })?;
    values.resize(component_count, 0.0);

    let dimension_usize =
        usize::try_from(dimension).map_err(|_| SourceBundleError::SizeOverflow)?;
    let npy_summary = visit_npy_reader(npy_reader, request.budgets, &mut |row, column, value| {
        let source_row = usize::try_from(row).map_err(|_| SourceBundleError::SizeOverflow)?;
        let canonical_row = parsed
            .rows
            .get(source_row)
            .ok_or(SourceBundleError::SizeOverflow)?
            .canonical_row;
        let index = canonical_row
            .checked_mul(dimension_usize)
            .and_then(|offset| offset.checked_add(column as usize))
            .ok_or(SourceBundleError::SizeOverflow)?;
        *values
            .get_mut(index)
            .ok_or(SourceBundleError::SizeOverflow)? = canonical_positive_zero(value);
        Ok(())
    })?;
    require_source_identity(
        &request.bindings.source_vectors,
        npy_summary.content_digest(),
        npy_summary.encoded_byte_len(),
    )?;

    let csv_summary = parsed.summary;
    parsed.rows.sort_unstable_by_key(|row| row.canonical_row);
    let mut row_entries = Vec::new();
    row_entries
        .try_reserve_exact(parsed.rows.len())
        .map_err(|_| SourceBundleError::AllocationFailed {
            requested: parsed
                .rows
                .len()
                .saturating_mul(size_of::<CellEmbeddingRowLinkEntry>()),
        })?;
    for (canonical_row, source_row) in parsed.rows.into_iter().enumerate() {
        if source_row.canonical_row != canonical_row {
            return Err(import_error(ImportFailure::IdentityMapMismatch));
        }
        let cell_id = request
            .expected
            .cells()
            .get(canonical_row)
            .ok_or_else(|| import_error(ImportFailure::IdentityMapMismatch))?
            .clone();
        row_entries.push(CellEmbeddingRowLinkEntry::present(
            cell_id,
            source_row.native_row,
            source_row.embedding_row,
        ));
    }

    let bindings = request.bindings;
    let row_link = CellEmbeddingRowLink::new(
        bindings.source_cells_artifact_id(),
        bindings.source_vectors_artifact_id(),
        bindings.expected_cells_artifact_id(),
        bindings.identity_map_artifact_id(),
        bindings.converter_artifact_id(),
        request.expected,
        request.hierarchy,
        row_entries,
        request.budgets.maximum_retained_bytes(),
    )
    .map_err(domain_error)?;
    Ok(CellVitHeImportCandidate {
        values,
        dimension,
        row_link,
        expected_cells_logical_digest: request.expected.logical_digest(),
        maximum_retained_bytes: request.budgets.maximum_retained_bytes(),
        npy_summary,
        csv_summary,
    })
}

fn validate_request(request: CellVitHeImportRequest<'_>) -> Result<(), SourceBundleError> {
    if request.expected.cells().is_empty() {
        return Err(import_error(ImportFailure::EmptyExpectedCells));
    }
    let bindings = request.bindings;
    if request.identity_map.source_cells_artifact_id() != bindings.source_cells_artifact_id()
        || request.identity_map.expected_cells_artifact_id()
            != bindings.expected_cells_artifact_id()
    {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    if request.identity_map.expected_cells_logical_digest() != request.expected.logical_digest()
        || request.identity_map.entries().len() != request.expected.cells().len()
    {
        return Err(import_error(ImportFailure::IdentityMapMismatch));
    }
    require_domain_payload(
        &bindings.expected_cells,
        request.expected.encoded_byte_len(),
        request.budgets,
        || request.expected.to_bytes(),
    )?;
    require_domain_payload(
        &bindings.identity_map,
        request.identity_map.encoded_byte_len(),
        request.budgets,
        || request.identity_map.to_bytes(),
    )?;
    if request.expected.cells().iter().any(|cell_id| {
        request
            .hierarchy
            .node(&HierarchyId::from(cell_id.clone()))
            .is_none()
    }) {
        return Err(import_error(ImportFailure::HierarchyMismatch));
    }
    Ok(())
}

fn require_record(
    record: &ArtifactRecord,
    schema_id: &str,
    content_kind: Option<&str>,
) -> Result<(), SourceBundleError> {
    if record.schema().id() != schema_id
        || record.schema().version() != 1
        || content_kind.is_some_and(|kind| record.content().kind() != kind)
        || record.table().is_some()
        || !record.semantic_metadata().is_empty()
    {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

fn require_schema(record: &ArtifactRecord, schema_id: &str) -> Result<(), SourceBundleError> {
    if record.schema().id() != schema_id || record.schema().version() != 1 {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

fn require_domain_payload<F>(
    record: &ArtifactRecord,
    encoded_byte_len: Result<usize, EmbeddingError>,
    budgets: SourceBundleBudgets,
    encode: F,
) -> Result<(), SourceBundleError>
where
    F: FnOnce() -> Result<Vec<u8>, EmbeddingError>,
{
    let encoded_byte_len = encoded_byte_len.map_err(domain_error)?;
    if u64::try_from(encoded_byte_len).map_err(|_| SourceBundleError::SizeOverflow)?
        != record.content().byte_len()
    {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    enforce_retained(encoded_byte_len, budgets)?;
    let bytes = encode().map_err(domain_error)?;
    if ContentDigest::from_bytes(&bytes) != record.content().digest() {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

fn require_source_identity(
    record: &ArtifactRecord,
    digest: ContentDigest,
    byte_len: u64,
) -> Result<(), SourceBundleError> {
    if record.content().digest() != digest || record.content().byte_len() != byte_len {
        return Err(import_error(ImportFailure::ArtifactBindingMismatch));
    }
    Ok(())
}

fn verified_reader_error(error: VerifiedReaderError<SourceBundleError>) -> SourceBundleError {
    match error {
        VerifiedReaderError::Store(_) => import_error(ImportFailure::ArtifactUnavailable),
        VerifiedReaderError::Callback(error) => error,
    }
}

fn bind_identity_rows(
    parsed: &mut ParsedCsv,
    request: CellVitHeImportRequest<'_>,
) -> Result<(), SourceBundleError> {
    if parsed.rows.len() != request.identity_map.entries().len() {
        return Err(import_error(ImportFailure::IdentityMapMismatch));
    }
    parsed
        .rows
        .sort_unstable_by(|left, right| left.source_cell_id.cmp(&right.source_cell_id));
    for (source_row, identity) in parsed.rows.iter_mut().zip(request.identity_map.entries()) {
        if source_row.source_cell_id != identity.source_cell_id() {
            return Err(import_error(ImportFailure::IdentityMapMismatch));
        }
        source_row.canonical_row = request
            .expected
            .cells()
            .binary_search(identity.cell_id())
            .map_err(|_| import_error(ImportFailure::IdentityMapMismatch))?;
    }
    parsed.rows.sort_unstable_by_key(|row| row.native_row);
    Ok(())
}

fn component_count(rows: u64, dimension: u32) -> Result<usize, SourceBundleError> {
    rows.checked_mul(u64::from(dimension))
        .ok_or(SourceBundleError::SizeOverflow)
        .and_then(|count| usize::try_from(count).map_err(|_| SourceBundleError::SizeOverflow))
}

fn peak_retained_bytes(
    parsed: &ParsedCsv,
    expected: &ExpectedCellSet,
    component_count: usize,
) -> Result<usize, SourceBundleError> {
    let value_bytes = component_count
        .checked_mul(size_of::<f32>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(cell.as_str().len())
            .ok_or(SourceBundleError::SizeOverflow)
    })?;
    let table_rows = expected
        .cells()
        .len()
        .checked_mul(size_of::<CellId>() + size_of::<EmbeddingStatus>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    let row_links = expected
        .cells()
        .len()
        .checked_mul(size_of::<CellEmbeddingRowLinkEntry>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    parsed
        .retained_bytes
        .checked_add(MAX_NPY_HEADER_RETAINED_BYTES)
        .and_then(|value| value.checked_add(NPY_STREAM_BUFFER_BYTES))
        .and_then(|value| value.checked_add(value_bytes))
        .and_then(|value| value.checked_add(table_rows))
        .and_then(|value| value.checked_add(row_links))
        .and_then(|value| value.checked_add(identifier_bytes.checked_mul(2)?))
        .ok_or(SourceBundleError::SizeOverflow)
}

fn enforce_retained(
    required: usize,
    budgets: SourceBundleBudgets,
) -> Result<(), SourceBundleError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(SourceBundleError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

fn import_error(reason: ImportFailure) -> SourceBundleError {
    SourceBundleError::Import { reason }
}

fn domain_error(error: EmbeddingError) -> SourceBundleError {
    match error {
        EmbeddingError::SizeOverflow => SourceBundleError::SizeOverflow,
        EmbeddingError::AllocationFailed { requested } => {
            SourceBundleError::AllocationFailed { requested }
        }
        EmbeddingError::RetainedByteBudgetExceeded { required, maximum } => {
            SourceBundleError::RetainedByteBudgetExceeded { required, maximum }
        }
        EmbeddingError::RowLinkCellMissingFromHierarchy => {
            import_error(ImportFailure::HierarchyMismatch)
        }
        _ => import_error(ImportFailure::DomainConstruction),
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, str::FromStr};

    use marklab_data::{HierarchyNode, PatientId, ReplicationRole, SlideId};

    use super::*;

    fn artifact_id(label: &[u8]) -> ArtifactId {
        ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
    }

    fn fixture() -> (
        ExpectedCellSet,
        CellVitHeImportCandidate,
        VerifiedCellEmbeddingArtifactGraph,
    ) {
        let cell = CellId::new("cell-a").expect("cell");
        let expected = ExpectedCellSet::new("all.v1", vec![cell.clone()]).expect("expected");
        let patient = HierarchyId::from(PatientId::new("patient").expect("patient"));
        let slide = HierarchyId::from(SlideId::new("slide").expect("slide"));
        let hierarchy = CohortHierarchy::new(
            vec![
                HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
                HierarchyNode::new(
                    slide.clone(),
                    None,
                    ReplicationRole::TechnicalReplicate {
                        biological_source: patient,
                    },
                ),
                HierarchyNode::new(
                    HierarchyId::from(cell.clone()),
                    Some(slide),
                    ReplicationRole::Structural,
                ),
            ],
            Vec::new(),
        )
        .expect("hierarchy");
        let source_cells_artifact_id = artifact_id(b"source-cells");
        let source_vectors_artifact_id = artifact_id(b"source-vectors");
        let expected_cells_artifact_id = artifact_id(b"expected-cells");
        let identity_map_artifact_id = artifact_id(b"identity-map");
        let converter_artifact_id = artifact_id(b"converter");
        let provenance_artifact_id = artifact_id(b"provenance");
        let row_link = CellEmbeddingRowLink::new(
            source_cells_artifact_id,
            source_vectors_artifact_id,
            expected_cells_artifact_id,
            identity_map_artifact_id,
            converter_artifact_id,
            &expected,
            &hierarchy,
            vec![CellEmbeddingRowLinkEntry::present(cell, 0, 0)],
            1024 * 1024,
        )
        .expect("row link");
        let npy_bytes = npy_bytes();
        let csv_bytes = csv_bytes();
        let budgets = SourceBundleBudgets::new(
            npy_bytes.len() as u64,
            csv_bytes.len() as u64,
            64 * 1024,
            1024 * 1024,
            1_280 * 4,
        );
        let mut npy_reader = Cursor::new(npy_bytes);
        let npy_summary = CellVitNpySummary::from_reader(&mut npy_reader, budgets).expect("NPY");
        let csv_summary = CellVitCsvSummary::from_bytes(&csv_bytes, budgets).expect("CSV");
        let graph = VerifiedCellEmbeddingArtifactGraph {
            provenance_artifact_id,
            dependency_count: 13,
            source_cells_artifact_id,
            source_vectors_artifact_id,
            expected_cells_artifact_id,
            identity_map_artifact_id,
            converter_artifact_id,
            expected_cells_logical_digest: expected.logical_digest(),
            row_link_logical_digest: row_link.logical_digest(),
        };
        let candidate = CellVitHeImportCandidate {
            values: vec![0.0; 1_280],
            dimension: 1_280,
            row_link,
            expected_cells_logical_digest: expected.logical_digest(),
            maximum_retained_bytes: 1024 * 1024,
            npy_summary,
            csv_summary,
        };
        (expected, candidate, graph)
    }

    fn npy_bytes() -> Vec<u8> {
        let dictionary = "{'descr': '<f4', 'fortran_order': False, 'shape': (1, 1280), }";
        let padding = (16 - ((10 + dictionary.len() + 1) % 16)) % 16;
        let mut header = dictionary.as_bytes().to_vec();
        header.resize(header.len() + padding, b' ');
        header.push(b'\n');
        let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
        bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.resize(bytes.len() + 1_280 * 4, 0);
        bytes
    }

    fn csv_bytes() -> Vec<u8> {
        concat!(
            "cell_id,case_id,specimen_id,timepoint,fragment_id,roi_id,native_row,",
            "embedding_row,x_px,y_px,x_um,y_um,cell_type_id,cell_type_label,",
            "type_probability,nucleus_area_um2,nucleus_perimeter_um,eccentricity,",
            "solidity,circularity,qc_pass,block_500_id,split\r\n",
            "source,case,specimen,time,fragment,roi,0,0,1,2,3,4,1,label,0.5,",
            "10,5,0.2,0.8,0.7,True,block,train\r\n"
        )
        .as_bytes()
        .to_vec()
    }

    #[test]
    fn candidate_finalization_requires_the_exact_verified_graph_binding() {
        let (expected, candidate, graph) = fixture();
        let imported = candidate
            .finalize(&expected, &graph)
            .expect("verified finalization");
        assert_eq!(imported.table().row_count(), 1);
        assert_eq!(imported.table().dimension(), 1_280);

        let (expected, candidate, graph) = fixture();
        let mismatched = VerifiedCellEmbeddingArtifactGraph {
            row_link_logical_digest: ContentDigest::from_bytes(b"other-row-link"),
            ..graph
        };
        assert!(matches!(
            candidate.finalize(&expected, &mismatched),
            Err(SourceBundleError::Import {
                reason: ImportFailure::VerifiedGraphMismatch,
            })
        ));
    }
}
