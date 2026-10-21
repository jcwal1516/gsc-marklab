use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_patch_embedding_table_parquet(
    store: &LocalArtifactStore,
    table: &PatchEmbeddingTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, PatchEmbeddingTableEntity>(store, &table, budgets)
}

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_region_embedding_table_parquet(
    store: &LocalArtifactStore,
    table: &RegionEmbeddingTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, RegionEmbeddingTableEntity>(store, &table, budgets)
}

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_slide_embedding_table_parquet(
    store: &LocalArtifactStore,
    table: &SlideEmbeddingTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, SlideEmbeddingTableEntity>(store, &table, budgets)
}
