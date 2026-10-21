use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_cell_embedding_row_link_parquet(
    store: &LocalArtifactStore,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, CellEmbeddingRowLinkEntity>(store, &row_link, budgets)
}
