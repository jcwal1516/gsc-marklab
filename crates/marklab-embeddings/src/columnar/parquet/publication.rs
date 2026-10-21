use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_cell_embedding_table_parquet(
    store: &LocalArtifactStore,
    table: &CellEmbeddingTable,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, CellEmbeddingTableEntity>(
        store,
        &CellEmbeddingTableWriteContext { table, bindings },
        budgets,
    )
}
