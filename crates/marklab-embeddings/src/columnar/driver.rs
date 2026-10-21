//! Shared publication, integrity, and typed dispatch for canonical columnar artifacts.
use std::io::{self, Read, Seek, SeekFrom, Write};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactReadSeek, ArtifactRecord, ContentDigestWriter,
    LocalArtifactStore, VerifiedReaderError,
};

use super::{ColumnarError, ColumnarPublicationError, EmbeddingColumnarBudgets};
use crate::multiscale::physical::{SpatialArtifactRole, SpatialPhysicalEncoding};
use crate::*;

/// Canonical Arrow IPC encoding.
pub struct ArrowIpc;
/// Canonical Parquet encoding.
pub struct Parquet;

/// Closed set of typed columnar artifact families.
pub trait ColumnarEntity: sealed::Entity {
    /// Reconstructed domain value, without a verification receipt.
    type Value;
    /// Caller-owned domain bindings required for reconstruction.
    type ReadContext<'a>;
    /// Domain value and bindings required to publish an artifact.
    type WriteContext<'a>: Sync;
    /// Domain value and proofs required to verify an artifact.
    type VerifyContext<'a>;
    /// Existing graph-bound receipt minted after physical verification.
    type Verified;
}

/// An encoding supported for a particular typed artifact family.
pub trait ColumnarFormat<E: ColumnarEntity>: sealed::Format<E> {}

/// Bind a cell matrix read to its expected cells, row linkage, and verified graph.
pub struct CellEmbeddingTableReadContext<'a> {
    /// Frozen expected cells.
    pub expected: &'a ExpectedCellSet,
    /// Authoritative row linkage and extraction statuses.
    pub row_link: &'a CellEmbeddingRowLink,
    /// Verified input artifact graph.
    pub graph: VerifiedCellEmbeddingArtifactGraph,
}

/// Cell matrix publication inputs.
pub struct CellEmbeddingTableWriteContext<'a> {
    /// Canonical matrix.
    pub table: &'a CellEmbeddingTable,
    /// Exact artifact identities persisted in its physical metadata.
    pub bindings: CellEmbeddingTablePhysicalBindings,
}

/// Cell row-link verification inputs.
pub struct CellEmbeddingRowLinkVerifyContext<'a> {
    /// Frozen expected cells.
    pub expected: &'a ExpectedCellSet,
    /// Canonical row link to compare with the physical rows.
    pub row_link: &'a CellEmbeddingRowLink,
}

/// Patch matrix verification inputs.
pub struct PatchEmbeddingTableVerifyContext<'a> {
    /// Canonical matrix to compare with the physical rows.
    pub table: &'a PatchEmbeddingTable,
    /// Canonical source-row linkage.
    pub source_row_link: &'a PatchEmbeddingSourceRowLink,
    /// Previously verified support artifact.
    pub support: VerifiedPatchEmbeddingSupportArtifact,
    /// Verified direct-patch input graph.
    pub graph: VerifiedDirectPatchEmbeddingArtifactGraph,
}

/// Footprint publication inputs.
pub struct PatchFootprintWriteContext<'a> {
    /// Frozen expected patches.
    pub expected: &'a ExpectedPatchSet,
    /// Exact patch extraction geometry.
    pub context: &'a PatchEmbeddingContext,
    /// Canonical footprints.
    pub footprints: &'a PatchFootprintSet,
}

/// Overlap publication inputs.
pub struct PatchOverlapWriteContext<'a> {
    /// Frozen expected patches.
    pub expected: &'a ExpectedPatchSet,
    /// Exact patch extraction geometry.
    pub context: &'a PatchEmbeddingContext,
    /// Canonical footprints.
    pub footprints: &'a PatchFootprintSet,
    /// Canonical overlap graph.
    pub overlap: &'a PatchOverlapGraph,
}

/// Footprint verification inputs.
pub struct PatchFootprintVerifyContext<'a> {
    /// Exact domain values to compare with the file.
    pub values: PatchFootprintWriteContext<'a>,
    /// Verified direct-patch input graph.
    pub graph: VerifiedDirectPatchEmbeddingArtifactGraph,
}

/// Overlap verification inputs.
pub struct PatchOverlapVerifyContext<'a> {
    /// Exact domain values to compare with the file.
    pub values: PatchOverlapWriteContext<'a>,
    /// Previously verified footprint artifact.
    pub verified_footprints: VerifiedPatchFootprintArtifact,
    /// Verified direct-patch input graph.
    pub graph: VerifiedDirectPatchEmbeddingArtifactGraph,
}

/// Cell-patch verification inputs shared by its assignment and edge artifacts.
pub struct CellPatchVerifyContext<'a> {
    /// Canonical cell-patch relation.
    pub link: &'a CellPatchLink,
    /// Verified input graph.
    pub graph: VerifiedCellPatchInputArtifactGraph,
}

/// Patch-region verification inputs.
pub struct PatchRegionVerifyContext<'a> {
    /// Canonical patch-region relation.
    pub link: &'a PatchRegionLink,
    /// Verified input graph.
    pub graph: VerifiedPatchRegionInputArtifactGraph,
}

macro_rules! entity {
    ($marker:ident, $value:ty, $read:ty, $write:ty, $verify:ty, $receipt:ty) => {
        #[doc = concat!("Typed columnar dispatch for `", stringify!($value), "`.")]
        pub struct $marker;
        impl sealed::Entity for $marker {}
        impl ColumnarEntity for $marker {
            type Value = $value;
            type ReadContext<'a> = $read;
            type WriteContext<'a> = $write;
            type VerifyContext<'a> = $verify;
            type Verified = $receipt;
        }
    };
}
entity!(
    CellEmbeddingTableEntity,
    CellEmbeddingTable,
    CellEmbeddingTableReadContext<'a>,
    CellEmbeddingTableWriteContext<'a>,
    CellEmbeddingTableReadContext<'a>,
    VerifiedCellEmbeddingTableArtifact
);
entity!(
    CellEmbeddingRowLinkEntity,
    CellEmbeddingRowLink,
    CellEmbeddingRowLinkReadContext<'a>,
    &'a CellEmbeddingRowLink,
    CellEmbeddingRowLinkVerifyContext<'a>,
    VerifiedCellEmbeddingRowLinkArtifact
);
entity!(
    PatchEmbeddingTableEntity,
    PatchEmbeddingTable,
    PatchEmbeddingTableReadContext<'a>,
    &'a PatchEmbeddingTable,
    PatchEmbeddingTableVerifyContext<'a>,
    VerifiedPatchEmbeddingTableArtifact
);
entity!(
    RegionEmbeddingTableEntity,
    RegionEmbeddingTable,
    RegionEmbeddingTableReadContext<'a>,
    &'a RegionEmbeddingTable,
    &'a DerivedRegionEmbeddingTableCandidate,
    VerifiedRegionEmbeddingTableArtifact
);
entity!(
    SlideEmbeddingTableEntity,
    SlideEmbeddingTable,
    SlideEmbeddingTableReadContext<'a>,
    &'a SlideEmbeddingTable,
    &'a DerivedSlideEmbeddingTableCandidate,
    VerifiedSlideEmbeddingTableArtifact
);
entity!(
    PatchFootprintEntity,
    PatchFootprintSet,
    PatchFootprintReadContext<'a>,
    PatchFootprintWriteContext<'a>,
    PatchFootprintVerifyContext<'a>,
    VerifiedPatchFootprintArtifact
);
entity!(
    PatchOverlapEntity,
    PatchOverlapGraph,
    PatchOverlapReadContext<'a>,
    PatchOverlapWriteContext<'a>,
    PatchOverlapVerifyContext<'a>,
    VerifiedPatchOverlapArtifact
);
entity!(
    CellPatchAssignmentEntity,
    CellPatchLink,
    CellPatchAssignmentReadContext<'a>,
    &'a CellPatchLink,
    CellPatchVerifyContext<'a>,
    VerifiedCellPatchAssignmentArtifact
);
entity!(
    CellPatchEdgeEntity,
    CellPatchLink,
    CellPatchEdgeReadContext<'a>,
    &'a CellPatchLink,
    CellPatchVerifyContext<'a>,
    VerifiedCellPatchEdgeArtifact
);
entity!(
    PatchRegionEntity,
    PatchRegionLink,
    PatchRegionReadContext<'a>,
    &'a PatchRegionLink,
    PatchRegionVerifyContext<'a>,
    VerifiedPatchRegionLinkArtifact
);

/// Count/hash, declare, replay, and durably publish a typed canonical artifact.
pub fn publish<F: ColumnarFormat<E>, E: ColumnarEntity>(
    store: &LocalArtifactStore,
    context: &E::WriteContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    let draft = F::publication_draft(context, budgets)?;
    store
        .publish_new_send(&draft, |output| {
            F::write(output, context, budgets).map_err(io::Error::other)
        })
        .map_err(ColumnarPublicationError::Store)
}

/// Read a typed artifact from a seekable source with bounded content verification.
///
/// The source must remain stable for this call; use the managed-store adapter for
/// its existing coordination and pre/post verification guarantees.
pub fn read<F: ColumnarFormat<E>, E: ColumnarEntity>(
    mut source: impl Read + Seek,
    record: &ArtifactRecord,
    context: &E::ReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<E::Value, ColumnarError> {
    // Preserve raw-profile error precedence for a mismatched bare source while
    // retaining early file-limit and I/O failures. No value escapes a mismatch.
    let integrity = match verify_content::<F, E>(&mut source, record, budgets) {
        Ok(()) => Ok(()),
        Err(error @ ColumnarError::ArtifactBindingMismatch) => Err(error),
        Err(error) => return Err(error),
    };
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| F::read_error())?;
    let value = F::read(&mut source, record, context, budgets)?;
    integrity?;
    Ok(value)
}

/// Verify physical rows and existing domain proofs without materializing another domain value.
///
/// The source must remain stable for this call. Profile errors take precedence over
/// content mismatches, and content must match before a receipt is returned.
pub fn verify<F: ColumnarFormat<E>, E: ColumnarEntity>(
    mut source: impl Read + Seek,
    record: &ArtifactRecord,
    context: &E::VerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<E::Verified, ColumnarError> {
    // Preserve raw-profile error precedence for a mismatched bare source while
    // retaining early file-limit and I/O failures. No value escapes a mismatch.
    let integrity = match verify_content::<F, E>(&mut source, record, budgets) {
        Ok(()) => Ok(()),
        Err(error @ ColumnarError::ArtifactBindingMismatch) => Err(error),
        Err(error) => return Err(error),
    };
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| F::read_error())?;
    let value = F::verify(&mut source, record, context, budgets)?;
    integrity?;
    Ok(value)
}

/// A borrowed managed reader inside the store's existing pre/post integrity envelope.
///
/// Instances can only be obtained through [`with_verified_columnar_reader`].
pub struct VerifiedColumnarReader<'a> {
    reader: &'a mut dyn ArtifactReadSeek,
    record: &'a ArtifactRecord,
    budgets: EmbeddingColumnarBudgets,
}

impl VerifiedColumnarReader<'_> {
    /// Reconstruct a typed value using the descriptor already verified by the store.
    pub fn read<F: ColumnarFormat<E>, E: ColumnarEntity>(
        &mut self,
        context: &E::ReadContext<'_>,
    ) -> Result<E::Value, ColumnarError> {
        self.reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| F::read_error())?;
        F::read(self.reader, self.record, context, self.budgets)
    }

    /// Verify a typed artifact without repeating the store's content-hash pass.
    pub fn verify<F: ColumnarFormat<E>, E: ColumnarEntity>(
        &mut self,
        context: &E::VerifyContext<'_>,
    ) -> Result<E::Verified, ColumnarError> {
        self.reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| F::read_error())?;
        F::verify(self.reader, self.record, context, self.budgets)
    }
}

/// Run a typed read or verification with the store's original locking and error precedence.
pub fn with_verified_columnar_reader<T>(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    budgets: EmbeddingColumnarBudgets,
    operation: impl FnOnce(&mut VerifiedColumnarReader<'_>) -> Result<T, ColumnarError>,
) -> Result<T, VerifiedReaderError<ColumnarError>> {
    super::enforce_file_budget(record.content().byte_len(), budgets)
        .map_err(VerifiedReaderError::Callback)?;
    store.with_verified_reader(record, |reader| {
        operation(&mut VerifiedColumnarReader {
            reader,
            record,
            budgets,
        })
    })
}

fn verify_content<F: ColumnarFormat<E>, E: ColumnarEntity>(
    source: &mut (impl Read + Seek),
    record: &ArtifactRecord,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), ColumnarError> {
    let length = source.seek(SeekFrom::End(0)).map_err(|_| F::read_error())?;
    super::enforce_file_budget(length, budgets)?;
    if length != record.content().byte_len() {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| F::read_error())?;
    let mut digest = ContentDigestWriter::default();
    // Bound the pass even if an external source grows after the length check.
    io::copy(&mut (&mut *source).take(length), &mut digest).map_err(|_| F::read_error())?;
    let (actual_digest, actual_length) = digest.finish();
    if actual_length != length || actual_digest != record.content().digest() {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| F::read_error())?;
    Ok(())
}

pub(super) mod sealed {
    use super::*;
    pub trait Entity {}
    pub trait Format<E: ColumnarEntity> {
        fn read_error() -> ColumnarError;
        fn publication_draft(
            context: &E::WriteContext<'_>,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<ArtifactDraft, ColumnarPublicationError>;
        fn write(
            output: &mut (dyn Write + Send),
            context: &E::WriteContext<'_>,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<(), ColumnarError>;
        fn read<R: Read + Seek + ?Sized>(
            source: &mut R,
            record: &ArtifactRecord,
            context: &E::ReadContext<'_>,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<E::Value, ColumnarError>;
        fn verify<R: Read + Seek + ?Sized>(
            source: &mut R,
            record: &ArtifactRecord,
            context: &E::VerifyContext<'_>,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<E::Verified, ColumnarError>;
    }
}

fn matrix_draft(
    encoding: SpatialPhysicalEncoding,
    table: &dyn super::multiscale::MultiscaleMatrixTable,
    summary: ColumnarWriteSummary,
) -> Result<ArtifactDraft, ColumnarPublicationError> {
    let profile = table.profile();
    artifact_draft(
        profile.content_kind(encoding),
        profile.schema_id(),
        summary.content_digest(),
        summary.encoded_byte_len(),
        profile
            .table_manifest(encoding, summary.row_count(), summary.dimension())
            .map_err(ColumnarPublicationError::declaration)?,
        super::multiscale::matrix_dependencies(table)?.into(),
    )
}

fn spatial_draft(
    encoding: SpatialPhysicalEncoding,
    role: SpatialArtifactRole,
    summary: SpatialColumnarWriteSummary,
    dependencies: Vec<marklab_project::ArtifactId>,
) -> Result<ArtifactDraft, ColumnarPublicationError> {
    use crate::multiscale::physical::{content_kind, schema_id, table_manifest};
    artifact_draft(
        content_kind(role, encoding),
        schema_id(role),
        summary.content_digest(),
        summary.encoded_byte_len(),
        table_manifest(role, encoding, summary.row_count())
            .map_err(ColumnarPublicationError::declaration)?,
        dependencies,
    )
}

fn artifact_draft(
    kind: &str,
    schema: &str,
    digest: marklab_project::ContentDigest,
    length: u64,
    manifest: marklab_project::TableManifest,
    dependencies: Vec<marklab_project::ArtifactId>,
) -> Result<ArtifactDraft, ColumnarPublicationError> {
    use marklab_project::{ArtifactRef, ArtifactSchema};
    ArtifactDraft::new(
        ArtifactRef::new(kind, digest, length).map_err(ColumnarPublicationError::declaration)?,
        ArtifactSchema::new(schema, 1).map_err(ColumnarPublicationError::declaration)?,
        Some(manifest),
        dependencies,
        std::collections::BTreeMap::new(),
    )
    .map_err(ColumnarPublicationError::declaration)
}

fn cell_draft(
    encoding: SpatialPhysicalEncoding,
    context: &CellEmbeddingTableWriteContext<'_>,
    summary: ColumnarWriteSummary,
) -> Result<ArtifactDraft, ColumnarPublicationError> {
    use marklab_project::{
        TableColumn, TableColumnType, TableFormat, TableManifest, TableScalarType,
    };
    let (kind, version, format) = match encoding {
        SpatialPhysicalEncoding::Arrow => (
            "application/vnd.marklab.cell-embedding-table.v1+arrow",
            "marklab.arrow-ipc.embedding-table.v1",
            TableFormat::ArrowIpcFile,
        ),
        SpatialPhysicalEncoding::Parquet => (
            "application/vnd.marklab.cell-embedding-table.v1+parquet",
            "marklab.parquet.embedding-table.v1",
            TableFormat::ParquetFile,
        ),
    };
    let columns = vec![
        TableColumn::new(
            "cell_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        ),
        TableColumn::new(
            "embedding",
            TableColumnType::FixedSizeList {
                element: TableScalarType::F32,
                length: summary.dimension(),
            },
            false,
        ),
        TableColumn::new(
            "embedding_status",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        ),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(ColumnarPublicationError::declaration)?;
    let manifest = TableManifest::new(
        format,
        version,
        summary.row_count(),
        columns,
        vec!["cell_id".to_owned()],
    )
    .map_err(ColumnarPublicationError::declaration)?;
    artifact_draft(
        kind,
        "marklab.cell_embedding_table",
        summary.content_digest(),
        summary.encoded_byte_len(),
        manifest,
        vec![
            context.bindings.expected_cells_artifact_id(),
            context.bindings.provenance_artifact_id(),
            context.bindings.row_link_artifact_id(),
        ],
    )
}

fn row_link_draft(
    encoding: SpatialPhysicalEncoding,
    link: &CellEmbeddingRowLink,
    summary: RowLinkColumnarWriteSummary,
) -> Result<ArtifactDraft, ColumnarPublicationError> {
    use marklab_project::{
        TableColumn, TableColumnType, TableFormat, TableManifest, TableScalarType,
    };
    let (kind, version, format) = match encoding {
        SpatialPhysicalEncoding::Arrow => (
            "application/vnd.marklab.embedding-row-link.v1+arrow",
            "marklab.arrow-ipc.embedding-row-link.v1",
            TableFormat::ArrowIpcFile,
        ),
        SpatialPhysicalEncoding::Parquet => (
            "application/vnd.marklab.embedding-row-link.v1+parquet",
            "marklab.parquet.embedding-row-link.v1",
            TableFormat::ParquetFile,
        ),
    };
    let columns = vec![
        TableColumn::new(
            "cell_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        ),
        TableColumn::new(
            "source_cell_row",
            TableColumnType::Scalar(TableScalarType::U64),
            false,
        ),
        TableColumn::new(
            "source_embedding_row",
            TableColumnType::Scalar(TableScalarType::U64),
            true,
        ),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(ColumnarPublicationError::declaration)?;
    let manifest = TableManifest::new(
        format,
        version,
        summary.row_count(),
        columns,
        vec!["cell_id".to_owned()],
    )
    .map_err(ColumnarPublicationError::declaration)?;
    artifact_draft(
        kind,
        "marklab.cell_embedding_row_link",
        summary.content_digest(),
        summary.encoded_byte_len(),
        manifest,
        link.direct_dependencies().to_vec(),
    )
}

macro_rules! format_entity {
    ($format:ty, $module:ident, $encoding:ident, $failure:ident, $entity:ty,
     $write:ident, $read:ident, $verify:ident,
     |$context:ident| ($($argument:expr),*),
     |$draft_context:ident, $summary:ident, $draft_encoding:ident| $draft:expr) => {
        impl ColumnarFormat<$entity> for $format {}
        impl sealed::Format<$entity> for $format {
            fn read_error() -> ColumnarError {
                ColumnarError::$encoding { reason: $failure::ArtifactRead }
            }
            fn publication_draft($context: &<$entity as ColumnarEntity>::WriteContext<'_>, budgets: EmbeddingColumnarBudgets)
                -> Result<ArtifactDraft, ColumnarPublicationError> {
                let $summary = super::$module::$write(&mut io::sink(), $($argument,)* budgets)?;
                let $draft_encoding = SpatialPhysicalEncoding::$encoding;
                let $draft_context = $context;
                $draft
            }
            fn write(output: &mut (dyn Write + Send), $context: &<$entity as ColumnarEntity>::WriteContext<'_>, budgets: EmbeddingColumnarBudgets)
                -> Result<(), ColumnarError> {
                super::$module::$write(output, $($argument,)* budgets).map(|_| ())
            }
            fn read<R: Read + Seek + ?Sized>(source: &mut R, record: &ArtifactRecord,
                context: &<$entity as ColumnarEntity>::ReadContext<'_>, budgets: EmbeddingColumnarBudgets)
                -> Result<<$entity as ColumnarEntity>::Value, ColumnarError> {
                super::$module::$read(source, record, context, budgets)
            }
            fn verify<R: Read + Seek + ?Sized>(source: &mut R, record: &ArtifactRecord,
                context: &<$entity as ColumnarEntity>::VerifyContext<'_>, budgets: EmbeddingColumnarBudgets)
                -> Result<<$entity as ColumnarEntity>::Verified, ColumnarError> {
                super::$module::$verify(source, record, context, budgets)
            }
        }
    };
}

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    CellEmbeddingTableEntity,
    write_cell_embedding_table_arrow,
    read_cell_embedding_table_arrow_reader,
    verify_cell_embedding_table_arrow_reader,
    |context| (context.table, context.bindings),
    |context, summary, encoding| cell_draft(encoding, context, summary)
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    CellEmbeddingRowLinkEntity,
    write_cell_embedding_row_link_arrow,
    read_cell_embedding_row_link_arrow_reader,
    verify_cell_embedding_row_link_arrow_reader,
    |context| (context),
    |context, summary, encoding| row_link_draft(encoding, context, summary)
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    PatchEmbeddingTableEntity,
    write_patch_embedding_table_arrow,
    read_patch_embedding_table_arrow_reader,
    verify_patch_embedding_table_arrow_reader,
    |context| (context),
    |context, summary, encoding| matrix_draft(encoding, *context, summary)
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    RegionEmbeddingTableEntity,
    write_region_embedding_table_arrow,
    read_region_embedding_table_arrow_reader,
    verify_region_embedding_table_arrow_reader,
    |context| (context),
    |context, summary, encoding| matrix_draft(encoding, *context, summary)
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    SlideEmbeddingTableEntity,
    write_slide_embedding_table_arrow,
    read_slide_embedding_table_arrow_reader,
    verify_slide_embedding_table_arrow_reader,
    |context| (context),
    |context, summary, encoding| matrix_draft(encoding, *context, summary)
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    PatchFootprintEntity,
    write_patch_footprint_set_arrow,
    read_patch_footprint_set_arrow_reader,
    verify_patch_footprint_set_arrow_reader,
    |context| (context.expected, context.context, context.footprints),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::Footprint,
        summary,
        {
            let mut ids = vec![
                context.footprints.expected_patches_artifact_id(),
                context.footprints.patch_context_artifact_id(),
            ];
            ids.sort_unstable();
            ids
        }
    )
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    PatchOverlapEntity,
    write_patch_overlap_graph_arrow,
    read_patch_overlap_graph_arrow_reader,
    verify_patch_overlap_graph_arrow_reader,
    |context| (
        context.expected,
        context.context,
        context.footprints,
        context.overlap
    ),
    |context, summary, encoding| spatial_draft(encoding, SpatialArtifactRole::Overlap, summary, {
        let mut ids = vec![
            context.overlap.expected_patches_artifact_id(),
            context.footprints.patch_context_artifact_id(),
            context.overlap.patch_footprints_artifact_id(),
        ];
        ids.sort_unstable();
        ids
    })
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    CellPatchAssignmentEntity,
    write_cell_patch_assignment_table_arrow,
    read_cell_patch_assignment_table_arrow_reader,
    verify_cell_patch_assignment_table_arrow_reader,
    |context| (context),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::CellPatchAssignment,
        summary,
        super::multiscale::cell_patch_dependencies(context).into()
    )
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    CellPatchEdgeEntity,
    write_cell_patch_edge_table_arrow,
    read_cell_patch_edge_table_arrow_reader,
    verify_cell_patch_edge_table_arrow_reader,
    |context| (context),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::CellPatchEdge,
        summary,
        super::multiscale::cell_patch_dependencies(context).into()
    )
);

format_entity!(
    ArrowIpc,
    arrow,
    Arrow,
    ArrowIpcFailure,
    PatchRegionEntity,
    write_patch_region_link_arrow,
    read_patch_region_link_arrow_reader,
    verify_patch_region_link_arrow_reader,
    |context| (context),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::PatchRegion,
        summary,
        super::multiscale::patch_region_dependencies(context).into()
    )
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    CellEmbeddingTableEntity,
    write_cell_embedding_table_parquet,
    read_cell_embedding_table_parquet_reader,
    verify_cell_embedding_table_parquet_reader,
    |context| (context.table, context.bindings),
    |context, summary, encoding| cell_draft(encoding, context, summary)
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    CellEmbeddingRowLinkEntity,
    write_cell_embedding_row_link_parquet,
    read_cell_embedding_row_link_parquet_reader,
    verify_cell_embedding_row_link_parquet_reader,
    |context| (context),
    |context, summary, encoding| row_link_draft(encoding, context, summary)
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    PatchEmbeddingTableEntity,
    write_patch_embedding_table_parquet,
    read_patch_embedding_table_parquet_reader,
    verify_patch_embedding_table_parquet_reader,
    |context| (context),
    |context, summary, encoding| matrix_draft(encoding, *context, summary)
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    RegionEmbeddingTableEntity,
    write_region_embedding_table_parquet,
    read_region_embedding_table_parquet_reader,
    verify_region_embedding_table_parquet_reader,
    |context| (context),
    |context, summary, encoding| matrix_draft(encoding, *context, summary)
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    SlideEmbeddingTableEntity,
    write_slide_embedding_table_parquet,
    read_slide_embedding_table_parquet_reader,
    verify_slide_embedding_table_parquet_reader,
    |context| (context),
    |context, summary, encoding| matrix_draft(encoding, *context, summary)
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    PatchFootprintEntity,
    write_patch_footprint_set_parquet,
    read_patch_footprint_set_parquet_reader,
    verify_patch_footprint_set_parquet_reader,
    |context| (context.expected, context.context, context.footprints),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::Footprint,
        summary,
        {
            let mut ids = vec![
                context.footprints.expected_patches_artifact_id(),
                context.footprints.patch_context_artifact_id(),
            ];
            ids.sort_unstable();
            ids
        }
    )
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    PatchOverlapEntity,
    write_patch_overlap_graph_parquet,
    read_patch_overlap_graph_parquet_reader,
    verify_patch_overlap_graph_parquet_reader,
    |context| (
        context.expected,
        context.context,
        context.footprints,
        context.overlap
    ),
    |context, summary, encoding| spatial_draft(encoding, SpatialArtifactRole::Overlap, summary, {
        let mut ids = vec![
            context.overlap.expected_patches_artifact_id(),
            context.footprints.patch_context_artifact_id(),
            context.overlap.patch_footprints_artifact_id(),
        ];
        ids.sort_unstable();
        ids
    })
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    CellPatchAssignmentEntity,
    write_cell_patch_assignment_table_parquet,
    read_cell_patch_assignment_table_parquet_reader,
    verify_cell_patch_assignment_table_parquet_reader,
    |context| (context),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::CellPatchAssignment,
        summary,
        super::multiscale::cell_patch_dependencies(context).into()
    )
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    CellPatchEdgeEntity,
    write_cell_patch_edge_table_parquet,
    read_cell_patch_edge_table_parquet_reader,
    verify_cell_patch_edge_table_parquet_reader,
    |context| (context),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::CellPatchEdge,
        summary,
        super::multiscale::cell_patch_dependencies(context).into()
    )
);

format_entity!(
    Parquet,
    parquet,
    Parquet,
    ParquetFailure,
    PatchRegionEntity,
    write_patch_region_link_parquet,
    read_patch_region_link_parquet_reader,
    verify_patch_region_link_parquet_reader,
    |context| (context),
    |context, summary, encoding| spatial_draft(
        encoding,
        SpatialArtifactRole::PatchRegion,
        summary,
        super::multiscale::patch_region_dependencies(context).into()
    )
);
