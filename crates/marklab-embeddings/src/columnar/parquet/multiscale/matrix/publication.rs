use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, LocalArtifactStore,
};

use crate::{PatchEmbeddingTable, RegionEmbeddingTable, SlideEmbeddingTable};

use super::writer::{
    write_patch_embedding_table_parquet, write_region_embedding_table_parquet,
    write_slide_embedding_table_parquet,
};
use crate::columnar::{
    multiscale::{matrix_dependencies, MultiscaleMatrixTable},
    ColumnarWriteSummary, EmbeddingColumnarBudgets, MultiscaleColumnarPublicationError,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

macro_rules! typed_publication {
    ($name:ident, $table:ty, $writer:path) => {
        #[doc = "Count/hash, replay, and publish one fresh canonical C-05 Parquet matrix."]
        pub fn $name(
            store: &LocalArtifactStore,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
            let summary = $writer(&mut io::sink(), table, budgets)?;
            let draft = draft(summary, table)?;
            store
                .publish_new_send(&draft, |output| {
                    $writer(output, table, budgets)
                        .map(|_| ())
                        .map_err(|_| io::Error::other("canonical multiscale matrix replay failed"))
                })
                .map_err(MultiscaleColumnarPublicationError::Store)
        }
    };
}

typed_publication!(
    publish_patch_embedding_table_parquet,
    PatchEmbeddingTable,
    write_patch_embedding_table_parquet
);
typed_publication!(
    publish_region_embedding_table_parquet,
    RegionEmbeddingTable,
    write_region_embedding_table_parquet
);
typed_publication!(
    publish_slide_embedding_table_parquet,
    SlideEmbeddingTable,
    write_slide_embedding_table_parquet
);

fn draft(
    summary: ColumnarWriteSummary,
    table: &dyn MultiscaleMatrixTable,
) -> Result<ArtifactDraft, MultiscaleColumnarPublicationError> {
    let profile = table.profile();
    let encoding = SpatialPhysicalEncoding::Parquet;
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
