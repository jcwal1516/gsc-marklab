use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_patch_embedding_table_arrow(
    store: &LocalArtifactStore,
    table: &PatchEmbeddingTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, PatchEmbeddingTableEntity>(store, &table, budgets)
}

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_region_embedding_table_arrow(
    store: &LocalArtifactStore,
    table: &RegionEmbeddingTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, RegionEmbeddingTableEntity>(store, &table, budgets)
}

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_slide_embedding_table_arrow(
    store: &LocalArtifactStore,
    table: &SlideEmbeddingTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, SlideEmbeddingTableEntity>(store, &table, budgets)
}
