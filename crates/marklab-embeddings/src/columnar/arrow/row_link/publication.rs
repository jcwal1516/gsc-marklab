use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_cell_embedding_row_link_arrow(
    store: &LocalArtifactStore,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, CellEmbeddingRowLinkEntity>(store, &row_link, budgets)
}
