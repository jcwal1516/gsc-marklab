use crate::columnar::driver::*;
pub use crate::columnar::error::ColumnarPublicationError;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_cell_embedding_table_arrow(
    store: &LocalArtifactStore,
    table: &CellEmbeddingTable,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, CellEmbeddingTableEntity>(
        store,
        &CellEmbeddingTableWriteContext { table, bindings },
        budgets,
    )
}
