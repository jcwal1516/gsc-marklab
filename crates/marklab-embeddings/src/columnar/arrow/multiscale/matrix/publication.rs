use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, LocalArtifactStore,
};

use crate::{PatchEmbeddingTable, RegionEmbeddingTable, SlideEmbeddingTable};

use super::writer::{
    write_patch_embedding_table_arrow, write_region_embedding_table_arrow,
    write_slide_embedding_table_arrow,
};
use crate::columnar::arrow::multiscale::MultiscaleColumnarPublicationError;
use crate::columnar::{
    multiscale::{matrix_dependencies, MultiscaleMatrixTable},
    ColumnarWriteSummary, EmbeddingColumnarBudgets, MultiscaleColumnarError,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

macro_rules! typed_publication {
    ($name:ident, $table:ty, $writer:path) => {
        #[doc = "Count/hash, replay, and publish a fresh canonical C-05 Arrow matrix."]
        pub fn $name(
            store: &LocalArtifactStore,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
            publish_matrix(store, table, budgets, $writer)
        }
    };
}

typed_publication!(
    publish_patch_embedding_table_arrow,
    PatchEmbeddingTable,
    write_patch_embedding_table_arrow
);
typed_publication!(
    publish_region_embedding_table_arrow,
    RegionEmbeddingTable,
    write_region_embedding_table_arrow
);
typed_publication!(
    publish_slide_embedding_table_arrow,
    SlideEmbeddingTable,
    write_slide_embedding_table_arrow
);

fn publish_matrix<T: MultiscaleMatrixTable>(
    store: &LocalArtifactStore,
    table: &T,
    budgets: EmbeddingColumnarBudgets,
    write: fn(
        &mut dyn std::io::Write,
        &T,
        EmbeddingColumnarBudgets,
    ) -> Result<ColumnarWriteSummary, MultiscaleColumnarError>,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    let summary = write(&mut io::sink(), table, budgets)?;
    let draft = draft(summary, table)?;
    store
        .publish_new_send(&draft, |output| {
            write(output, table, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical multiscale Arrow matrix replay failed"))
        })
        .map_err(MultiscaleColumnarPublicationError::Store)
}

fn draft(
    summary: ColumnarWriteSummary,
    table: &dyn MultiscaleMatrixTable,
) -> Result<ArtifactDraft, MultiscaleColumnarPublicationError> {
    let profile = table.profile();
    let encoding = SpatialPhysicalEncoding::Arrow;
    let content = ArtifactRef::new(
        profile.content_kind(encoding),
        summary.content_digest(),
        summary.encoded_byte_len(),
    )
    .map_err(map_declaration_error)?;
    let manifest = profile
        .table_manifest(encoding, summary.row_count(), summary.dimension())
        .map_err(map_declaration_error)?;
    ArtifactDraft::new(
        content,
        ArtifactSchema::new(profile.schema_id(), 1).map_err(map_declaration_error)?,
        Some(manifest),
        matrix_dependencies(table)
            .map_err(MultiscaleColumnarPublicationError::Columnar)?
            .into(),
        BTreeMap::new(),
    )
    .map_err(map_declaration_error)
}

fn map_declaration_error<T>(_: T) -> MultiscaleColumnarPublicationError {
    MultiscaleColumnarPublicationError::Declaration
}
