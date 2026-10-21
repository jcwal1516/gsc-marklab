use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_cell_patch_assignment_table_parquet(
    store: &LocalArtifactStore,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, CellPatchAssignmentEntity>(store, &link, budgets)
}

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_cell_patch_edge_table_parquet(
    store: &LocalArtifactStore,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, CellPatchEdgeEntity>(store, &link, budgets)
}
